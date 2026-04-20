//! CLI end-to-end tests for `agm llm-bench`.
//!
//! All tests use cassette Replay mode — no network calls are made.

use assert_cmd::Command;
use predicates::prelude::*;

/// Path to the committed cassettes directory.
fn cassettes_dir() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/tests/llm/cassettes")
}

/// Run `agm llm-bench` with cassette replay for the given provider.
fn bench_cmd(provider: &str, model: &str) -> Command {
    let mut cmd = Command::cargo_bin("agm").unwrap();
    cmd.arg("llm-bench")
        .arg("--model")
        .arg(model)
        .arg("--provider")
        .arg(provider)
        .arg("--cassettes")
        .arg(cassettes_dir());
    cmd
}

#[test]
fn test_bench_messages_text_format_exits_0_or_1() {
    let mut cmd = bench_cmd("messages", "demo-m1");
    cmd.arg("--format").arg("text");
    // Exits 0 (all pass) or 1 (some fail) — both are valid for cassette-driven run
    cmd.assert()
        .code(predicate::in_iter([0i32, 1i32]))
        .stdout(predicate::str::contains("Compliance:"));
}

#[test]
fn test_bench_chat_text_format_exits_0_or_1() {
    let mut cmd = bench_cmd("chat", "demo-m1");
    cmd.arg("--format").arg("text");
    cmd.assert()
        .code(predicate::in_iter([0i32, 1i32]))
        .stdout(predicate::str::contains("Compliance:"));
}

#[test]
fn test_bench_json_format_is_valid_json() {
    let output = bench_cmd("messages", "demo-m1")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert!(parsed.get("compliance_rate").is_some());
    assert!(parsed.get("results").is_some());
}

#[test]
fn test_bench_markdown_format_contains_table() {
    bench_cmd("messages", "demo-m1")
        .arg("--format")
        .arg("markdown")
        .assert()
        .code(predicate::in_iter([0i32, 1i32]))
        .stdout(predicate::str::contains("| Case |"));
}

#[test]
fn test_bench_case_filter_ticket_only() {
    bench_cmd("messages", "demo-m1")
        .arg("--case")
        .arg("ticket/*")
        .assert()
        .code(predicate::in_iter([0i32, 1i32]))
        .stdout(predicate::str::contains("Cases:  4"));
}

#[test]
fn test_bench_case_filter_single_case() {
    bench_cmd("messages", "demo-m1")
        .arg("--case")
        .arg("ticket/a")
        .assert()
        .code(predicate::in_iter([0i32, 1i32]))
        .stdout(predicate::str::contains("Cases:  1"));
}

#[test]
fn test_bench_missing_model_exits_2() {
    Command::cargo_bin("agm")
        .unwrap()
        .arg("llm-bench")
        .arg("--cassettes")
        .arg(cassettes_dir())
        .assert()
        .code(2);
}

#[test]
fn test_bench_cost_only_requires_both_flags() {
    // --cost-only without pricing flags → exit 2
    Command::cargo_bin("agm")
        .unwrap()
        .arg("llm-bench")
        .arg("--model")
        .arg("demo-m1")
        .arg("--cost-only")
        .assert()
        .code(2);
}

#[test]
fn test_bench_cost_only_with_flags_exits_0() {
    Command::cargo_bin("agm")
        .unwrap()
        .arg("llm-bench")
        .arg("--model")
        .arg("demo-m1")
        .arg("--cost-only")
        .arg("--cost-per-1k-in")
        .arg("3.0")
        .arg("--cost-per-1k-out")
        .arg("15.0")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Cost estimate"));
}

#[test]
fn test_bench_missing_key_without_cassettes_exits_3() {
    // No cassettes, no live key → exit 3
    Command::cargo_bin("agm")
        .unwrap()
        .arg("llm-bench")
        .arg("--model")
        .arg("demo-m1")
        .arg("--live")
        .env_remove("AGM_MESSAGES_KEY")
        .env_remove("AGM_MESSAGES_ENDPOINT")
        .assert()
        .code(3);
}

#[test]
fn test_bench_report_compliance_rate_in_range() {
    let output = bench_cmd("messages", "demo-m1")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let rate = parsed["compliance_rate"].as_f64().unwrap();
    // Synthesized cassettes should produce some passing cases
    assert!((0.0..=1.0).contains(&rate));
}
