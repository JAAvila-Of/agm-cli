//! E2E tests for CLI command edge cases.

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

/// Returns an absolute path to a fixture in the workspace-root `tests/fixtures/` tree.
fn fixture(relative: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    path.to_string_lossy().into_owned()
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
// CLI Command Edge Cases
// =============================================================================

/// Validate a non-existent file: `agm validate nonexistent.agm` — verify
/// helpful error, non-zero exit.
#[test]
fn test_validate_nonexistent_file_exits_nonzero_with_error() {
    agm_cmd()
        .arg("validate")
        .arg("__totally_nonexistent_file__.agm")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot read").or(predicate::str::contains("error")));
}

/// Validate a directory instead of a file: `agm validate ./somedir/` — verify error.
#[test]
fn test_validate_directory_path_exits_nonzero_with_error() {
    let dir = tempdir().unwrap();
    // Pass the directory path itself as the file argument
    agm_cmd()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .failure();
}

/// Render with an invalid format flag: `agm render file.agm --format invalid` —
/// verify clap rejects the value with a non-zero exit.
#[test]
fn test_render_invalid_format_exits_nonzero() {
    agm_cmd()
        .arg("render")
        .arg(fixture("json/forward/minimal.agm"))
        .arg("--format")
        .arg("totally_invalid_format_xyz")
        .assert()
        .failure();
}

/// Graph on a file with no dependency edges (isolated nodes): verify graph
/// still succeeds and produces output.
#[test]
fn test_graph_no_dependencies_still_outputs_dot() {
    let dir = tempdir().unwrap();

    let agm_content = "agm: 1.0\n\
        package: test.graph.isolated1\n\
        version: 1.0.0\n\
        \n\
        node node_alpha\n\
        type: facts\n\
        summary: Isolated facts node\n\
        \n\
        node node_beta\n\
        type: facts\n\
        summary: Isolated facts node 2\n";

    let agm_file = dir.path().join("isolated_nodes.agm");
    fs::write(&agm_file, agm_content).unwrap();

    agm_cmd()
        .arg("graph")
        .arg(&agm_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph"));
}

/// Load with summary mode: verify JSON output contains nodes.
#[test]
fn test_load_summary_mode_outputs_json_with_nodes() {
    agm_cmd()
        .arg("load")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--mode")
        .arg("summary")
        .assert()
        .success()
        .stdout(predicate::str::contains("node").or(predicate::str::contains("nodes")));
}

/// Load with operational mode: verify JSON output.
#[test]
fn test_load_operational_mode_outputs_json() {
    let output = agm_cmd()
        .arg("load")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--mode")
        .arg("operational")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "load operational failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with('{') || trimmed.starts_with('['),
        "expected JSON output for operational mode, got: {trimmed}"
    );
}

/// Load with executable mode: verify JSON output.
#[test]
fn test_load_executable_mode_outputs_json() {
    let output = agm_cmd()
        .arg("load")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--mode")
        .arg("executable")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "load executable failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with('{') || trimmed.starts_with('['),
        "expected JSON output for executable mode, got: {trimmed}"
    );
}

/// Load with full mode: verify JSON output.
#[test]
fn test_load_full_mode_outputs_json() {
    let output = agm_cmd()
        .arg("load")
        .arg(fixture("json/forward/auth_platform.agm"))
        .arg("--mode")
        .arg("full")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "load full failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with('{') || trimmed.starts_with('['),
        "expected JSON output for full mode, got: {trimmed}"
    );
}

/// Multiple commands in sequence on the same file: validate, render, graph —
/// all must succeed and produce consistent output.
#[test]
fn test_multiple_commands_in_sequence_are_consistent() {
    let dir = tempdir().unwrap();

    let agm_content = "agm: 1.0\n\
        package: test.edge.sequence1\n\
        version: 1.0.0\n\
        \n\
        node node_a\n\
        type: facts\n\
        summary: First node\n\
        \n\
        node node_b\n\
        type: facts\n\
        summary: Second node depends on first\n\
        depends: [node_a]\n";

    let agm_file = dir.path().join("sequence_test.agm");
    fs::write(&agm_file, agm_content).unwrap();

    // Step 1: validate
    agm_cmd()
        .arg("validate")
        .arg(&agm_file)
        .assert()
        .success();

    // Step 2: render to JSON
    let render_output = agm_cmd()
        .arg("render")
        .arg(&agm_file)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        render_output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&render_output.stderr)
    );

    // Step 3: graph
    agm_cmd()
        .arg("graph")
        .arg(&agm_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("node_a"))
        .stdout(predicate::str::contains("node_b"));
}

/// Large file: generate an AGM with 100 nodes, validate — must succeed.
#[test]
fn test_validate_large_file_with_100_nodes_succeeds() {
    let dir = tempdir().unwrap();

    let mut content = String::from("agm: 1.0\npackage: test.edge.large1\nversion: 1.0.0\n\n");

    // Root node
    content.push_str("node node_000\ntype: facts\nsummary: Root node\n\n");

    // 99 more nodes, each depending on the previous
    for i in 1..100usize {
        let node_id = format!("node_{:03}", i);
        let dep_id = format!("node_{:03}", i - 1);
        content.push_str(&format!(
            "node {node_id}\ntype: facts\nsummary: Node number {i}\ndepends: [{dep_id}]\n\n"
        ));
    }

    let agm_file = dir.path().join("large_file.agm");
    fs::write(&agm_file, &content).unwrap();

    agm_cmd()
        .arg("validate")
        .arg(&agm_file)
        .assert()
        .success();
}

/// Render with all supported formats succeeds.
#[test]
fn test_render_all_formats_succeed() {
    let formats = ["json", "json-canonical", "markdown", "agm"];

    for format in &formats {
        let output = agm_cmd()
            .arg("render")
            .arg(fixture("json/forward/minimal.agm"))
            .arg("--format")
            .arg(format)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "render --format {format} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !output.stdout.is_empty(),
            "render --format {format} produced no output"
        );
    }
}

/// Graph --topo flag: topological output must list node IDs.
#[test]
fn test_graph_topo_lists_node_ids() {
    let dir = tempdir().unwrap();

    let agm_content = "agm: 1.0\n\
        package: test.edge.topo1\n\
        version: 1.0.0\n\
        \n\
        node topo_start\n\
        type: facts\n\
        summary: Start node\n\
        \n\
        node topo_end\n\
        type: facts\n\
        summary: End node\n\
        depends: [topo_start]\n";

    let agm_file = dir.path().join("topo_test.agm");
    fs::write(&agm_file, agm_content).unwrap();

    agm_cmd()
        .arg("graph")
        .arg(&agm_file)
        .arg("--topo")
        .assert()
        .success()
        .stdout(predicate::str::contains("topo_start"));
}

/// Lint command on a valid file exits zero.
#[test]
fn test_lint_valid_inline_file_exits_zero() {
    let dir = tempdir().unwrap();

    let agm_content = "agm: 1.0\n\
        package: test.edge.lint1\n\
        version: 1.0.0\n\
        \n\
        node lint_node\n\
        type: facts\n\
        summary: A valid node for lint testing\n\
        items:\n\
          - some item\n";

    let agm_file = dir.path().join("lint_test.agm");
    fs::write(&agm_file, agm_content).unwrap();

    agm_cmd().arg("lint").arg(&agm_file).assert().success();
}

/// `agm run --dry-run` must not create a state file.
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
        ".state file must NOT be created during --dry-run"
    );
}

/// `agm --help` prints help text without error.
#[test]
fn test_help_flag_exits_zero() {
    agm_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("render"))
        .stdout(predicate::str::contains("graph"));
}

// =============================================================================
// STRESS / LARGE FILE TESTS
// =============================================================================

fn build_200_node_agm() -> String {
    let mut content = String::from("agm: 1.0\npackage: stress.pkg\nversion: 1.0.0\n\n");
    // Root node (no dependency)
    content.push_str("node stress.n000\ntype: facts\nsummary: Stress node 0\n\n");
    for i in 1..200usize {
        content.push_str(&format!(
            "node stress.n{i:03}\ntype: facts\nsummary: Stress node {i}\ndepends: [stress.n{:03}]\n\n",
            i - 1
        ));
    }
    content
}

/// Validate a 200-node AGM file: must exit successfully.
#[test]
fn test_validate_200_node_file_succeeds() {
    let dir = tempdir().unwrap();
    let agm_file = dir.path().join("stress_200.agm");
    fs::write(&agm_file, build_200_node_agm()).unwrap();

    agm_cmd()
        .arg("validate")
        .arg(&agm_file)
        .assert()
        .success();
}

/// Render a 200-node AGM file as JSON: must succeed and output all node IDs.
#[test]
fn test_render_200_node_file_json() {
    let dir = tempdir().unwrap();
    let agm_file = dir.path().join("stress_200.agm");
    fs::write(&agm_file, build_200_node_agm()).unwrap();

    let output = agm_cmd()
        .arg("render")
        .arg(&agm_file)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "render json on 200-node file failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Spot-check a few node IDs are present in the JSON output
    assert!(
        stdout.contains("stress.n000"),
        "JSON output should contain first node ID"
    );
    assert!(
        stdout.contains("stress.n199"),
        "JSON output should contain last node ID"
    );
}

/// Run `agm graph` on a 200-node AGM file: must succeed.
#[test]
fn test_graph_200_node_file() {
    let dir = tempdir().unwrap();
    let agm_file = dir.path().join("stress_200.agm");
    fs::write(&agm_file, build_200_node_agm()).unwrap();

    agm_cmd()
        .arg("graph")
        .arg(&agm_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph"));
}
