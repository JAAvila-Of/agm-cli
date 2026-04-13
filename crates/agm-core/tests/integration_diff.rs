//! Integration tests for agm-core::diff using fixture `.agm` files.

use agm_core::diff::render::{DiffFormat, render_diff};
use agm_core::diff::{self, ChangeKind, ChangeSeverity};
use agm_core::parser;

fn fixture(relative: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

fn parse_fixture(relative: &str) -> agm_core::model::file::AgmFile {
    let src = fixture(relative);
    parser::parse(&src).expect("fixture should parse successfully")
}

// ---------------------------------------------------------------------------
// Core diff tests
// ---------------------------------------------------------------------------

#[test]
fn test_diff_identical_files_returns_empty() {
    let file = parse_fixture("diff/identical/base.agm");
    let report = diff::diff(&file, &file);
    assert!(
        report.is_empty(),
        "identical files should produce empty diff"
    );
    assert_eq!(report.summary.nodes_unchanged, 2);
}

#[test]
fn test_diff_added_node_detected() {
    let left = parse_fixture("diff/added_node/left.agm");
    let right = parse_fixture("diff/added_node/right.agm");
    let report = diff::diff(&left, &right);
    assert_eq!(report.added_nodes.len(), 1);
    assert_eq!(report.added_nodes[0], "test.node.c");
    assert!(report.removed_nodes.is_empty());
    assert!(
        !report.has_breaking_changes(),
        "adding a node is not breaking"
    );
}

#[test]
fn test_diff_removed_node_is_breaking() {
    let left = parse_fixture("diff/removed_node/left.agm");
    let right = parse_fixture("diff/removed_node/right.agm");
    let report = diff::diff(&left, &right);
    assert_eq!(report.removed_nodes.len(), 1);
    assert_eq!(report.removed_nodes[0], "test.node.b");
    assert!(
        report.has_breaking_changes(),
        "removing a node must be breaking"
    );
}

#[test]
fn test_diff_modified_fields_classified_correctly() {
    let left = parse_fixture("diff/modified_fields/left.agm");
    let right = parse_fixture("diff/modified_fields/right.agm");
    let report = diff::diff(&left, &right);

    assert!(
        !report.modified_nodes.is_empty(),
        "should have modified nodes"
    );

    // Find test.node.a which has summary, priority and items changes
    let node_a = report
        .modified_nodes
        .iter()
        .find(|nd| nd.node_id == "test.node.a")
        .expect("test.node.a should be modified");

    // summary change -> Minor
    let summary_change = node_a
        .field_changes
        .iter()
        .find(|fc| fc.field == "summary")
        .expect("summary should have a change");
    assert_eq!(summary_change.severity, ChangeSeverity::Minor);

    // priority change -> Minor
    let priority_change = node_a
        .field_changes
        .iter()
        .find(|fc| fc.field == "priority")
        .expect("priority should have a change");
    assert_eq!(priority_change.severity, ChangeSeverity::Minor);
}

#[test]
fn test_diff_breaking_type_change_detected() {
    let left = parse_fixture("diff/breaking_change/left.agm");
    let right = parse_fixture("diff/breaking_change/right.agm");
    let report = diff::diff(&left, &right);

    assert!(report.has_breaking_changes());

    let login = report
        .modified_nodes
        .iter()
        .find(|nd| nd.node_id == "auth.login")
        .expect("auth.login should be modified");

    assert!(login.has_breaking_change);

    // type change -> Breaking
    let type_change = login
        .field_changes
        .iter()
        .find(|fc| fc.field == "type")
        .expect("type field should have changed");
    assert_eq!(type_change.severity, ChangeSeverity::Breaking);
    assert_eq!(type_change.kind, ChangeKind::Modified);
}

#[test]
fn test_diff_header_changes_detected() {
    let left = parse_fixture("diff/header_change/left.agm");
    let right = parse_fixture("diff/header_change/right.agm");
    let report = diff::diff(&left, &right);

    assert_eq!(
        report.header_changes.len(),
        3,
        "version, title, status should change"
    );

    let version_change = report
        .header_changes
        .iter()
        .find(|hc| hc.field == "version")
        .expect("version should have changed");
    assert_eq!(version_change.severity, ChangeSeverity::Info);

    let title_change = report
        .header_changes
        .iter()
        .find(|hc| hc.field == "title")
        .expect("title should have been added");
    assert_eq!(title_change.kind, ChangeKind::Added);
    assert_eq!(title_change.severity, ChangeSeverity::Info);

    let status_change = report
        .header_changes
        .iter()
        .find(|hc| hc.field == "status")
        .expect("status should have changed");
    assert_eq!(status_change.severity, ChangeSeverity::Minor);
}

#[test]
fn test_diff_complex_fields_detected() {
    let left = parse_fixture("diff/complex_fields/left.agm");
    let right = parse_fixture("diff/complex_fields/right.agm");
    let report = diff::diff(&left, &right);

    let executor = report
        .modified_nodes
        .iter()
        .find(|nd| nd.node_id == "test.executor")
        .expect("test.executor should be modified");

    // target field should have changed
    let target_change = executor
        .field_changes
        .iter()
        .find(|fc| fc.field == "target");
    assert!(target_change.is_some(), "target should have changed");

    // code field should have changed
    let code_change = executor.field_changes.iter().find(|fc| fc.field == "code");
    assert!(code_change.is_some(), "code should have changed");
}

// ---------------------------------------------------------------------------
// Snapshot tests for rendered output
// ---------------------------------------------------------------------------

#[test]
fn test_diff_render_text_snapshot() {
    let left = parse_fixture("diff/breaking_change/left.agm");
    let right = parse_fixture("diff/breaking_change/right.agm");
    let report = diff::diff(&left, &right);
    let output = render_diff(&report, DiffFormat::Text);
    insta::assert_snapshot!(output);
}

#[test]
fn test_diff_render_json_snapshot() {
    let left = parse_fixture("diff/modified_fields/left.agm");
    let right = parse_fixture("diff/modified_fields/right.agm");
    let report = diff::diff(&left, &right);
    let output = render_diff(&report, DiffFormat::Json);
    // Verify it's valid JSON and round-trips
    let back: agm_core::diff::DiffReport =
        serde_json::from_str(&output).expect("JSON output should deserialize back to DiffReport");
    assert_eq!(report, back);
    insta::assert_snapshot!(output);
}

#[test]
fn test_diff_render_markdown_snapshot() {
    let left = parse_fixture("diff/header_change/left.agm");
    let right = parse_fixture("diff/header_change/right.agm");
    let report = diff::diff(&left, &right);
    let output = render_diff(&report, DiffFormat::Markdown);
    insta::assert_snapshot!(output);
}

#[test]
fn test_diff_breaking_only_filter() {
    let left = parse_fixture("diff/breaking_change/left.agm");
    let right = parse_fixture("diff/breaking_change/right.agm");
    let full_report = diff::diff(&left, &right);
    let breaking_report = full_report.breaking_only();

    // The breaking_only report should have no added nodes
    assert!(breaking_report.added_nodes.is_empty());

    // Every field change in modified nodes must be Breaking
    for nd in &breaking_report.modified_nodes {
        for fc in &nd.field_changes {
            assert_eq!(
                fc.severity,
                ChangeSeverity::Breaking,
                "breaking_only should filter non-breaking field changes"
            );
        }
    }
}
