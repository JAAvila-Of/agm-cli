//! CLI integration tests for `type: ticket` nodes.
//!
//! Fixture paths are relative to `crates/agm-core/tests/fixtures/`.

use assert_cmd::Command;
use predicates::prelude::*;

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").unwrap()
}

fn fixture(relative: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .join("../agm-core/tests/fixtures")
        .join(relative);
    path.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// validate — valid ticket
// ---------------------------------------------------------------------------

#[test]
fn cli_validate_ticket_valid_exits_zero() {
    agm_cmd()
        .arg("validate")
        .arg(fixture("valid/ticket_create.agm"))
        .assert()
        .success()
        .stderr(predicate::str::contains("OK"));
}

// ---------------------------------------------------------------------------
// validate — invalid tickets
// ---------------------------------------------------------------------------

#[test]
fn cli_validate_ticket_missing_priority_exits_nonzero_with_v024() {
    agm_cmd()
        .arg("validate")
        .arg(fixture("invalid/ticket_missing_priority.agm"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("AGM-V024"));
}

#[test]
fn cli_validate_ticket_edit_without_id_exits_nonzero_with_v031() {
    agm_cmd()
        .arg("validate")
        .arg(fixture("invalid/ticket_edit_without_ticket_id.agm"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("AGM-V031"));
}

#[test]
fn cli_validate_ticket_invalid_action_exits_nonzero_with_v029() {
    agm_cmd()
        .arg("validate")
        .arg(fixture("invalid/ticket_invalid_action.agm"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("AGM-V029"));
}

// ---------------------------------------------------------------------------
// render — JSON output
// ---------------------------------------------------------------------------

#[test]
fn cli_render_ticket_json_contains_all_fields() {
    let output = agm_cmd()
        .arg("render")
        .arg(fixture("valid/ticket_full.agm"))
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(output.status.success(), "render command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");

    // The rendered JSON is an object with a "nodes" array.
    let nodes = parsed["nodes"].as_array().expect("expected nodes array");
    let ticket = nodes
        .iter()
        .find(|n| n.get("type").and_then(|v| v.as_str()) == Some("ticket"))
        .expect("expected at least one ticket node in output");

    assert_eq!(ticket["type"], "ticket");
    assert!(ticket.get("title").is_some(), "title should be present");
    assert!(
        ticket.get("description").is_some(),
        "description should be present"
    );
    assert!(
        ticket.get("priority").is_some(),
        "priority should be present"
    );
    assert!(ticket.get("action").is_some(), "action should be present");
}

// ---------------------------------------------------------------------------
// graph — edge from ticket to dependency
// ---------------------------------------------------------------------------

#[test]
fn cli_graph_ticket_depends_produces_edge() {
    agm_cmd()
        .arg("graph")
        .arg(fixture("valid/ticket_with_depends.agm"))
        .assert()
        .success()
        .stdout(predicate::str::contains("->"));
}
