//! `compile` command: compile a Markdown file into an AGM file.

use std::path::Path;

use agm_core::compiler::{CompileOptions, compile};
use agm_core::renderer::canonical::render_canonical;
use agm_core::renderer::json::render_json;
use agm_core::validator::{ValidateOptions, validate};

use super::helpers;

#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &Path,
    output: Option<&Path>,
    package: &str,
    version: &str,
    id_prefix: Option<&str>,
    min_confidence: f32,
    do_validate: bool,
    json_output: bool,
) -> i32 {
    // Read Markdown input
    let source = helpers::read_file(input);

    // Compile
    let options = CompileOptions {
        package: package.to_owned(),
        version: version.to_owned(),
        min_confidence,
        merge_same_type: false,
        id_prefix: id_prefix.map(|s| s.to_owned()),
    };

    let result = compile(&source, &options);

    // Report warnings
    for warning in &result.warnings {
        let line_info = warning
            .source_line
            .map(|l| format!(" (line {l})"))
            .unwrap_or_default();
        eprintln!(
            "warning: {:?}{}: {}",
            warning.kind, line_info, warning.message
        );
    }

    // Check if we got any nodes
    if result.file.nodes.is_empty() {
        eprintln!("error: compilation produced no nodes");
        return helpers::EXIT_VALIDATION_ERROR;
    }

    // Optional validation
    if do_validate {
        let rendered = render_canonical(&result.file);
        let file_name = input.display().to_string();
        let opts = ValidateOptions::default();
        let collection = validate(&result.file, &rendered, &file_name, &opts);

        if collection.has_errors() {
            eprintln!("Compiled output has validation errors:");
            helpers::render_diagnostics(
                &collection,
                agm_core::error::output::ErrorOutputFormat::Text,
            );
            return helpers::EXIT_VALIDATION_ERROR;
        }

        if !collection.is_empty() {
            eprintln!("Compiled output has warnings:");
            helpers::render_diagnostics(
                &collection,
                agm_core::error::output::ErrorOutputFormat::Text,
            );
        }
    }

    // Render output
    let output_text = if json_output {
        render_json(&result.file)
    } else {
        render_canonical(&result.file)
    };

    // Write output
    match output {
        Some(path) => {
            if let Err(e) = std::fs::write(path, &output_text) {
                eprintln!("error: cannot write {}: {}", path.display(), e);
                return helpers::EXIT_IO_ERROR;
            }
            eprintln!(
                "Compiled {} node(s) from {} -> {}",
                result.file.nodes.len(),
                input.display(),
                path.display()
            );
        }
        None => {
            print!("{output_text}");
        }
    }

    helpers::EXIT_SUCCESS
}
