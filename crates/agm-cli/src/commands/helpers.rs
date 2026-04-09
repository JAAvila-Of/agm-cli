//! Shared helper functions used by all command modules.

use std::path::Path;
use std::process;

use agm_core::error::diagnostic::{AgmError, DiagnosticCollection};
use agm_core::error::output::{ErrorOutputFormat, format_error_json};
use agm_core::model::file::AgmFile;

/// Exit with code 0: success.
pub const EXIT_SUCCESS: i32 = 0;
/// Exit with code 1: validation errors found.
pub const EXIT_VALIDATION_ERROR: i32 = 1;
/// Exit with code 2: file not found or I/O error.
pub const EXIT_IO_ERROR: i32 = 2;

/// Reads a file from disk. On failure, prints a message to stderr and exits
/// with code 2.
pub fn read_file(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", path.display(), e);
            process::exit(EXIT_IO_ERROR);
        }
    }
}

/// Parses AGM source text. On parse failure, renders diagnostics in the
/// requested format (text or json) to stderr/stdout and exits with code 1.
///
/// Returns the parsed `AgmFile` on success.
pub fn parse_or_exit(source: &str, file_name: &str, errors_format: ErrorOutputFormat) -> AgmFile {
    match agm_core::parser::parse(source) {
        Ok(file) => file,
        Err(errors) => {
            render_errors(&errors, file_name, source, errors_format);
            process::exit(EXIT_VALIDATION_ERROR);
        }
    }
}

/// Renders a list of `AgmError`s to stderr (text) or stdout (json).
pub fn render_errors(
    errors: &[AgmError],
    file_name: &str,
    source: &str,
    format: ErrorOutputFormat,
) {
    match format {
        ErrorOutputFormat::Text => {
            let mut collection = DiagnosticCollection::new(file_name, source);
            collection.extend(errors.iter().cloned());
            eprint!("{}", collection.render_miette());
        }
        ErrorOutputFormat::Json => {
            let json_errors: Vec<serde_json::Value> =
                errors.iter().map(format_error_json).collect();
            let json_array = serde_json::Value::Array(json_errors);
            println!("{}", serde_json::to_string_pretty(&json_array).unwrap());
        }
    }
}

/// Renders a `DiagnosticCollection` (from validator) to stderr (text) or stdout (json).
pub fn render_diagnostics(collection: &DiagnosticCollection, format: ErrorOutputFormat) {
    match format {
        ErrorOutputFormat::Text => {
            eprint!("{}", collection.render_miette());
        }
        ErrorOutputFormat::Json => {
            let json_errors: Vec<serde_json::Value> = collection
                .diagnostics()
                .iter()
                .map(format_error_json)
                .collect();
            let json_array = serde_json::Value::Array(json_errors);
            println!("{}", serde_json::to_string_pretty(&json_array).unwrap());
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2 helpers
// ---------------------------------------------------------------------------

use agm_core::graph::{AgmGraph, build_graph};
use agm_core::validator::{ValidateOptions, validate};

use crate::runtime::memory::{self, MemoryRuntime};
use crate::runtime::state::ExecutionTracker;

/// Exit code for in-progress / no-checks / IO-error conditions.
pub const EXIT_IN_PROGRESS: i32 = 2;

/// Parsed and validated AGM file with its dependency graph.
pub struct ParsedAgm {
    pub file: AgmFile,
    #[allow(dead_code)]
    pub source: String,
    pub graph: AgmGraph,
}

/// Full Phase 2 runtime context: parsed file + tracker + memory.
pub struct RuntimeContext {
    pub parsed: ParsedAgm,
    pub tracker: ExecutionTracker,
    pub memory: MemoryRuntime,
}

/// Parses, validates (standard enforcement), and builds graph.
/// On failure, prints errors to stderr and exits.
pub fn parse_and_build_graph(path: &Path) -> ParsedAgm {
    let file_name = path.display().to_string();
    let source = read_file(path);
    let file = parse_or_exit(&source, &file_name, ErrorOutputFormat::Text);

    let options = ValidateOptions {
        enforcement_level: agm_core::model::schema::EnforcementLevel::Standard,
        import_resolver: None,
    };
    let collection = validate(&file, &source, &file_name, &options);
    if collection.has_errors() {
        render_diagnostics(&collection, ErrorOutputFormat::Text);
        process::exit(EXIT_VALIDATION_ERROR);
    }
    if !collection.is_empty() {
        render_diagnostics(&collection, ErrorOutputFormat::Text);
    }

    let graph = build_graph(&file);
    ParsedAgm {
        file,
        source,
        graph,
    }
}

/// Builds the full runtime context (file + tracker + memory).
pub fn build_runtime_context(path: &Path) -> RuntimeContext {
    let parsed = parse_and_build_graph(path);
    let tracker = match ExecutionTracker::new(path, &parsed.file, &parsed.graph) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: failed to initialize state tracker: {e}");
            process::exit(EXIT_IO_ERROR);
        }
    };
    let mem_project_path = memory::mem_path_from_agm(path);
    let global_path = match memory::global_mem_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: failed to resolve global memory path: {e}");
            process::exit(EXIT_IO_ERROR);
        }
    };
    let memory =
        match MemoryRuntime::new(mem_project_path, global_path, &parsed.file.header.package) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("error: failed to initialize memory runtime: {e}");
                process::exit(EXIT_IO_ERROR);
            }
        };
    RuntimeContext {
        parsed,
        tracker,
        memory,
    }
}

/// Finds a node by ID in the parsed file, or prints an error and exits.
pub fn find_node_or_exit<'a>(file: &'a AgmFile, node_id: &str) -> &'a agm_core::model::node::Node {
    file.nodes
        .iter()
        .find(|n| n.id == node_id)
        .unwrap_or_else(|| {
            eprintln!("error: node '{}' not found in file", node_id);
            process::exit(EXIT_VALIDATION_ERROR);
        })
}
