//! End-to-end CLI tests for `agm normalize`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").unwrap()
}

fn normalize_fixture_path(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .join("../..")
        .join("crates/agm-core/tests/fixtures/normalize")
        .join(name);
    path.to_string_lossy().into_owned()
}

fn write_temp_agm(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::with_suffix(".agm").unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

// ---------------------------------------------------------------------------
// Basic normalize — rewrites and exits 0
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_non_canonical_exits_0_and_outputs_canonical() {
    let path = normalize_fixture_path("depends_on_universal.agm");
    agm_cmd()
        .args(["normalize", &path])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("depends:"))
        .stdout(predicate::str::contains("depends_on:").not());
}

// ---------------------------------------------------------------------------
// --check on non-canonical → exit 1, empty stdout
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_check_non_canonical_exits_1() {
    let path = normalize_fixture_path("depends_on_universal.agm");
    agm_cmd()
        .args(["normalize", "--check", &path])
        .assert()
        .failure()
        .code(1)
        .stdout("");
}

// ---------------------------------------------------------------------------
// --check on canonical → exit 0
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_check_canonical_exits_0() {
    let path = normalize_fixture_path("already_canonical.agm");
    agm_cmd()
        .args(["normalize", "--check", &path])
        .assert()
        .success()
        .code(0);
}

// ---------------------------------------------------------------------------
// --explain --report-format json → stderr contains parseable JSON
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_explain_json_report_on_stderr() {
    let path = normalize_fixture_path("depends_on_universal.agm");
    let output = agm_cmd()
        .args(["normalize", "--explain", "--report-format", "json", &path])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should be valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&stderr).expect("stderr should be valid JSON");
    assert!(
        parsed.get("rewrites").is_some(),
        "JSON should have 'rewrites' key"
    );
}

// ---------------------------------------------------------------------------
// Missing file → exit 2
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_missing_file_exits_2() {
    agm_cmd()
        .args(["normalize", "this_file_does_not_exist.agm"])
        .assert()
        .failure()
        .code(2);
}

// ---------------------------------------------------------------------------
// Bad rule file → exit 3
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_bad_rules_file_exits_3() {
    let path = normalize_fixture_path("already_canonical.agm");
    let mut rules_file = NamedTempFile::with_suffix(".yaml").unwrap();
    rules_file.write_all(b"{ invalid yaml: [unclosed").unwrap();

    agm_cmd()
        .args([
            "normalize",
            "--rules",
            rules_file.path().to_str().unwrap(),
            &path,
        ])
        .assert()
        .failure()
        .code(3);
}

// ---------------------------------------------------------------------------
// --in-place rewrites the file, no backup created
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_in_place_rewrites_file() {
    let non_canonical = "\
agm: 1.0\npackage: test.inplace\nversion: 0.1.0\n\n\
node n1\ntype: workflow\nsummary: a node\ndepends_on: [dep.node]\n";

    let tmp = write_temp_agm(non_canonical);
    let path = tmp.path().to_str().unwrap();

    agm_cmd()
        .args(["normalize", "--in-place", path])
        .assert()
        .success()
        .code(0);

    let result = std::fs::read_to_string(path).unwrap();
    assert!(
        result.contains("depends:"),
        "in-place file should use canonical 'depends:'"
    );
    assert!(
        !result.contains("depends_on:"),
        "in-place file should not have synonym"
    );

    // No .bak file created
    let bak = format!("{path}.bak");
    assert!(
        !std::path::Path::new(&bak).exists(),
        "no .bak file should be created"
    );
}

// ---------------------------------------------------------------------------
// --output writes to separate file
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_output_flag_writes_to_file() {
    let path = normalize_fixture_path("depends_on_universal.agm");
    let out_file = NamedTempFile::with_suffix(".agm").unwrap();
    let out_path = out_file.path().to_str().unwrap();

    agm_cmd()
        .args(["normalize", "--output", out_path, &path])
        .assert()
        .success()
        .code(0);

    let result = std::fs::read_to_string(out_path).unwrap();
    assert!(result.contains("depends:"));
}

// ---------------------------------------------------------------------------
// --no-types disables type rewrite
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_no_types_flag() {
    let path = normalize_fixture_path("plan_execution_synonym.agm");
    agm_cmd()
        .args(["normalize", "--no-types", &path])
        .assert()
        .success()
        .code(0)
        // type should NOT be rewritten
        .stdout(predicate::str::contains("type: plan_execution"));
}

// ---------------------------------------------------------------------------
// --no-fields disables field rewrite
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_no_fields_flag() {
    let path = normalize_fixture_path("depends_on_universal.agm");
    agm_cmd()
        .args(["normalize", "--no-fields", &path])
        .assert()
        .success()
        .code(0)
        // field should NOT be rewritten
        .stdout(predicate::str::contains("depends_on:"));
}

// ---------------------------------------------------------------------------
// --explain in text format shows rewrite summary on stderr
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_explain_text_on_stderr() {
    let path = normalize_fixture_path("depends_on_universal.agm");
    let output = agm_cmd()
        .args(["normalize", "--explain", &path])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Normalize report:"),
        "stderr should contain report header"
    );
}
