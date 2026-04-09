//! CLI integration tests — execution state lifecycle, scheduler, memory, and end-to-end flows.
//! Covers steps 25.3 through 25.6.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").unwrap()
}

/// Returns an `agm` command with `HOME`/`USERPROFILE` pointed at `home_dir` so
/// that each test gets an isolated `~/.agm/global.mem` and parallel test runs
/// don't race on the shared global memory store.
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
// 25.3 — Execution State Lifecycle
// =============================================================================

#[test]
fn test_run_linear_chain_creates_state_file() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    assert!(
        state_file.exists(),
        "expected .state sidecar at {}",
        state_file.display()
    );

    let content = fs::read_to_string(&state_file).unwrap();
    assert!(
        content.contains("completed"),
        "expected all nodes completed in state file"
    );
    assert!(content.contains("step_a"), "missing step_a in state");
    assert!(content.contains("step_b"), "missing step_b in state");
    assert!(content.contains("step_c"), "missing step_c in state");
}

#[test]
fn test_run_dry_run_does_not_create_state_file() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--dry-run")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    assert!(
        !state_file.exists(),
        ".state file must NOT be created on dry-run"
    );
}

#[test]
fn test_status_after_run_shows_all_completed() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    agm_cmd()
        .arg("status")
        .arg(&agm_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("completed"));
}

#[test]
fn test_status_json_after_run_valid_json() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    let output = agm_cmd()
        .arg("status")
        .arg(&agm_file)
        .arg("--json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "agm status --json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with('{') || trimmed.starts_with('['),
        "expected JSON output, got: {trimmed}"
    );
}

#[test]
fn test_state_list_after_run_shows_nodes() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    agm_cmd()
        .arg("state")
        .arg("list")
        .arg(&agm_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("step_a"))
        .stdout(predicate::str::contains("completed"));
}

#[test]
fn test_state_get_after_run_shows_node_detail() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    agm_cmd()
        .arg("state")
        .arg("get")
        .arg(&agm_file)
        .arg("--node")
        .arg("step_a")
        .assert()
        .success()
        .stdout(predicate::str::contains("completed"));
}

#[test]
fn test_state_export_json_valid() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    let output = agm_cmd()
        .arg("state")
        .arg("export")
        .arg(&agm_file)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "agm state export --format json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with('{') || trimmed.starts_with('['),
        "expected JSON output, got: {trimmed}"
    );
}

#[test]
fn test_state_reset_then_rerun_succeeds() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    // First run
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    // Reset state
    agm_cmd()
        .arg("state")
        .arg("reset")
        .arg(&agm_file)
        .arg("--yes")
        .assert()
        .success();

    // Second run
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

// =============================================================================
// 25.4 — Scheduler Integration
// =============================================================================

#[test]
fn test_run_linear_chain_order_abc() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    let output = agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pos_a = stdout.find("step_a");
    let pos_b = stdout.find("step_b");
    let pos_c = stdout.find("step_c");

    // All three nodes must appear in output
    assert!(pos_a.is_some(), "step_a not found in output");
    assert!(pos_b.is_some(), "step_b not found in output");
    assert!(pos_c.is_some(), "step_c not found in output");

    // They must appear in order
    assert!(
        pos_a.unwrap() < pos_b.unwrap(),
        "step_a must appear before step_b"
    );
    assert!(
        pos_b.unwrap() < pos_c.unwrap(),
        "step_b must appear before step_c"
    );
}

#[test]
fn test_run_parallel_diamond_all_succeed() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/parallel-diamond.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--concurrency")
        .arg("2")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(&state_file).unwrap();
    // All 4 nodes must be completed
    let completed_count = content.matches("completed").count();
    assert!(
        completed_count >= 4,
        "expected at least 4 completed statuses, got {completed_count}"
    );
}

#[test]
fn test_run_parallel_groups_orchestrated() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/parallel-groups.agm", dir.path());

    // Run with --group phase1 only
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--group")
        .arg("phase1")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

#[test]
fn test_run_failure_propagation_exits_nonzero() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/failure-propagation.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure(); // step_b's verify check must fail → non-zero exit
}

#[test]
fn test_run_failure_propagation_state_shows_blocked() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/failure-propagation.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    // Run will fail; we don't assert success here
    let _ = agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .output()
        .unwrap();

    // State file should exist
    assert!(
        state_file.exists(),
        ".state file must exist even after failure"
    );

    let content = fs::read_to_string(&state_file).unwrap();
    // step_a should have run
    assert!(content.contains("step_a"), "step_a missing from state");
    // step_c and step_d depend on step_b which failed — they should be blocked or absent
    if content.contains("step_c") {
        assert!(
            content.contains("blocked")
                || content.contains("failed")
                || content.contains("skipped"),
            "step_c should be blocked/failed/skipped, state was:\n{content}"
        );
    }
    if content.contains("step_d") {
        assert!(
            content.contains("blocked")
                || content.contains("failed")
                || content.contains("skipped"),
            "step_d should be blocked/failed/skipped, state was:\n{content}"
        );
    }
}

#[test]
fn test_run_no_verify_all_complete() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/no-verify.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();
}

// =============================================================================
// 25.5 — Memory Integration
// =============================================================================

#[test]
fn test_run_memory_lifecycle_creates_mem_sidecar() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/memory-lifecycle.agm", dir.path());
    let mem_file = PathBuf::from(format!("{}.mem", agm_file.display()));

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    assert!(
        mem_file.exists(),
        "expected .mem sidecar at {}",
        mem_file.display()
    );
}

#[test]
fn test_mem_list_after_run_shows_entries() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/memory-lifecycle.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    agm_cmd()
        .arg("mem")
        .arg("list")
        .arg(&agm_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("project.db_host"));
}

#[test]
fn test_mem_gc_removes_expired_entries() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/memory-lifecycle.agm", dir.path());

    // Copy the expired.agm.mem fixture to the tempdir as the sidecar for this agm file
    let expired_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("memory")
        .join("expired.agm.mem");
    let mem_dest = PathBuf::from(format!("{}.mem", agm_file.display()));
    fs::copy(&expired_src, &mem_dest).expect("failed to copy expired.agm.mem");

    agm_cmd()
        .arg("mem")
        .arg("gc")
        .arg(&agm_file)
        .assert()
        .success()
        // Output should mention GC activity (removed/expired/gc)
        .stdout(
            predicate::str::contains("GC")
                .or(predicate::str::contains("gc"))
                .or(predicate::str::contains("expired"))
                .or(predicate::str::contains("removed"))
                .or(predicate::str::contains("complete")),
        );
}

#[test]
fn test_mem_list_json_valid() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/memory-lifecycle.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    let output = agm_cmd()
        .arg("mem")
        .arg("list")
        .arg(&agm_file)
        .arg("--json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "agm mem list --json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with('{') || trimmed.starts_with('['),
        "expected JSON output, got: {trimmed}"
    );
}

// =============================================================================
// 25.6 — CLI End-to-End
// =============================================================================

#[test]
fn test_verify_failing_node_exits_nonzero() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/failure-propagation.agm", dir.path());

    // Run first so a state file exists (verify may require prior run context)
    let _ = agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .output()
        .unwrap();

    // Verify step_b — its file_exists check will fail (file doesn't exist)
    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--node")
        .arg("step_b")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

#[test]
fn test_verify_passing_node_exits_0() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/retry-scenario.agm", dir.path());

    // Create the marker file the verify check looks for
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

#[test]
fn test_verify_all_no_checks_exits_nonzero() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/no-verify.agm", dir.path());

    // Nodes with no verify checks: `agm verify --all` should exit non-zero
    // (nothing to verify, or "no verify checks found" is treated as an error)
    agm_cmd()
        .arg("verify")
        .arg(&agm_file)
        .arg("--all")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

#[test]
fn test_retry_failed_node_succeeds_after_fix() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/retry-scenario.agm", dir.path());

    // First run — fails because retry_marker.txt doesn't exist
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();

    // Fix: create the file that the verify check requires
    fs::write(dir.path().join("retry_marker.txt"), "present").unwrap();

    // Retry the failed node — should now succeed
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

#[test]
fn test_context_node_produces_output() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    let output = agm_cmd()
        .arg("context")
        .arg(&agm_file)
        .arg("--node")
        .arg("step_b")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "agm context failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.stdout.is_empty(),
        "agm context --node step_b produced no output"
    );
}

#[test]
fn test_context_json_has_token_estimate() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    let output = agm_cmd()
        .arg("context")
        .arg(&agm_file)
        .arg("--node")
        .arg("step_b")
        .arg("--json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "agm context --json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // The JSON output must contain "token" somewhere (token_estimate, token_count, etc.)
    assert!(
        stdout.contains("token"),
        "expected 'token' in JSON output, got: {stdout}"
    );
}

#[test]
fn test_state_import_from_fixture() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());

    // Copy the completed state fixture to the tempdir under a distinct name
    let state_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("state")
        .join("completed.agm.state");
    let state_dest = dir.path().join("import_source.agm.state");
    fs::copy(&state_src, &state_dest).expect("failed to copy completed.agm.state");

    agm_cmd()
        .arg("state")
        .arg("import")
        .arg(&agm_file)
        .arg("--from")
        .arg(&state_dest)
        .assert()
        .success();
}

#[test]
fn test_run_fail_fast_stops_early() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/failure-propagation.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--fail-fast")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .failure();
}

#[test]
fn test_run_single_node_executes_only_target() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/linear-chain.agm", dir.path());
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--node")
        .arg("step_a")
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(&state_file).unwrap();
    // step_a must be completed
    assert!(content.contains("step_a"), "step_a missing from state");

    // step_b and step_c must either be absent or not completed
    if content.contains("step_b") {
        assert!(
            !content.contains("step_b\nexecution_status: completed")
                && !content.contains("step_b\r\nexecution_status: completed"),
            "step_b should not be completed when only step_a was targeted"
        );
    }
    if content.contains("step_c") {
        assert!(
            !content.contains("step_c\nexecution_status: completed")
                && !content.contains("step_c\r\nexecution_status: completed"),
            "step_c should not be completed when only step_a was targeted"
        );
    }
}
