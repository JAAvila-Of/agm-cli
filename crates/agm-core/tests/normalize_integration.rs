//! Integration tests for `agm_core::normalize`.
//!
//! Each test loads a fixture, calls `normalize_text`, and snapshots the
//! (output, report summary) via `insta`.

use agm_core::normalize::{NormalizeConfig, normalize_text};

fn fixture(name: &str) -> String {
    let path = format!("tests/fixtures/normalize/{name}");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"))
}

// ---------------------------------------------------------------------------
// plan_execution_synonym — type + field rewrites
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_plan_execution_synonym() {
    let raw = fixture("plan_execution_synonym.agm");
    let config = NormalizeConfig::default();
    let (output, report) = normalize_text(&raw, &config).expect("parse error");

    // Type must be rewritten
    assert!(
        output.contains("type: orchestration"),
        "expected type: orchestration in output:\n{output}"
    );
    // groups synonym must disappear (but parallel_groups is ok)
    // Check that standalone "groups:" does not appear (not as part of "parallel_groups:")
    let has_bare_groups = output.lines().any(|l| {
        let trimmed = l.trim();
        trimmed == "groups:" || trimmed.starts_with("groups: ") || trimmed.starts_with("- groups:")
    });
    assert!(
        !has_bare_groups,
        "expected no bare 'groups:' in output:\n{output}"
    );
    // depends_on synonym must disappear
    assert!(
        !output.contains("depends_on:"),
        "expected no 'depends_on:' in output:\n{output}"
    );
    // Report must have at least 2 rewrites (type + field)
    assert!(
        report.rewrites.len() >= 2,
        "expected >=2 rewrites, got {}",
        report.rewrites.len()
    );

    insta::assert_yaml_snapshot!("plan_execution_synonym_report", {
        ".rewrites[].span" => "[span]",
        ".warnings[].span" => "[span]",
    }, &report);
}

// ---------------------------------------------------------------------------
// phases_synonym — field rewrite only (type already canonical)
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_phases_synonym() {
    let raw = fixture("phases_synonym.agm");
    let config = NormalizeConfig::default();
    let (output, report) = normalize_text(&raw, &config).expect("parse error");

    assert!(
        !output.contains("phases:"),
        "expected no 'phases:' in output:\n{output}"
    );
    assert!(
        !report.rewrites.is_empty(),
        "expected at least one field rewrite"
    );
    // Type should remain orchestration (no type rewrite needed)
    let type_rewrites: Vec<_> = report
        .rewrites
        .iter()
        .filter(|r| r.rule_id.starts_with("type."))
        .collect();
    assert!(
        type_rewrites.is_empty(),
        "expected no type rewrites for already-canonical type"
    );

    insta::assert_yaml_snapshot!("phases_synonym_report", {
        ".rewrites[].span" => "[span]",
        ".warnings[].span" => "[span]",
    }, &report);
}

// ---------------------------------------------------------------------------
// depends_on_universal — universal field alias rewrite
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_depends_on_universal() {
    let raw = fixture("depends_on_universal.agm");
    let config = NormalizeConfig::default();
    let (output, report) = normalize_text(&raw, &config).expect("parse error");

    assert!(
        output.contains("depends:"),
        "expected 'depends:' in output:\n{output}"
    );
    assert!(
        !output.contains("depends_on:"),
        "expected no 'depends_on:' in output:\n{output}"
    );
    assert_eq!(report.rewrites.len(), 1);
    assert_eq!(report.rewrites[0].before, "depends_on");
    assert_eq!(report.rewrites[0].after, "depends");

    insta::assert_yaml_snapshot!("depends_on_universal_report", {
        ".rewrites[].span" => "[span]",
        ".warnings[].span" => "[span]",
    }, &report);
}

// ---------------------------------------------------------------------------
// both_present_equal — silent drop (no rewrites, no warnings)
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_both_present_equal() {
    let raw = fixture("both_present_equal.agm");
    let config = NormalizeConfig::default();
    let (_, report) = normalize_text(&raw, &config).expect("parse error");

    assert!(
        report.rewrites.is_empty(),
        "expected no rewrites for equal collision, got: {:?}",
        report.rewrites
    );
    assert!(
        report.warnings.is_empty(),
        "expected no warnings for equal collision, got: {:?}",
        report.warnings
    );
}

// ---------------------------------------------------------------------------
// both_present_different — warning emitted, canonical kept
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_both_present_different() {
    use agm_core::normalize::NormalizeWarningCode;
    let raw = fixture("both_present_different.agm");
    let config = NormalizeConfig::default();
    let (output, report) = normalize_text(&raw, &config).expect("parse error");

    // Canonical value kept
    assert!(
        output.contains("canonical.dep"),
        "expected canonical dep in output:\n{output}"
    );
    assert!(
        report.rewrites.is_empty(),
        "expected no rewrites for collision"
    );
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(
        report.warnings[0].code,
        NormalizeWarningCode::CollisionKeepingCanonical
    );

    insta::assert_yaml_snapshot!("both_present_different_report", {
        ".rewrites[].span" => "[span]",
        ".warnings[].span" => "[span]",
    }, &report);
}

// ---------------------------------------------------------------------------
// custom_type_unknown — type preserved (no synonym mapping)
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_custom_type_unknown() {
    let raw = fixture("custom_type_unknown.agm");
    let config = NormalizeConfig::default();
    let (output, report) = normalize_text(&raw, &config).expect("parse error");

    assert!(
        output.contains("type: my_custom_type"),
        "custom type must be preserved:\n{output}"
    );
    assert!(
        report.rewrites.is_empty(),
        "expected no rewrites for unknown custom type"
    );
}

// ---------------------------------------------------------------------------
// wrong_kind — TypeMismatch warning, no rewrite
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_wrong_kind() {
    use agm_core::normalize::NormalizeWarningCode;
    let raw = fixture("wrong_kind.agm");
    let config = NormalizeConfig::default();
    let (output, report) = normalize_text(&raw, &config).expect("parse error");

    // The synonym field must still be in the output (was not rewritten)
    assert!(
        output.contains("depends_on:"),
        "synonym must be preserved on type mismatch:\n{output}"
    );
    assert!(
        report.rewrites.is_empty(),
        "expected no rewrites on type mismatch"
    );
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].code, NormalizeWarningCode::TypeMismatch);

    insta::assert_yaml_snapshot!("wrong_kind_report", {
        ".rewrites[].span" => "[span]",
        ".warnings[].span" => "[span]",
    }, &report);
}

// ---------------------------------------------------------------------------
// already_canonical — zero rewrites
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_already_canonical() {
    let raw = fixture("already_canonical.agm");
    let config = NormalizeConfig::default();
    let (_, report) = normalize_text(&raw, &config).expect("parse error");

    assert!(
        report.is_empty(),
        "expected empty report for canonical input, got: {}",
        report.summary()
    );
}

// ---------------------------------------------------------------------------
// Idempotency: normalize(normalize(x)) == normalize(x)
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_idempotent_across_all_fixtures() {
    let fixtures = [
        "plan_execution_synonym.agm",
        "phases_synonym.agm",
        "depends_on_universal.agm",
        "both_present_equal.agm",
        "both_present_different.agm",
        "custom_type_unknown.agm",
        "wrong_kind.agm",
        "already_canonical.agm",
    ];

    let config = NormalizeConfig::default();
    for name in fixtures {
        let raw = fixture(name);
        let (first_pass, _) =
            normalize_text(&raw, &config).unwrap_or_else(|_| panic!("parse error on {name}"));
        let (second_pass, second_report) = normalize_text(&first_pass, &config)
            .unwrap_or_else(|_| panic!("parse error on second pass of {name}"));

        // The output must be stable
        assert_eq!(
            first_pass, second_pass,
            "idempotency violated for fixture {name}"
        );
        // The second report must be empty of rewrites
        assert!(
            second_report.rewrites.is_empty(),
            "second pass produced rewrites for fixture {name}: {:?}",
            second_report.rewrites
        );
    }
}

// ---------------------------------------------------------------------------
// RuleSet::builtin() snapshot (catches accidental rule drift)
// ---------------------------------------------------------------------------

#[test]
fn test_builtin_ruleset_snapshot() {
    let rs = agm_core::normalize::RuleSet::builtin();
    insta::assert_yaml_snapshot!("builtin_ruleset", &rs);
}

// ---------------------------------------------------------------------------
// Perf smoke: large file normalizes in under 50ms
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_large_file_under_50ms() {
    use std::time::Instant;

    // Build a synthetic 50-node file with depends_on synonyms
    let mut src = "agm: 1.0\npackage: perf.test\nversion: 0.1.0\n\n".to_owned();
    for i in 0..50 {
        src.push_str(&format!(
            "node node{i}\ntype: workflow\nsummary: node {i}\ndepends_on: [base.facts]\n\n"
        ));
    }

    let config = NormalizeConfig::default();
    let start = Instant::now();
    let (_, report) = normalize_text(&src, &config).expect("parse error");
    let elapsed = start.elapsed();

    assert_eq!(report.rewrites.len(), 50, "expected 50 rewrites");
    assert!(
        elapsed.as_millis() < 50,
        "normalization of 50 nodes took {}ms (>50ms limit)",
        elapsed.as_millis()
    );
}
