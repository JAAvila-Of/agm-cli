//! CLI e2e tests for `agm schema`.
//!
//! Step 12 from the implementation plan.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn agm() -> Command {
    Command::cargo_bin("agm").expect("agm binary not found")
}

// ---------------------------------------------------------------------------
// Basic output
// ---------------------------------------------------------------------------

#[test]
fn test_schema_ticket_outputs_valid_json() {
    agm()
        .args(["schema", "ticket"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\": \"object\""));
}

#[test]
fn test_schema_ticket_has_schema_key() {
    let output = agm().args(["schema", "ticket"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["type"].as_str(), Some("object"));
    assert!(v.get("required").is_some());
}

// ---------------------------------------------------------------------------
// Dialect: anthropic-tool-use
// ---------------------------------------------------------------------------

#[test]
fn test_schema_ticket_anthropic_tool_use_has_name() {
    let output = agm()
        .args(["schema", "ticket", "--for", "anthropic-tool-use"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["name"].as_str(), Some("create_ticket"));
    assert!(v.get("input_schema").is_some(), "must have input_schema");
}

#[test]
fn test_schema_ticket_anthropic_tool_use_has_input_schema() {
    agm()
        .args(["schema", "ticket", "--for", "anthropic-tool-use"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"input_schema\""));
}

// ---------------------------------------------------------------------------
// Dialect: openai-tool
// ---------------------------------------------------------------------------

#[test]
fn test_schema_ticket_openai_tool_has_function_key() {
    let output = agm()
        .args(["schema", "ticket", "--for", "openai-tool"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["type"].as_str(), Some("function"));
    let func = v.get("function").expect("must have 'function' key");
    assert_eq!(func["name"].as_str(), Some("create_ticket"));
}

// ---------------------------------------------------------------------------
// Strict mode
// ---------------------------------------------------------------------------

#[test]
fn test_schema_ticket_strict_has_additional_properties_false() {
    let output = agm()
        .args(["schema", "ticket", "--strict"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        v["additionalProperties"].as_bool(),
        Some(false),
        "strict mode must set additionalProperties=false"
    );
}

// ---------------------------------------------------------------------------
// Custom / unknown type → exit code 1
// ---------------------------------------------------------------------------

#[test]
fn test_schema_custom_foo_exits_with_1() {
    agm()
        .args(["schema", "custom_foo"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("custom"));
}

#[test]
fn test_schema_unknown_type_exits_with_1() {
    agm()
        .args(["schema", "nonexistent_type"])
        .assert()
        .failure()
        .code(1);
}

// ---------------------------------------------------------------------------
// Custom tool name
// ---------------------------------------------------------------------------

#[test]
fn test_schema_ticket_custom_tool_name_anthropic() {
    let output = agm()
        .args([
            "schema",
            "ticket",
            "--for",
            "anthropic-tool-use",
            "--tool-name",
            "my_ticket",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["name"].as_str(), Some("my_ticket"));
}

// ---------------------------------------------------------------------------
// `all` subcommand
// ---------------------------------------------------------------------------

#[test]
fn test_schema_all_creates_11_files() {
    let tmp = TempDir::new().unwrap();
    agm()
        .args(["schema", "all", "--output", tmp.path().to_str().unwrap()])
        .assert()
        .success();

    let expected_types = [
        "facts",
        "rules",
        "workflow",
        "entity",
        "decision",
        "exception",
        "example",
        "glossary",
        "anti_pattern",
        "orchestration",
        "ticket",
    ];
    for t in expected_types {
        let file = tmp.path().join(format!("{t}.json"));
        assert!(file.exists(), "expected {t}.json to be created");
        let content = std::fs::read_to_string(&file).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&content).unwrap_or_else(|_| panic!("{t}.json is not valid JSON"));
        assert_eq!(v["type"].as_str(), Some("object"));
    }
}

#[test]
fn test_schema_all_anthropic_creates_subdir_files() {
    let tmp = TempDir::new().unwrap();
    agm()
        .args([
            "schema",
            "all",
            "--for",
            "anthropic-tool-use",
            "--output",
            tmp.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let subdir = tmp.path().join("anthropic_tool_use");
    assert!(
        subdir.exists(),
        "anthropic_tool_use subdir should be created"
    );
    assert!(subdir.join("ticket.json").exists());
}

// ---------------------------------------------------------------------------
// YAML format
// ---------------------------------------------------------------------------

#[test]
fn test_schema_ticket_yaml_format() {
    agm()
        .args(["schema", "ticket", "--format", "yaml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("type: object"));
}

// ---------------------------------------------------------------------------
// Help
// ---------------------------------------------------------------------------

#[test]
fn test_schema_help_exits_successfully() {
    agm().args(["schema", "--help"]).assert().success();
}
