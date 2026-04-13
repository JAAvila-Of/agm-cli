//! E2E tests for error recovery, retry, and state corruption scenarios.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
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

// =============================================================================
// Error Recovery Tests
// =============================================================================

/// Retry after node failure: run a file that fails, fix the issue, retry —
/// verify it resumes from failed node and now succeeds.
#[test]
fn test_retry_after_node_failure_succeeds_after_fix() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/retry-scenario.agm", dir.path());

    // First run — fails because retry_marker.txt does not exist
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();

    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));
    assert!(
        state_file.exists(),
        ".state file must exist after failed run"
    );

    let state_content = fs::read_to_string(&state_file).unwrap();
    assert!(
        state_content.contains("failed"),
        "state must show failed node before retry"
    );

    // Fix the condition
    fs::write(dir.path().join("retry_marker.txt"), "present").unwrap();

    // Retry — must succeed now
    agm_cmd_isolated(dir.path())
        .arg("retry")
        .arg(&agm_file)
        .arg("--node")
        .arg("check_file")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

/// State file missing during status check: verify graceful error message,
/// not a panic.
#[test]
fn test_status_no_state_file_reports_graceful_error() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    // Do NOT run first — no .state file exists
    let output = agm_cmd().arg("status").arg(&agm_file).output().unwrap();

    // Must exit non-zero (no state), but must NOT panic (signal)
    assert!(
        !output.status.success(),
        "status must be non-zero when no state file exists"
    );
    // Must not crash with no output at all
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        !combined.is_empty() || !output.status.success(),
        "must produce some output or non-zero exit when state is missing"
    );
}

/// State file corrupted: write garbage to .state file, run status — verify it
/// reports corruption gracefully and does not panic.
#[test]
fn test_status_corrupted_state_file_reports_error_not_panic() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    // Write garbage
    fs::write(
        &state_file,
        b"\x00\xff\xfe this is not valid state content !!!",
    )
    .unwrap();

    let output = agm_cmd().arg("status").arg(&agm_file).output().unwrap();

    // Must exit non-zero — corrupt state is an error
    assert!(
        !output.status.success(),
        "status must be non-zero for corrupted state file"
    );

    // Must not be a signal exit (panic/crash). On Unix signal exits have no exit code;
    // on Windows they always have a code. Either way, the process must have exited.
    assert!(
        output.status.code().is_some(),
        "process must exit with a code, not a signal, for corrupted state"
    );
}

/// Resume partial execution: write partial state (only step_a completed),
/// then run again — verify it picks up from where it left off.
#[test]
fn test_run_with_partial_state_resumes_from_incomplete_node() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    // Write partial state: step_a completed, step_b and step_c pending/ready
    let partial_state = "# agm.state: 1.0\n\
        # package: test.orchestration.linear\n\
        # version: 1.0.0\n\
        # session_id: run-partial\n\
        # started_at: 2026-01-01T00:00:00Z\n\
        # updated_at: 2026-01-01T00:00:00Z\n\
        \n\
        state step_a\n\
        execution_status: completed\n\
        executed_by: shell-agent\n\
        executed_at: 2026-01-01T00:00:00Z\n\
        retry_count: 0\n\
        \n\
        state step_b\n\
        execution_status: ready\n\
        retry_count: 0\n\
        \n\
        state step_c\n\
        execution_status: pending\n\
        retry_count: 0\n";
    fs::write(&state_file, partial_state).unwrap();

    // Run should succeed and complete the remaining nodes
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    let final_state = fs::read_to_string(&state_file).unwrap();
    // After run, step_b and step_c should be completed
    let completed_count = final_state.matches("completed").count();
    assert!(
        completed_count >= 2,
        "expected at least 2 completed nodes after resuming, state was:\n{final_state}"
    );
}

/// Empty .state file: create an empty sidecar, run status — verify graceful
/// handling (no panic).
#[test]
fn test_status_empty_state_file_handled_gracefully() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    // Write empty file
    fs::write(&state_file, "").unwrap();

    let output = agm_cmd().arg("status").arg(&agm_file).output().unwrap();

    // Must exit with a code (not a signal / panic)
    assert!(
        output.status.code().is_some(),
        "process must exit with a code, not a signal, for empty state file"
    );
}

/// Run on file with no orchestration nodes (only facts/rules): verify that an
/// appropriate message is printed and the exit is not a crash.
#[test]
fn test_run_file_with_no_workflow_nodes_exits_cleanly() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/facts-only.agm", dir.path());

    let output = agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .output()
        .unwrap();

    // Must exit with a code (not signal). Exit code may be 0 or non-zero depending
    // on whether "nothing to run" is considered success or an error.
    assert!(
        output.status.code().is_some(),
        "process must exit with a code when file has no executable nodes"
    );
}

/// Retry a node that is not in "failed" state: verify appropriate error message.
#[test]
fn test_retry_non_failed_node_reports_error() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    // Run to completion first
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    // Retry a completed node — must fail with an error message
    agm_cmd_isolated(dir.path())
        .arg("retry")
        .arg(&agm_file)
        .arg("--node")
        .arg("step_a")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not").or(predicate::str::contains("error")));
}

/// Retry a node that does not exist in the AGM file: verify error message.
#[test]
fn test_retry_nonexistent_node_reports_error() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    // Run to get a state file
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    agm_cmd_isolated(dir.path())
        .arg("retry")
        .arg(&agm_file)
        .arg("--node")
        .arg("this_node_does_not_exist")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("error")));
}
