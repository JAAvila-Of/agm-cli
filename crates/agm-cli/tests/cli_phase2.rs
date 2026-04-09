//! Integration tests for Phase 2 CLI commands.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").unwrap()
}

/// Write the simple two-node workflow fixture to a temp dir.
fn write_fixture(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("test.agm");
    fs::write(
        &file,
        r#"agm: 1.0
package: test.cli
version: 1.0.0

node setup
type: workflow
summary: Setup step
code:
  lang: sh
  action: full
  body: echo setup done

node build
type: workflow
summary: Build step
depends: [setup]
code:
  lang: sh
  action: full
  body: echo build done
"#,
    )
    .unwrap();
    file
}

// ---------------------------------------------------------------------------
// run command
// ---------------------------------------------------------------------------

#[test]
fn test_run_help() {
    agm_cmd()
        .arg("run")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Execute nodes"));
}

#[test]
fn test_run_dry_run_prints_plan() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("run")
        .arg(file.to_str().unwrap())
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
}

// ---------------------------------------------------------------------------
// status command
// ---------------------------------------------------------------------------

#[test]
fn test_status_help() {
    agm_cmd()
        .arg("status")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("execution status"));
}

#[test]
fn test_status_no_state_exits_2() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("status")
        .arg(file.to_str().unwrap())
        .assert()
        .code(2);
}

// ---------------------------------------------------------------------------
// state command
// ---------------------------------------------------------------------------

#[test]
fn test_state_list_no_sidecar_exits_2() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("state")
        .arg("list")
        .arg(file.to_str().unwrap())
        .assert()
        .code(2);
}

#[test]
fn test_state_help() {
    agm_cmd()
        .arg("state")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Manage execution state"));
}

// ---------------------------------------------------------------------------
// verify command
// ---------------------------------------------------------------------------

#[test]
fn test_verify_no_checks_exits_2() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("verify")
        .arg(file.to_str().unwrap())
        .arg("--all")
        .assert()
        .code(2);
}

#[test]
fn test_verify_help() {
    agm_cmd()
        .arg("verify")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("verify checks"));
}

// ---------------------------------------------------------------------------
// context command
// ---------------------------------------------------------------------------

#[test]
fn test_context_node_prints_prompt() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("context")
        .arg(file.to_str().unwrap())
        .arg("--node")
        .arg("setup")
        .assert()
        .success()
        .stdout(predicate::str::contains("setup"));
}

#[test]
fn test_context_json_output() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("context")
        .arg(file.to_str().unwrap())
        .arg("--node")
        .arg("setup")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("node_id"))
        .stdout(predicate::str::contains("token_estimate"));
}

// ---------------------------------------------------------------------------
// mem command
// ---------------------------------------------------------------------------

#[test]
fn test_mem_gc_no_sidecar_succeeds() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("mem")
        .arg("gc")
        .arg(file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("GC complete"));
}

#[test]
fn test_mem_help() {
    agm_cmd()
        .arg("mem")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Manage memory sidecars"));
}

// ---------------------------------------------------------------------------
// retry command
// ---------------------------------------------------------------------------

#[test]
fn test_retry_help() {
    agm_cmd()
        .arg("retry")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Retry failed"));
}

// ---------------------------------------------------------------------------
// Additional edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_state_get_no_sidecar_exits_2() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("state")
        .arg("get")
        .arg(file.to_str().unwrap())
        .arg("--node")
        .arg("setup")
        .assert()
        .code(2);
}

#[test]
fn test_context_missing_node_exits_1() {
    let dir = tempdir().unwrap();
    let file = write_fixture(dir.path());
    agm_cmd()
        .arg("context")
        .arg(file.to_str().unwrap())
        .arg("--node")
        .arg("nonexistent.node")
        .assert()
        .failure();
}
