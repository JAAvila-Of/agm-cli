//! `validate` command: parse and validate an AGM file against the spec.

use std::path::Path;

use agm_core::error::output::ErrorOutputFormat;
use agm_core::model::schema::EnforcementLevel;
use agm_core::validator::{ValidateOptions, validate};

use super::helpers;

pub fn run(file: &Path, enforcement: EnforcementLevel, errors_format: ErrorOutputFormat) -> i32 {
    let file_name = file.display().to_string();
    let source = helpers::read_file(file);

    // Parse
    let agm_file = helpers::parse_or_exit(&source, &file_name, errors_format);

    // Validate
    let options = ValidateOptions {
        enforcement_level: enforcement,
        import_resolver: None,
        ..Default::default()
    };
    let collection = validate(&agm_file, &source, &file_name, &options);

    // Report results
    if collection.is_empty() {
        if errors_format == ErrorOutputFormat::Text {
            eprintln!("{}: OK", file_name);
        }
        helpers::EXIT_SUCCESS
    } else {
        helpers::render_diagnostics(&collection, errors_format);
        if collection.has_errors() {
            helpers::EXIT_VALIDATION_ERROR
        } else {
            // Warnings only — still exit 0
            helpers::EXIT_SUCCESS
        }
    }
}
