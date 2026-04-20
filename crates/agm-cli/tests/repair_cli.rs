//! CLI end-to-end tests for `agm repair` and `agm fix` subcommands.

use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;

/// Path to a repair fixture.
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("agm-core/tests/fixtures/repair")
        .join(name)
}

/// Write a temp fixture and return its path and a cleanup guard.
fn write_temp(name: &str, content: &str) -> (PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("write temp");
    (path, dir)
}

// ---------------------------------------------------------------------------
// agm repair basic
// ---------------------------------------------------------------------------

#[test]
fn test_repair_stdout_emits_repaired_text() {
    let path = fixture_path("wrapped_fence.agm");
    let mut cmd = Command::cargo_bin("agm").unwrap();
    let assert = cmd.arg("repair").arg(&path).assert();
    assert
        .success()
        .stdout(predicates::str::contains("agm: 1.0"))
        .stdout(predicates::str::contains("```").not());
}

#[test]
fn test_repair_help_shows_flags() {
    let mut cmd = Command::cargo_bin("agm").unwrap();
    let assert = cmd.arg("repair").arg("--help").assert();
    assert
        .success()
        .stdout(predicates::str::contains("--in-place"))
        .stdout(predicates::str::contains("--explain"))
        .stdout(predicates::str::contains("--disable-rule"))
        .stdout(predicates::str::contains("--enable-only"))
        .stdout(predicates::str::contains("--no-safety-net"))
        .stdout(predicates::str::contains("--check"));
}

#[test]
fn test_fix_help_shows_combined_flags() {
    let mut cmd = Command::cargo_bin("agm").unwrap();
    let assert = cmd.arg("fix").arg("--help").assert();
    assert
        .success()
        // repair flags
        .stdout(predicates::str::contains("--in-place"))
        .stdout(predicates::str::contains("--explain"))
        .stdout(predicates::str::contains("--disable-rule"))
        // normalize flags
        .stdout(predicates::str::contains("--no-types"))
        .stdout(predicates::str::contains("--no-fields"));
}

// ---------------------------------------------------------------------------
// agm repair --check
// ---------------------------------------------------------------------------

#[test]
fn test_repair_check_exits_1_when_rewrites_needed() {
    let path = fixture_path("wrapped_fence.agm");
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("repair").arg("--check").arg(&path).assert().code(1);
}

#[test]
fn test_repair_check_exits_0_on_clean_file() {
    let path = fixture_path("already_clean.agm");
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("repair").arg("--check").arg(&path).assert().code(0);
}

// ---------------------------------------------------------------------------
// agm repair --explain
// ---------------------------------------------------------------------------

#[test]
fn test_repair_explain_shows_report_on_stderr() {
    let path = fixture_path("smart_quotes.agm");
    let mut cmd = Command::cargo_bin("agm").unwrap();
    let assert = cmd.arg("repair").arg("--explain").arg(&path).assert();
    // --explain output goes to stderr (per D7)
    assert
        .success()
        .stderr(predicates::str::contains("Repair report:"));
}

// ---------------------------------------------------------------------------
// agm repair --no-safety-net
// ---------------------------------------------------------------------------

#[test]
fn test_repair_no_safety_net_keeps_repaired_text_even_if_invalid() {
    let path = fixture_path("safety_net_triggers.agm");
    // With safety net: should exit 2 (rollback)
    let mut cmd_with_net = Command::cargo_bin("agm").unwrap();
    cmd_with_net.arg("repair").arg(&path).assert().code(2); // rolled back

    // Without safety net: should succeed (exit 0) and emit the stripped content
    let mut cmd_no_net = Command::cargo_bin("agm").unwrap();
    cmd_no_net
        .arg("repair")
        .arg("--no-safety-net")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicates::str::contains("```").not());
}

// ---------------------------------------------------------------------------
// agm repair --output
// ---------------------------------------------------------------------------

#[test]
fn test_repair_output_writes_to_file() {
    let input_path = fixture_path("crlf.agm");
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("out.agm");

    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("repair")
        .arg("--output")
        .arg(&out_path)
        .arg(&input_path)
        .assert()
        .success();

    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(!content.contains('\r'), "output should have no CRLF");
}

// ---------------------------------------------------------------------------
// agm repair --in-place
// ---------------------------------------------------------------------------

#[test]
fn test_repair_in_place_rewrites_file() {
    let input_content = "```agm\nagm: 1.0\npackage: test\nversion: 0.1.0\n\nnode n\ntype: facts\nsummary: test\n```\n";
    let (path, _dir) = write_temp("in_place.agm", input_content);

    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("repair")
        .arg("--in-place")
        .arg(&path)
        .assert()
        .success();

    let result = std::fs::read_to_string(&path).unwrap();
    assert!(
        !result.starts_with("```"),
        "fence should be stripped in-place"
    );
    assert!(result.contains("agm: 1.0"), "content should be preserved");
}

// ---------------------------------------------------------------------------
// agm repair unknown rule ID
// ---------------------------------------------------------------------------

#[test]
fn test_repair_unknown_disable_rule_exits_2() {
    let path = fixture_path("already_clean.agm");
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("repair")
        .arg("--disable-rule")
        .arg("R-NONEXISTENT")
        .arg(&path)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unknown rule id"));
}

#[test]
fn test_repair_unknown_enable_only_exits_2() {
    let path = fixture_path("already_clean.agm");
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("repair")
        .arg("--enable-only")
        .arg("R-NONEXISTENT")
        .arg(&path)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unknown rule id"));
}

// ---------------------------------------------------------------------------
// agm fix basic (repair + normalize)
// ---------------------------------------------------------------------------

#[test]
fn test_fix_repairs_and_normalizes() {
    // Create a file that needs both repair (CRLF) and normalize (depends_on -> depends).
    let content = "agm: 1.0\r\npackage: fix.test\r\nversion: 0.1.0\r\n\r\nnode n1\r\ntype: facts\r\nsummary: N1\r\n\r\nnode n2\r\ntype: facts\r\nsummary: N2\r\ndepends: [n1]\r\n";
    let (path, _dir) = write_temp("fix_input.agm", content);

    let mut cmd = Command::cargo_bin("agm").unwrap();
    let assert = cmd.arg("fix").arg(&path).assert();
    assert
        .success()
        .stdout(predicates::str::contains("agm: 1.0"))
        .stdout(predicates::str::contains('\r').not()); // CRLF fixed
}

#[test]
fn test_fix_check_exits_1_when_either_stage_has_rewrites() {
    let path = fixture_path("crlf.agm");
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("fix").arg("--check").arg(&path).assert().code(1);
}

#[test]
fn test_fix_explain_shows_both_stage_reports() {
    let content = "agm: 1.0\r\npackage: explain.test\r\nversion: 0.1.0\r\n\r\nnode n1\r\ntype: facts\r\nsummary: test\r\n";
    let (path, _dir) = write_temp("fix_explain.agm", content);

    let mut cmd = Command::cargo_bin("agm").unwrap();
    let assert = cmd.arg("fix").arg("--explain").arg(&path).assert();
    assert
        .success()
        .stderr(predicates::str::contains("Repair stage:"))
        .stderr(predicates::str::contains("Normalize stage:"));
}
