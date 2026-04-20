//! `lint` command: validate + extended quality heuristics.

use std::collections::HashMap;
use std::path::Path;

use agm_core::error::ErrorCode;
use agm_core::error::diagnostic::{AgmError, ErrorLocation, Severity};
use agm_core::error::output::ErrorOutputFormat;
use agm_core::model::file::AgmFile;
use agm_core::model::node::Node;
use agm_core::model::schema::EnforcementLevel;
use agm_core::validator::{ValidateOptions, validate};

use super::helpers;

pub fn run(file: &Path, enforcement: EnforcementLevel, errors_format: ErrorOutputFormat) -> i32 {
    let file_name = file.display().to_string();
    let source = helpers::read_file(file);

    // Parse
    let agm_file = helpers::parse_or_exit(&source, &file_name, errors_format);

    // Validate (full spec validation)
    let options = ValidateOptions {
        enforcement_level: enforcement,
        import_resolver: None,
        ..Default::default()
    };
    let mut collection = validate(&agm_file, &source, &file_name, &options);

    // Lint heuristics (quality checks)
    let lint_diagnostics = lint_quality_checks(&agm_file, &file_name);
    collection.extend(lint_diagnostics);

    // Report results
    if collection.is_empty() {
        if errors_format == ErrorOutputFormat::Text {
            eprintln!("{}: OK (lint)", file_name);
        }
        helpers::EXIT_SUCCESS
    } else {
        helpers::render_diagnostics(&collection, errors_format);
        if collection.has_errors() {
            helpers::EXIT_VALIDATION_ERROR
        } else {
            helpers::EXIT_SUCCESS
        }
    }
}

fn lint_quality_checks(file: &AgmFile, file_name: &str) -> Vec<AgmError> {
    let mut diags = Vec::new();

    // L001: Summary length check (info if > 100 chars and <= 200 chars)
    // Note: V012 covers > 200 chars as a warning. L001 is a quality hint.
    for node in &file.nodes {
        if node.summary.len() > 100 && node.summary.len() <= 200 {
            diags.push(AgmError::with_severity(
                ErrorCode::L001,
                Severity::Info,
                format!(
                    "Summary of node `{}` is {} chars; consider shortening (> 100 chars)",
                    node.id,
                    node.summary.len()
                ),
                ErrorLocation::new(
                    Some(file_name.to_string()),
                    Some(node.span.start_line),
                    Some(node.id.clone()),
                ),
            ));
        }
    }

    // L002: Summary uniqueness — warn if two nodes share identical summaries
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for node in &file.nodes {
        if let Some(existing_id) = seen.get(node.summary.as_str()) {
            diags.push(AgmError::with_severity(
                ErrorCode::L002,
                Severity::Info,
                format!(
                    "Nodes `{}` and `{}` have identical summaries",
                    existing_id, node.id,
                ),
                ErrorLocation::new(
                    Some(file_name.to_string()),
                    Some(node.span.start_line),
                    Some(node.id.clone()),
                ),
            ));
        } else {
            seen.insert(&node.summary, &node.id);
        }
    }

    // L003: Node granularity heuristic — info if a single node has > 15 fields
    for node in &file.nodes {
        let field_count = count_populated_fields(node);
        if field_count > 15 {
            diags.push(AgmError::with_severity(
                ErrorCode::L003,
                Severity::Info,
                format!(
                    "Node `{}` has {} populated fields; consider splitting into smaller nodes",
                    node.id, field_count,
                ),
                ErrorLocation::new(
                    Some(file_name.to_string()),
                    Some(node.span.start_line),
                    Some(node.id.clone()),
                ),
            ));
        }
    }

    diags
}

/// Counts the number of optional fields that are populated in a node.
/// Required fields (id, node_type, summary) are not counted.
fn count_populated_fields(node: &Node) -> usize {
    let mut count = 0;

    if node.priority.is_some() {
        count += 1;
    }
    if node.stability.is_some() {
        count += 1;
    }
    if node.confidence.is_some() {
        count += 1;
    }
    if node.status.is_some() {
        count += 1;
    }
    if node.depends.is_some() {
        count += 1;
    }
    if node.related_to.is_some() {
        count += 1;
    }
    if node.replaces.is_some() {
        count += 1;
    }
    if node.conflicts.is_some() {
        count += 1;
    }
    if node.see_also.is_some() {
        count += 1;
    }
    if node.items.is_some() {
        count += 1;
    }
    if node.steps.is_some() {
        count += 1;
    }
    if node.fields.is_some() {
        count += 1;
    }
    if node.input.is_some() {
        count += 1;
    }
    if node.output.is_some() {
        count += 1;
    }
    if node.detail.is_some() {
        count += 1;
    }
    if node.rationale.is_some() {
        count += 1;
    }
    if node.tradeoffs.is_some() {
        count += 1;
    }
    if node.resolution.is_some() {
        count += 1;
    }
    if node.examples.is_some() {
        count += 1;
    }
    if node.notes.is_some() {
        count += 1;
    }
    if node.code.is_some() {
        count += 1;
    }
    if node.code_blocks.is_some() {
        count += 1;
    }
    if node.verify.is_some() {
        count += 1;
    }
    if node.agent_context.is_some() {
        count += 1;
    }
    if node.target.is_some() {
        count += 1;
    }
    if node.execution_status.is_some() {
        count += 1;
    }
    if node.executed_by.is_some() {
        count += 1;
    }
    if node.executed_at.is_some() {
        count += 1;
    }
    if node.execution_log.is_some() {
        count += 1;
    }
    if node.retry_count.is_some() {
        count += 1;
    }
    if node.parallel_groups.is_some() {
        count += 1;
    }
    if node.memory.is_some() {
        count += 1;
    }
    if node.scope.is_some() {
        count += 1;
    }
    if node.applies_when.is_some() {
        count += 1;
    }
    if node.valid_from.is_some() {
        count += 1;
    }
    if node.valid_until.is_some() {
        count += 1;
    }
    if node.tags.is_some() {
        count += 1;
    }
    if node.aliases.is_some() {
        count += 1;
    }
    if node.keywords.is_some() {
        count += 1;
    }
    count += node.extra_fields.len();

    count
}
