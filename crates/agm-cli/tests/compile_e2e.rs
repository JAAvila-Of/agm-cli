//! End-to-end tests for the `agm compile` command.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_compile_valid_markdown_to_stdout() {
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("compile")
        .arg("../../tests/fixtures/compiler/valid/login.md")
        .arg("--package")
        .arg("auth.platform")
        .arg("--version")
        .arg("0.1.0")
        .arg("--min-confidence")
        .arg("0");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("node "))
        .stdout(predicate::str::contains("type: "));
}

#[test]
fn test_compile_to_output_file() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.agm");

    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("compile")
        .arg("../../tests/fixtures/compiler/valid/login.md")
        .arg("--package")
        .arg("auth.platform")
        .arg("--output")
        .arg(&output)
        .arg("--min-confidence")
        .arg("0");
    cmd.assert().success();

    let content = fs::read_to_string(&output).unwrap();
    assert!(content.contains("agm: 1.0"));
    assert!(content.contains("package: auth.platform"));
}

#[test]
fn test_compile_with_validate_flag() {
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("compile")
        .arg("../../tests/fixtures/compiler/valid/login.md")
        .arg("--package")
        .arg("auth.platform")
        .arg("--validate")
        .arg("--min-confidence")
        .arg("0");
    cmd.assert().success();
}

#[test]
fn test_compile_empty_file_returns_error() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("empty.md");
    fs::write(&input, "").unwrap();

    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("compile")
        .arg(&input)
        .arg("--package")
        .arg("test.pkg");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("no nodes"));
}

#[test]
fn test_compile_json_output() {
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("compile")
        .arg("../../tests/fixtures/compiler/valid/login.md")
        .arg("--package")
        .arg("auth.platform")
        .arg("--json")
        .arg("--min-confidence")
        .arg("0");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"nodes\""));
}
