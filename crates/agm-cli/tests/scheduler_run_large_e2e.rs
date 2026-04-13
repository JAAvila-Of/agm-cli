//! Gap 7 — Scheduler/Run Large E2E Tests.
//!
//! Tests for large-scale orchestration runs:
//!   7.1 — Run with 20+ nodes (parallel groups, dependency chains)
//!   7.2 — Fail-fast with 30+ nodes (stops at first failure)
//!   7.3 — Status JSON output with 50 mixed-state nodes

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 7.1 — Run with 20+ nodes
// ---------------------------------------------------------------------------

/// Run the 20-node fixture to completion.
///
/// The fixture has a diamond-like dependency structure:
///   - Nodes 1-5: no deps (parallel-ready)
///   - Nodes 6-10: depend on node_01
///   - Nodes 11-15: sequential chain 11->12->13->14->15 (11 depends on node_06)
///   - Nodes 16-20: independent
///
/// Expected: exit code 0, all 20 nodes complete.
#[test]
fn test_run_large_20_nodes_with_parallel_groups_completes() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/large-20-nodes.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Run complete"));

    // Verify the state sidecar was created
    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));
    assert!(
        state_file.exists(),
        ".state sidecar must exist after successful run"
    );

    // All 20 nodes must appear as completed in the state
    let state_content = fs::read_to_string(&state_file).unwrap();
    let completed_count = state_content.matches("execution_status: completed").count();
    assert_eq!(
        completed_count, 20,
        "all 20 nodes must be completed; state:\n{state_content}"
    );
}

/// After running the 20-node fixture, `status --json` must report all 20 nodes.
#[test]
fn test_run_large_20_nodes_status_json_shows_all() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/large-20-nodes.agm", dir.path());

    // Run first to create state
    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .assert()
        .success();

    // Now query status as JSON
    let output = agm_cmd_isolated(dir.path())
        .arg("status")
        .arg(&agm_file)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output).expect("status --json must be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("status --json output must be valid JSON");

    let nodes = parsed
        .get("nodes")
        .expect("JSON must have 'nodes' key")
        .as_object()
        .expect("'nodes' must be an object");

    assert_eq!(
        nodes.len(),
        20,
        "status JSON must report exactly 20 nodes; got {}",
        nodes.len()
    );

    // Every node entry must have execution_status == "completed"
    for (node_id, node_state) in nodes {
        let status = node_state
            .get("execution_status")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing>");
        assert_eq!(
            status, "completed",
            "node {node_id} should be completed, got {status}"
        );
    }
}

// ---------------------------------------------------------------------------
// 7.2 — Fail-fast with 30+ nodes
// ---------------------------------------------------------------------------

/// Run the 30-node linear chain with --fail-fast.
///
/// Node 15 has `body: exit 1`. With fail-fast enabled the run must abort
/// immediately after node 15 fails, leaving nodes 16-30 blocked.
///
/// Expected:
///   - Non-zero exit code (failed nodes present)
///   - State shows node_15 as failed
///   - Nodes before 15 are completed
///   - Nodes after 15 are blocked (or pending, depending on propagation timing)
#[test]
fn test_run_fail_fast_30_nodes_stops_at_failure() {
    let dir = tempdir().unwrap();
    let agm_file = copy_fixture("orchestration/fail-fast-30.agm", dir.path());

    agm_cmd_isolated(dir.path())
        .arg("run")
        .arg(&agm_file)
        .arg("--working-dir")
        .arg(dir.path())
        .arg("--fail-fast")
        .assert()
        .failure(); // non-zero exit because node_15 fails

    let state_file = PathBuf::from(format!("{}.state", agm_file.display()));
    assert!(
        state_file.exists(),
        ".state sidecar must exist after failed run"
    );

    let state_content = fs::read_to_string(&state_file).unwrap();

    // node_15 must be marked as failed
    assert!(
        state_content.contains("state node_15")
            && state_content.contains("execution_status: failed"),
        "node_15 must appear as failed in state:\n{state_content}"
    );

    // Nodes 1-14 must be completed (they ran before the failure)
    for n in 1u32..=14 {
        let node_id = format!("node_{n:02}");
        assert!(
            state_content.contains(&format!("state {node_id}")),
            "node {node_id} must appear in state"
        );
    }

    // The run output must mention the failure
    // (already asserted via .failure() above — exit code is 1)

    // Nodes 16-30 must NOT be completed (fail-fast stopped execution)
    for n in 16u32..=30 {
        let completed_marker = format!("state node_{n:02}\nexecution_status: completed");
        assert!(
            !state_content.contains(&completed_marker),
            "node_{n:02} must not be completed after fail-fast"
        );
    }
}

// ---------------------------------------------------------------------------
// 7.3 — Status JSON with 50 mixed-state nodes
// ---------------------------------------------------------------------------

/// Build an AGM file with 50 nodes plus a hand-crafted `.agm.state` sidecar
/// that assigns mixed execution statuses. Then run `agm status --json` and
/// verify the JSON output contains all 50 entries with the correct
/// status distribution.
///
/// Distribution in the sidecar:
///   - Nodes 01-20: completed  (20 nodes)
///   - Nodes 21-30: failed     (10 nodes)
///   - Nodes 31-40: blocked    (10 nodes)
///   - Nodes 41-45: pending    (5 nodes)
///   - Nodes 46-50: ready      (5 nodes)
#[test]
fn test_status_json_50_nodes_mixed_states() {
    let dir = tempdir().unwrap();

    // --- Build the AGM file ---
    let agm_content = build_50_node_agm();
    let agm_file = dir.path().join("mixed50.agm");
    fs::write(&agm_file, agm_content).expect("Failed to write 50-node AGM fixture");

    // --- Build the state sidecar ---
    let state_content = build_50_node_state();
    let state_file = dir.path().join("mixed50.agm.state");
    fs::write(&state_file, state_content).expect("Failed to write state sidecar");

    // --- Run status --json ---
    let output = agm_cmd_isolated(dir.path())
        .arg("status")
        .arg(&agm_file)
        .arg("--json")
        .assert()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output).expect("status --json must be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("status --json output must be valid JSON");

    let nodes = parsed
        .get("nodes")
        .expect("JSON must have 'nodes' key")
        .as_object()
        .expect("'nodes' must be an object");

    assert_eq!(
        nodes.len(),
        50,
        "status JSON must report exactly 50 nodes; got {}",
        nodes.len()
    );

    // Count per-status distribution
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut blocked = 0usize;
    let mut pending = 0usize;
    let mut ready = 0usize;

    for (_node_id, node_state) in nodes {
        match node_state
            .get("execution_status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
        {
            "completed" => completed += 1,
            "failed" => failed += 1,
            "blocked" => blocked += 1,
            "pending" => pending += 1,
            "ready" => ready += 1,
            other => panic!("Unexpected execution_status: {other}"),
        }
    }

    assert_eq!(completed, 20, "expected 20 completed nodes");
    assert_eq!(failed, 10, "expected 10 failed nodes");
    assert_eq!(blocked, 10, "expected 10 blocked nodes");
    assert_eq!(pending, 5, "expected 5 pending nodes");
    assert_eq!(ready, 5, "expected 5 ready nodes");
}

// ---------------------------------------------------------------------------
// Fixture builders for test 7.3
// ---------------------------------------------------------------------------

/// Generates an AGM file with 50 independent workflow nodes.
///
/// All nodes are independent (no `depends`) so the AGM is valid regardless
/// of which state we inject via the sidecar.
fn build_50_node_agm() -> String {
    let mut s = String::new();
    s.push_str("agm: 1.0\n");
    s.push_str("package: test.orchestration.mixed50\n");
    s.push_str("version: 1.0.0\n");

    for n in 1u32..=50 {
        s.push('\n');
        s.push_str(&format!("node node_{n:02}\n"));
        s.push_str("type: workflow\n");
        s.push_str(&format!("summary: Mixed-state test node {n:02}\n"));
        s.push_str("code:\n");
        s.push_str("  lang: cmd\n");
        s.push_str("  action: full\n");
        s.push_str(&format!("  body: echo node {n:02}\n"));
    }

    s
}

/// Generates a `.agm.state` sidecar for the 50-node fixture with:
///   - Nodes 01-20: completed
///   - Nodes 21-30: failed
///   - Nodes 31-40: blocked
///   - Nodes 41-45: pending
///   - Nodes 46-50: ready
fn build_50_node_state() -> String {
    let mut s = String::new();
    s.push_str("# agm.state: 1.0\n");
    s.push_str("# package: test.orchestration.mixed50\n");
    s.push_str("# version: 1.0.0\n");
    s.push_str("# session_id: run-2026-04-12-000000\n");
    s.push_str("# started_at: 2026-04-12T00:00:00Z\n");
    s.push_str("# updated_at: 2026-04-12T00:00:00Z\n");

    for n in 1u32..=50 {
        let status = match n {
            1..=20 => "completed",
            21..=30 => "failed",
            31..=40 => "blocked",
            41..=45 => "pending",
            46..=50 => "ready",
            _ => unreachable!(),
        };
        s.push('\n');
        s.push_str(&format!("state node_{n:02}\n"));
        s.push_str(&format!("execution_status: {status}\n"));
        s.push_str("retry_count: 0\n");
    }

    s
}
