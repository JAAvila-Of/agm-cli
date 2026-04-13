//! E2E tests for the `verify` command.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tempfile::tempdir;

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").unwrap()
}

fn agm_cmd_isolated(home_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.env("HOME", home_dir).env("USERPROFILE", home_dir);
    cmd
}

fn copy_fixture(fixture_relative: &str, dir: &Path) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(fixture_relative);
    let filename = src.file_name().unwrap();
    let dest = dir.join(filename);
    fs::copy(&src, &dest)
        .unwrap_or_else(|e| panic!("Failed to copy fixture {}: {e}", src.display()));
    dest
}

/// Creates an AGM fixture file with a single workflow node that has a
/// `file_exists` verify check.
fn write_file_exists_agm(dir: &TempDir, pkg: &str, node_id: &str, check_file: &str) -> PathBuf {
    let content = format!(
        "agm: 1.0\npackage: {pkg}\nversion: 1.0.0\n\nnode {node_id}\ntype: workflow\nsummary: Verify file_exists check\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n  - type: file_exists\n    file: {check_file}\n"
    );
    let path = dir.path().join(format!("{pkg}.agm"));
    fs::write(&path, content).unwrap();
    path
}

/// Creates an AGM fixture file with a single workflow node that has a
/// `file_contains` verify check.
fn write_file_contains_agm(
    dir: &TempDir,
    pkg: &str,
    node_id: &str,
    check_file: &str,
    pattern: &str,
) -> PathBuf {
    let content = format!(
        "agm: 1.0\npackage: {pkg}\nversion: 1.0.0\n\nnode {node_id}\ntype: workflow\nsummary: Verify file_contains check\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n  - type: file_contains\n    file: {check_file}\n    pattern: {pattern}\n"
    );
    let path = dir.path().join(format!("{pkg}.agm"));
    fs::write(&path, content).unwrap();
    path
}

// =============================================================================
// Verifier E2E Tests
// =============================================================================

/// Verify file_exists check passes when the file is present.
#[test]
fn test_verify_file_exists_check_passes_when_file_present() {
    let dir = tempdir().unwrap();
    let agm_file = write_file_exists_agm(
        &dir,
        "test.verify.fepresent",
        "check_present",
        "expected.txt",
    );
    fs::write(dir.path().join("expected.txt"), "content").unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("check_present")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// Verify file_exists check fails when the file is absent.
#[test]
fn test_verify_file_exists_check_fails_when_file_absent() {
    let dir = tempdir().unwrap();
    let agm_file = write_file_exists_agm(
        &dir,
        "test.verify.feabsent",
        "check_absent",
        "absolutely_missing.txt",
    );
    // Do NOT create the file

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("check_absent")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

/// Verify file_contains check passes when pattern is found in the file.
#[test]
fn test_verify_file_contains_check_passes_when_pattern_found() {
    let dir = tempdir().unwrap();
    let agm_file = write_file_contains_agm(
        &dir,
        "test.verify.fcpresent",
        "check_content",
        "output.txt",
        "SUCCESS",
    );
    fs::write(dir.path().join("output.txt"), "Operation SUCCESS\n").unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("check_content")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// Verify file_contains check fails when pattern is not found.
#[test]
fn test_verify_file_contains_check_fails_when_pattern_absent() {
    let dir = tempdir().unwrap();
    let agm_file = write_file_contains_agm(
        &dir,
        "test.verify.fcabsent",
        "check_content_fail",
        "output.txt",
        "EXPECTED",
    );
    fs::write(dir.path().join("output.txt"), "This has no match\n").unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("check_content_fail")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

/// Verify all checks pass: multiple verify checks all passing — exits 0.
#[test]
fn test_verify_all_checks_pass_exits_zero() {
    let dir = tempdir().unwrap();

    // Node with two verify checks both expected to pass
    let content = "agm: 1.0\npackage: test.verify.allpass\nversion: 1.0.0\n\nnode multi_check\ntype: workflow\nsummary: Node with multiple passing checks\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n  - type: file_exists\n    file: data.txt\n  - type: file_contains\n    file: data.txt\n    pattern: hello\n";

    let agm_file = dir.path().join("verify_allpass.agm");
    fs::write(&agm_file, content).unwrap();
    fs::write(dir.path().join("data.txt"), "hello world\n").unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("multi_check")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// Verify mixed results: some checks pass, some fail — verify exits non-zero.
#[test]
fn test_verify_mixed_results_exits_nonzero() {
    let dir = tempdir().unwrap();

    // Two checks: first passes (present.txt exists), second fails (missing.txt absent)
    let content = "agm: 1.0\npackage: test.verify.mixed1\nversion: 1.0.0\n\nnode mixed_checks\ntype: workflow\nsummary: Node with mixed pass/fail checks\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n  - type: file_exists\n    file: present.txt\n  - type: file_exists\n    file: missing.txt\n";

    let agm_file = dir.path().join("verify_mixed.agm");
    fs::write(&agm_file, content).unwrap();
    fs::write(dir.path().join("present.txt"), "here").unwrap();
    // "missing.txt" intentionally not created

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("mixed_checks")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

/// Verify on a non-existent node: exits with an error.
#[test]
fn test_verify_nonexistent_node_reports_error() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("this_node_does_not_exist_at_all")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("error")));
}

/// Verify --all on a file with no verify checks exits non-zero with a message.
#[test]
fn test_verify_all_no_checks_file_exits_nonzero() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/no-verify.agm", dir.path());

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--all")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

/// Verify JSON output with --json flag: verify the output is valid JSON and
/// contains "all_passed".
#[test]
fn test_verify_json_output_is_valid_json() {
    let dir = tempdir().unwrap();
    let agm_file = write_file_exists_agm(&dir, "test.verify.jsonout", "json_node", "marker.txt");
    fs::write(dir.path().join("marker.txt"), "present").unwrap();

    let output = agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("json_node")
        .arg("--json")
        .arg("--working-dir")
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "verify --json must succeed when checks pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("verify --json output must be valid JSON");
    assert!(
        parsed.is_array(),
        "verify --json output must be a JSON array"
    );
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "verify --json array must not be empty");
    assert!(
        arr[0].get("all_passed").is_some(),
        "each result must have 'all_passed' field"
    );
}

/// Verify the existing retry-scenario fixture node passes after creating the marker.
#[test]
fn test_verify_passing_node_from_fixture_exits_zero() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/retry-scenario.agm", dir.path());

    fs::write(dir.path().join("retry_marker.txt"), "present").unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("check_file")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// Verify the existing retry-scenario fixture node fails without the marker.
#[test]
fn test_verify_failing_node_from_fixture_exits_nonzero() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/retry-scenario.agm", dir.path());

    // Do NOT create retry_marker.txt
    agm_cmd_isolated(dir.path())
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("check_file")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

// =============================================================================
// Gap 3 — command verify type E2E tests
// =============================================================================

/// Creates an AGM fixture with a single workflow node that has a `command`
/// verify check. `expect` is written verbatim into the `expect:` field when
/// `Some`, or omitted when `None`.
fn write_command_verify_agm(
    dir: &TempDir,
    pkg: &str,
    node_id: &str,
    command: &str,
    expect: Option<&str>,
) -> PathBuf {
    let expect_line = match expect {
        Some(e) => format!("    expect: {e}\n"),
        None => String::new(),
    };
    let content = format!(
        "agm: 1.0\npackage: {pkg}\nversion: 1.0.0\n\nnode {node_id}\ntype: workflow\nsummary: Command verify check\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n  - type: command\n    run: {command}\n{expect_line}"
    );
    let path = dir.path().join(format!("{pkg}.agm"));
    fs::write(&path, content).unwrap();
    path
}

// ---------------------------------------------------------------------------
// 3.1 command verify type tests
// ---------------------------------------------------------------------------

/// command verify check passes when the command exits 0.
#[test]
fn test_verify_command_check_passes_when_command_succeeds() {
    let dir = tempdir().unwrap();
    // `echo hello` exits 0 on both Unix and Windows
    let agm_file = write_command_verify_agm(
        &dir,
        "test.verify.cmd.success",
        "cmd_pass",
        "echo hello",
        None,
    );

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("cmd_pass")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// command verify check fails when the command exits non-zero.
#[test]
fn test_verify_command_check_fails_when_command_fails() {
    let dir = tempdir().unwrap();
    // `exit 1` is a shell built-in; cmd /C wraps it on Windows, sh -c on Unix
    #[cfg(windows)]
    let fail_cmd = "exit 1";
    #[cfg(not(windows))]
    let fail_cmd = "exit 1";

    let agm_file =
        write_command_verify_agm(&dir, "test.verify.cmd.fail", "cmd_fail", fail_cmd, None);

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("cmd_fail")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

/// command verify check with `output_contains:` passes when stdout matches.
#[test]
fn test_verify_command_check_with_expect_passes_when_output_matches() {
    let dir = tempdir().unwrap();
    // `echo SUCCESS` produces output containing "SUCCESS"
    let agm_file = write_command_verify_agm(
        &dir,
        "test.verify.cmd.expect.pass",
        "cmd_expect_pass",
        "echo SUCCESS",
        Some("output_contains: SUCCESS"),
    );

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("cmd_expect_pass")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// command verify check with `output_contains:` fails when stdout does not match.
#[test]
fn test_verify_command_check_with_expect_fails_when_output_does_not_match() {
    let dir = tempdir().unwrap();
    // `echo WRONG` does not contain "CORRECT"
    let agm_file = write_command_verify_agm(
        &dir,
        "test.verify.cmd.expect.fail",
        "cmd_expect_fail",
        "echo WRONG",
        Some("output_contains: CORRECT"),
    );

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("cmd_expect_fail")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// 3.2 Node with 20+ mixed verify checks
// ---------------------------------------------------------------------------

/// A single node with 20 verify checks (7 file_exists + 7 file_contains +
/// 6 command) all passing — exits 0.
#[test]
fn test_verify_node_with_20_mixed_checks_all_pass() {
    let dir = tempdir().unwrap();

    // Create the 7 files for file_exists and 7 files for file_contains
    for i in 1..=7 {
        fs::write(dir.path().join(format!("fe_{i}.txt")), "present").unwrap();
        fs::write(
            dir.path().join(format!("fc_{i}.txt")),
            format!("MARKER_{i}"),
        )
        .unwrap();
    }

    // Build verify block: 7 file_exists + 7 file_contains + 6 command (echo)
    let mut verify_lines = String::new();
    for i in 1..=7 {
        verify_lines.push_str(&format!("  - type: file_exists\n    file: fe_{i}.txt\n"));
    }
    for i in 1..=7 {
        verify_lines.push_str(&format!(
            "  - type: file_contains\n    file: fc_{i}.txt\n    pattern: MARKER_{i}\n"
        ));
    }
    for i in 1..=6 {
        verify_lines.push_str(&format!("  - type: command\n    run: echo check{i}\n"));
    }

    let content = format!(
        "agm: 1.0\npackage: test.verify.mixed20.pass\nversion: 1.0.0\n\nnode big_node\ntype: workflow\nsummary: Node with 20 mixed passing checks\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n{verify_lines}"
    );
    let agm_file = dir.path().join("mixed20_pass.agm");
    fs::write(&agm_file, content).unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("big_node")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// Same 20-check node but one file_exists references a missing file — exits non-zero.
#[test]
fn test_verify_node_with_20_mixed_checks_one_fails_exits_nonzero() {
    let dir = tempdir().unwrap();

    // Create files 1-6 for file_exists; intentionally omit fe_7.txt
    for i in 1..=6 {
        fs::write(dir.path().join(format!("fe_{i}.txt")), "present").unwrap();
    }
    for i in 1..=7 {
        fs::write(
            dir.path().join(format!("fc_{i}.txt")),
            format!("MARKER_{i}"),
        )
        .unwrap();
    }

    let mut verify_lines = String::new();
    for i in 1..=7 {
        // fe_7.txt is missing — this check will fail
        verify_lines.push_str(&format!("  - type: file_exists\n    file: fe_{i}.txt\n"));
    }
    for i in 1..=7 {
        verify_lines.push_str(&format!(
            "  - type: file_contains\n    file: fc_{i}.txt\n    pattern: MARKER_{i}\n"
        ));
    }
    for i in 1..=6 {
        verify_lines.push_str(&format!("  - type: command\n    run: echo check{i}\n"));
    }

    let content = format!(
        "agm: 1.0\npackage: test.verify.mixed20.fail\nversion: 1.0.0\n\nnode big_node_fail\ntype: workflow\nsummary: Node with 20 mixed checks one failing\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n{verify_lines}"
    );
    let agm_file = dir.path().join("mixed20_fail.agm");
    fs::write(&agm_file, content).unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("big_node_fail")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// 3.3 `verify --all` on a large file
// ---------------------------------------------------------------------------

/// 10 workflow nodes each with 2 passing verify checks — `--all` exits 0.
#[test]
fn test_verify_all_on_10_node_file_all_passing() {
    let dir = tempdir().unwrap();

    // Create marker files for each node's file_exists check
    for i in 1..=10 {
        fs::write(dir.path().join(format!("node{i}_marker.txt")), "ok").unwrap();
    }

    let mut content = "agm: 1.0\npackage: test.verify.all10.pass\nversion: 1.0.0\n\n".to_owned();
    for i in 1..=10 {
        content.push_str(&format!(
            "node workflow_{i}\ntype: workflow\nsummary: Node {i}\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n  - type: file_exists\n    file: node{i}_marker.txt\n  - type: command\n    run: echo ok\n\n"
        ));
    }

    let agm_file = dir.path().join("all10_pass.agm");
    fs::write(&agm_file, &content).unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--all")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// 10 workflow nodes but one node's file_exists check fails — `--all` exits non-zero.
#[test]
fn test_verify_all_on_10_node_file_one_node_fails() {
    let dir = tempdir().unwrap();

    // Create markers for nodes 1-9; intentionally omit node10_marker.txt
    for i in 1..=9 {
        fs::write(dir.path().join(format!("node{i}_marker.txt")), "ok").unwrap();
    }

    let mut content = "agm: 1.0\npackage: test.verify.all10.fail\nversion: 1.0.0\n\n".to_owned();
    for i in 1..=10 {
        content.push_str(&format!(
            "node workflow_{i}\ntype: workflow\nsummary: Node {i}\ncode:\n  lang: sh\n  action: full\n  body: echo done\nverify:\n  - type: file_exists\n    file: node{i}_marker.txt\n  - type: command\n    run: echo ok\n\n"
        ));
    }

    let agm_file = dir.path().join("all10_fail.agm");
    fs::write(&agm_file, &content).unwrap();

    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--all")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}
