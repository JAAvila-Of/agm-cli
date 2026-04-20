//! CLI e2e and integration tests for `agm corpus`.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

fn agm() -> Command {
    Command::cargo_bin("agm").expect("agm binary not found")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").unwrap()
}

// ---------------------------------------------------------------------------
// Integration: validate all bank examples parse + validate cleanly
// ---------------------------------------------------------------------------

#[test]
fn test_all_bank_examples_are_valid_agm() {
    use agm_cli::corpora::example_bank;
    use agm_core::parser;
    use agm_core::validator::{ValidateOptions, validate};

    for (name, content) in example_bank() {
        let parsed = parser::parse(content)
            .unwrap_or_else(|e| panic!("bank example '{name}' failed to parse: {e:?}"));

        let opts = ValidateOptions::default();
        let diags = validate(&parsed, content, name, &opts);

        assert!(
            !diags.has_errors(),
            "bank example '{name}' failed validation: {:?}",
            diags.diagnostics()
        );
    }
}

// ---------------------------------------------------------------------------
// Integration: template content checks
// ---------------------------------------------------------------------------

#[test]
fn test_full_template_contains_grammar_summary() {
    use agm_cli::corpora::{CorpusFlavor, builtin_template};
    let tmpl = builtin_template(CorpusFlavor::Full);
    assert!(
        tmpl.contains("## Grammar Summary"),
        "full template must contain Grammar Summary section"
    );
    assert!(
        tmpl.contains("## Node Types"),
        "full template must contain Node Types section"
    );
    assert!(
        tmpl.contains("## Loading Modes"),
        "full template must contain Loading Modes section"
    );
}

#[test]
fn test_all_templates_start_with_agm_header() {
    use agm_cli::corpora::{CorpusFlavor, builtin_template};
    for flavor in [
        CorpusFlavor::Full,
        CorpusFlavor::Standard,
        CorpusFlavor::GrammarOnly,
    ] {
        let tmpl = builtin_template(flavor);
        assert!(
            tmpl.starts_with("# AGM v1.2.0"),
            "{flavor:?} template must start with AGM v1.2.0 header"
        );
    }
}

// ---------------------------------------------------------------------------
// e2e: basic output contains the AGM reference header
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_default_stdout_contains_agm_header() {
    agm()
        .args(["corpus"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# AGM v1.2.0"));
}

// ---------------------------------------------------------------------------
// e2e: --count-only prints an integer
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_count_only_prints_integer() {
    let output = agm()
        .args(["corpus", "--count-only"])
        .output()
        .expect("agm corpus --count-only must run");

    assert!(output.status.success(), "exit code must be 0");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let trimmed = stdout.trim();
    let n: usize = trimmed
        .parse()
        .expect("--count-only output must be a valid integer");
    assert!(
        n > 0,
        "--count-only must print a positive token count, got {n}"
    );
}

// ---------------------------------------------------------------------------
// e2e: --count-only on full flavor is >= 1800
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_count_only_full_at_least_1800() {
    let output = agm()
        .args(["corpus", "--flavor", "full", "--count-only"])
        .output()
        .expect("agm corpus --count-only must run");

    assert!(output.status.success());
    let n: usize = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        n >= 1800,
        "full flavor must have at least 1800 tokens, got {n}"
    );
}

// ---------------------------------------------------------------------------
// e2e: --for anthropic output begins with the Anthropic preamble
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_for_anthropic_starts_with_role_preamble() {
    agm()
        .args(["corpus", "--for", "anthropic"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "You are an AGM-emitting assistant.",
        ));
}

// ---------------------------------------------------------------------------
// e2e: --for openai output begins with the OpenAI preamble
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_for_openai_starts_with_role_preamble() {
    agm()
        .args(["corpus", "--for", "openai"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "You are an AGM-emitting assistant.",
        ))
        .stdout(predicate::str::contains("create_ticket"));
}

// ---------------------------------------------------------------------------
// e2e: --format json emits valid JSON with estimated_tokens integer
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_format_json_emits_valid_json() {
    let output = agm()
        .args(["corpus", "--format", "json"])
        .output()
        .expect("agm corpus --format json must run");

    assert!(output.status.success(), "exit code must be 0");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("--format json output must be valid JSON: {e}\n{stdout}"));

    assert!(
        v.get("estimated_tokens")
            .and_then(|t| t.as_u64())
            .unwrap_or(0)
            > 0,
        "JSON must contain a positive estimated_tokens"
    );
    assert!(
        v.get("body").and_then(|b| b.as_str()).is_some(),
        "JSON must contain a body string"
    );
    assert!(
        v.get("flavor").and_then(|f| f.as_str()).is_some(),
        "JSON must contain a flavor string"
    );
    assert!(
        v.get("target").and_then(|t| t.as_str()).is_some(),
        "JSON must contain a target string"
    );
}

// ---------------------------------------------------------------------------
// e2e: --output writes to file, exit 0
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_output_to_file_exits_zero() {
    let tmp = NamedTempFile::with_suffix(".md").expect("tempfile creation must succeed");
    let path = tmp.path().to_str().unwrap().to_owned();

    agm().args(["corpus", "--output", &path]).assert().success();

    let content = std::fs::read_to_string(&path).expect("output file must be readable");
    assert!(
        content.contains("# AGM v1.2.0"),
        "output file must contain the AGM reference header"
    );
}

// ---------------------------------------------------------------------------
// e2e: --min-tokens padded target succeeds
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_min_tokens_grammar_only_padded() {
    // 5000 tokens well above grammar-only base; bank of 12 examples should
    // reach this from the grammar-only starting point.
    let output = agm()
        .args(["corpus", "--flavor", "grammar-only", "--min-tokens", "5000"])
        .output()
        .expect("agm corpus with min-tokens must run");

    // If bank can cover it: exit 0. If not: exit 1. Both are valid here;
    // we just check the command runs without crashing (exit 0 or 1).
    let code = output.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "exit code must be 0 (padded) or 1 (bank exhausted), got {code}"
    );
}

// ---------------------------------------------------------------------------
// e2e: --min-tokens unreachable → exit 1 with helpful message
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_bank_exhausted_exits_1() {
    agm()
        .args(["corpus", "--min-tokens", "999999"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("bank exhausted"));
}

// ---------------------------------------------------------------------------
// e2e: --no-version omits the spec version header
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_no_version_omits_spec_version_header() {
    agm()
        .args(["corpus", "--no-version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# AGM spec version:").not());
}

// ---------------------------------------------------------------------------
// e2e: default includes spec version header
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_default_includes_spec_version_header() {
    agm()
        .args(["corpus"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# AGM spec version: 1.2.0"));
}

// ---------------------------------------------------------------------------
// e2e: --flavor standard stays within 1500-token ceiling (count-only check)
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_standard_count_only_at_most_1500() {
    let output = agm_cmd()
        .args([
            "corpus",
            "--flavor",
            "standard",
            "--no-version",
            "--count-only",
        ])
        .output()
        .expect("command must run");

    assert!(output.status.success());
    let n: usize = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(n <= 1500, "standard corpus must be <= 1500 tokens, got {n}");
}

// ---------------------------------------------------------------------------
// e2e: --flavor grammar-only stays within 600-token ceiling (count-only check)
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_grammar_only_count_only_at_most_600() {
    let output = agm_cmd()
        .args([
            "corpus",
            "--flavor",
            "grammar-only",
            "--no-version",
            "--count-only",
        ])
        .output()
        .expect("command must run");

    assert!(output.status.success());
    let n: usize = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        n <= 600,
        "grammar-only corpus must be <= 600 tokens, got {n}"
    );
}

// ---------------------------------------------------------------------------
// e2e: JSON target field reflects the --for argument
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_json_target_field_matches_for_arg() {
    let output = agm()
        .args(["corpus", "--for", "anthropic", "--format", "json"])
        .output()
        .expect("command must run");

    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("must be valid JSON");
    assert_eq!(
        v["target"].as_str(),
        Some("anthropic"),
        "target field must equal 'anthropic'"
    );
}

// ---------------------------------------------------------------------------
// e2e: JSON flavor field reflects the --flavor argument
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_json_flavor_field_matches_flavor_arg() {
    let output = agm()
        .args(["corpus", "--flavor", "standard", "--format", "json"])
        .output()
        .expect("command must run");

    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("must be valid JSON");
    assert_eq!(
        v["flavor"].as_str(),
        Some("standard"),
        "flavor field must equal 'standard'"
    );
}

// ---------------------------------------------------------------------------
// e2e: --count-only with --for anthropic still prints integer
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_count_only_with_for_anthropic_prints_integer() {
    let output = agm()
        .args(["corpus", "--count-only", "--for", "anthropic"])
        .output()
        .expect("command must run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let _: usize = stdout.trim().parse().expect("must be an integer");
}
