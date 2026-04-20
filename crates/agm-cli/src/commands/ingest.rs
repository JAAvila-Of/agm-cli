//! `ingest` command: convert tool-call JSON args into canonical AGM text.
//!
//! Exit codes:
//!   0 — ingested successfully, AGM emitted.
//!   1 — schema or validation failure (diagnostics to stderr).
//!   2 — input parse error (not valid JSON).
//!   3 — missing required flag or argument error.

use std::io::{self, Read, Write};
use std::path::Path;

use agm_core::ingest::{IngestConfig, IngestError, ingest_many, ingest_one};
use agm_core::model::fields::NodeType;
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::schema::EnforcementLevel;
use agm_core::renderer::{RenderFormat, render};
use agm_core::validator::{self, ValidateOptions};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------

const EXIT_OK: i32 = 0;
const EXIT_VALIDATION_FAILURE: i32 = 1;
const EXIT_JSON_PARSE_ERROR: i32 = 2;
const EXIT_MISSING_ARG: i32 = 3;

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Entry point called from `main.rs`.
///
/// # Parameters
/// - `type_str` — node type string (e.g. "ticket", "workflow")
/// - `package` — required package name for the generated AGM file header
/// - `id` — optional node id / id prefix for batches
/// - `file` — read JSON from this path instead of stdin
/// - `no_normalize` — skip the normalize pass
/// - `no_schema_check` — skip the pre-build JSON Schema check
/// - `enforcement` — enforcement level for post-build validation
/// - `output` — write AGM output to this path instead of stdout
/// - `version` — package version in the generated file header (default "0.1.0")
/// - `header_title` — optional title field in the generated file header
///
/// # Returns
///
/// The process exit code (0–3).
#[allow(clippy::too_many_arguments)]
pub fn run(
    type_str: &str,
    package: &str,
    id: Option<&str>,
    file: Option<&Path>,
    no_normalize: bool,
    no_schema_check: bool,
    enforcement: EnforcementLevel,
    output: Option<&Path>,
    version: &str,
    header_title: Option<&str>,
) -> i32 {
    // ---- Parse node type ----
    let node_type = match parse_node_type(type_str) {
        Ok(t) => t,
        Err(msg) => {
            eprintln!("error: {msg}");
            eprintln!("hint: run `agm ingest --help` for supported types");
            return EXIT_MISSING_ARG;
        }
    };

    // ---- Read JSON input ----
    let json_bytes = match read_input(file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_JSON_PARSE_ERROR;
        }
    };

    let json_value: Value = match serde_json::from_slice(&json_bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: invalid JSON: {e}");
            return EXIT_JSON_PARSE_ERROR;
        }
    };

    // ---- Build IngestConfig ----
    let config = IngestConfig {
        normalize: !no_normalize,
        schema_check: !no_schema_check,
        enforcement: enforcement.clone(),
    };

    // ---- Dispatch: object vs array ----
    let nodes = match &json_value {
        Value::Array(arr) => {
            // Batch mode
            let id_prefix = match id {
                Some(p) => p,
                None => {
                    eprintln!(
                        "error: --id is required when ingesting a JSON array (used as id prefix)"
                    );
                    return EXIT_MISSING_ARG;
                }
            };
            match ingest_many(node_type.clone(), id_prefix, arr.clone(), &config) {
                Ok(nodes) => nodes,
                Err(e) => {
                    print_ingest_error(&e);
                    return ingest_error_exit_code(&e);
                }
            }
        }
        Value::Object(_) => {
            // Single-node mode
            let node_id = id.unwrap_or("");
            match ingest_one(node_type.clone(), node_id, json_value.clone(), &config) {
                Ok(node) => vec![node],
                Err(e) => {
                    print_ingest_error(&e);
                    return ingest_error_exit_code(&e);
                }
            }
        }
        _ => {
            eprintln!("error: input must be a JSON object or array of objects");
            return EXIT_JSON_PARSE_ERROR;
        }
    };

    // ---- Assemble AgmFile ----
    let agm_file = AgmFile {
        header: Header {
            agm: "1.0".to_owned(),
            package: package.to_owned(),
            version: version.to_owned(),
            title: header_title.map(str::to_owned),
            owner: None,
            imports: None,
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        },
        nodes,
    };

    // ---- Post-build validation at requested enforcement level ----
    // Render to canonical AGM text first so we can pass source to the validator.
    let canonical = render(&agm_file, RenderFormat::Canonical);

    let validate_opts = ValidateOptions {
        enforcement_level: enforcement,
        ..Default::default()
    };
    let diagnostics = validator::validate(&agm_file, &canonical, "ingest", &validate_opts);
    if diagnostics.has_errors() {
        eprintln!(
            "error: validation failed with {} diagnostic(s)",
            diagnostics.diagnostics().len()
        );
        for d in diagnostics.diagnostics() {
            let line_str = d
                .location
                .line
                .map_or_else(String::new, |l| format!(" at line {l}"));
            eprintln!("  {}{}: {}", d.code, line_str, d.message);
        }
        return EXIT_VALIDATION_FAILURE;
    }
    // Print warnings (non-fatal)
    for d in diagnostics.diagnostics().iter().filter(|d| d.is_warning()) {
        let line_str = d
            .location
            .line
            .map_or_else(String::new, |l| format!(" at line {l}"));
        eprintln!("warning: {}{}: {}", d.code, line_str, d.message);
    }

    // ---- Write output ----
    match write_output(output, &canonical) {
        Ok(()) => EXIT_OK,
        Err(e) => {
            eprintln!("error: write failed: {e}");
            EXIT_VALIDATION_FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_node_type(s: &str) -> Result<NodeType, String> {
    match s {
        "facts" => Ok(NodeType::Facts),
        "rules" => Ok(NodeType::Rules),
        "workflow" => Ok(NodeType::Workflow),
        "entity" => Ok(NodeType::Entity),
        "decision" => Ok(NodeType::Decision),
        "exception" => Ok(NodeType::Exception),
        "example" => Ok(NodeType::Example),
        "glossary" => Ok(NodeType::Glossary),
        "anti_pattern" | "anti-pattern" => Ok(NodeType::AntiPattern),
        "orchestration" => Ok(NodeType::Orchestration),
        "ticket" => Ok(NodeType::Ticket),
        other => Err(format!(
            "unknown node type `{other}`; supported: facts, rules, workflow, entity, \
             decision, exception, example, glossary, anti_pattern, orchestration, ticket"
        )),
    }
}

fn read_input(file: Option<&Path>) -> Result<Vec<u8>, String> {
    match file {
        Some(path) => {
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
        }
        None => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| format!("cannot read stdin: {e}"))?;
            Ok(buf)
        }
    }
}

fn write_output(output: Option<&Path>, content: &str) -> Result<(), String> {
    match output {
        Some(path) => std::fs::write(path, content)
            .map_err(|e| format!("cannot write {}: {e}", path.display())),
        None => io::stdout()
            .write_all(content.as_bytes())
            .map_err(|e| format!("cannot write stdout: {e}")),
    }
}

fn print_ingest_error(e: &IngestError) {
    match e {
        IngestError::NotAnObject => eprintln!("error: input is not a JSON object"),
        IngestError::SchemaCheck(msg) => eprintln!("error: schema check failed: {msg}"),
        IngestError::MissingId => {
            eprintln!(
                "error: node id is missing — pass --id or include a \"node\" field in the JSON"
            );
        }
        IngestError::Build(build_err) => {
            eprintln!("error: build failed: {build_err}");
        }
        IngestError::JsonParse(msg) => eprintln!("error: JSON parse error: {msg}"),
        IngestError::Batch { errors } => {
            eprintln!("error: batch ingest failed ({} error(s)):", errors.len());
            for (idx, err) in errors {
                eprintln!("  [{idx}] {err}");
            }
        }
    }
}

fn ingest_error_exit_code(e: &IngestError) -> i32 {
    match e {
        IngestError::NotAnObject | IngestError::JsonParse(_) => EXIT_JSON_PARSE_ERROR,
        IngestError::MissingId => EXIT_MISSING_ARG,
        IngestError::SchemaCheck(_) | IngestError::Build(_) | IngestError::Batch { .. } => {
            EXIT_VALIDATION_FAILURE
        }
    }
}
