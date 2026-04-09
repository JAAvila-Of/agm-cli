//! End-to-end CLI tests using `assert_cmd`.
//!
//! Fixture paths are relative to the workspace root's `tests/fixtures/` directory.
//!
//! - Valid AGM files (agm: 1.0 format) are in `json/forward/`
//! - Invalid files for parse errors are in `parse/invalid/`
//! - Invalid files for validation errors are in `validate/invalid/`
//! - Graph fixtures are in `graph/`

use assert_cmd::Command;
use predicates::prelude::*;

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").unwrap()
}

/// Returns an absolute path to a fixture file given a path relative to the
/// workspace root's `tests/fixtures/` directory.
fn fixture(relative: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    path.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// Validate tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_valid_file_exits_zero() {
    agm_cmd()
        .arg("validate")
        .arg(fixture("json/forward/auth_platform.agm"))
        .assert()
        .success()
        .stderr(predicate::str::contains("OK"));
}

#[test]
fn test_validate_missing_header_exits_one_with_p001() {
    agm_cmd()
        .arg("validate")
        .arg(fixture("parse/invalid/missing_header_agm.agm"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("AGM-P001"));
}

#[test]
fn test_validate_strict_enforcement_disallowed_field_exits_one() {
    agm_cmd()
        .arg("validate")
        .arg("--enforcement")
        .arg("strict")
        .arg(fixture("validate/invalid/disallowed_field_strict.agm"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("AGM-V016"));
}

#[test]
fn test_validate_errors_format_json_outputs_valid_json() {
    let output = agm_cmd()
        .arg("validate")
        .arg("--errors-format")
        .arg("json")
        .arg(fixture("parse/invalid/missing_header_agm.agm"))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(parsed.is_array(), "expected JSON array");
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "expected at least one error");
    assert!(
        arr[0]["code"].as_str().unwrap().starts_with("AGM-"),
        "expected AGM- prefixed code"
    );
}

// ---------------------------------------------------------------------------
// Lint tests
// ---------------------------------------------------------------------------

#[test]
fn test_lint_valid_file_exits_zero() {
    agm_cmd()
        .arg("lint")
        .arg(fixture("json/forward/minimal.agm"))
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Load tests
// ---------------------------------------------------------------------------

#[test]
fn test_load_summary_mode_outputs_json() {
    agm_cmd()
        .arg("load")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--mode")
        .arg("summary")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"nodes\"").or(predicate::str::contains("\"node\"")));
}

#[test]
fn test_load_invalid_mode_exits_with_error() {
    agm_cmd()
        .arg("load")
        .arg(fixture("json/forward/minimal.agm"))
        .arg("--mode")
        .arg("invalid_mode")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("invalid load mode"));
}

// ---------------------------------------------------------------------------
// Render tests
// ---------------------------------------------------------------------------

#[test]
fn test_render_json_outputs_valid_json() {
    let output = agm_cmd()
        .arg("render")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _: serde_json::Value =
        serde_json::from_str(&stdout).expect("render json output should be valid JSON");
}

#[test]
fn test_render_json_canonical_outputs_valid_json() {
    let output = agm_cmd()
        .arg("render")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--format")
        .arg("json-canonical")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _: serde_json::Value =
        serde_json::from_str(&stdout).expect("render json-canonical output should be valid JSON");
}

#[test]
fn test_render_markdown_outputs_markdown() {
    agm_cmd()
        .arg("render")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--format")
        .arg("markdown")
        .assert()
        .success()
        .stdout(predicate::str::contains("#")); // Markdown headers
}

// ---------------------------------------------------------------------------
// Graph tests
// ---------------------------------------------------------------------------

#[test]
fn test_graph_default_outputs_dot() {
    agm_cmd()
        .arg("graph")
        .arg(fixture("graph/auth_platform.agm"))
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph"));
}

#[test]
fn test_graph_topo_outputs_node_ids() {
    agm_cmd()
        .arg("graph")
        .arg(fixture("graph/auth_platform.agm"))
        .arg("--topo")
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Help / edge-case tests
// ---------------------------------------------------------------------------

#[test]
fn test_help_prints_help() {
    agm_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("lint"))
        .stdout(predicate::str::contains("load"))
        .stdout(predicate::str::contains("render"))
        .stdout(predicate::str::contains("graph"));
}

#[test]
fn test_validate_nonexistent_file_exits_two() {
    agm_cmd()
        .arg("validate")
        .arg("nonexistent.agm")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot read"));
}
