//! Integration tests for `agm_core::repair`.
//!
//! Each test loads a fixture, runs `repair_text`, and verifies the output
//! and report structure.

use agm_core::repair::{RepairConfig, repair_text};

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/repair")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"))
}

// ---------------------------------------------------------------------------
// Helper: run repair with safety_net disabled (fixtures may not parse cleanly)
// ---------------------------------------------------------------------------

fn repair_no_safety(input: &str) -> agm_core::repair::RepairOutput {
    repair_text(input, &RepairConfig {
        safety_net: false,
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

#[test]
fn test_repair_smart_quotes_fixture() {
    let raw = fixture("smart_quotes.agm");
    let out = repair_no_safety(&raw);

    // Smart quotes should be replaced
    assert!(
        !out.text.contains('\u{201C}')
            && !out.text.contains('\u{201D}')
            && !out.text.contains('\u{2018}')
            && !out.text.contains('\u{2019}')
            && !out.text.contains('\u{00AB}')
            && !out.text.contains('\u{00BB}'),
        "smart quotes should all be replaced"
    );
    assert!(!out.report.is_empty(), "should have repair records");
}

#[test]
fn test_repair_star_bullets_fixture() {
    let raw = fixture("star_bullets.agm");
    let out = repair_no_safety(&raw);

    // All * and + bullets should be replaced with -
    for line in out.text.lines() {
        let trimmed = line.trim_start();
        assert!(
            !trimmed.starts_with("* ") && !trimmed.starts_with("+ "),
            "star/plus bullet should be replaced: {line:?}"
        );
    }
    assert!(!out.report.is_empty());
}

#[test]
fn test_repair_prose_prefix_fixture() {
    let raw = fixture("prose_prefix.agm");
    let out = repair_no_safety(&raw);

    // Output should start with the AGM header
    let first_non_blank = out.text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    assert!(
        first_non_blank.starts_with("agm:"),
        "first non-blank line should be 'agm:' after stripping prose, got: {first_non_blank:?}"
    );
    assert!(!out.report.is_empty());
}

#[test]
fn test_repair_wrapped_fence_fixture() {
    let raw = fixture("wrapped_fence.agm");
    let out = repair_no_safety(&raw);

    // The wrapping fences should be stripped
    let first_non_blank = out.text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    assert!(
        !first_non_blank.starts_with("```"),
        "fence should be stripped, got first line: {first_non_blank:?}"
    );
    assert!(!out.report.is_empty());
}

#[test]
fn test_repair_crlf_fixture() {
    let raw = fixture("crlf.agm");
    let out = repair_no_safety(&raw);

    assert!(!out.text.contains('\r'), "CRLF should be normalized to LF");
    assert!(!out.report.is_empty());
}

#[test]
fn test_repair_tabs_fixture() {
    let raw = fixture("tabs.agm");
    let out = repair_no_safety(&raw);

    // No leading tabs in output
    for line in out.text.lines() {
        assert!(
            !line.starts_with('\t'),
            "leading tab should be replaced: {line:?}"
        );
    }
    assert!(!out.report.is_empty());
}

#[test]
fn test_repair_bom_fixture() {
    let raw = fixture("bom.agm");
    assert!(
        raw.starts_with('\u{FEFF}'),
        "fixture must have BOM prefix"
    );
    let out = repair_no_safety(&raw);
    assert!(
        !out.text.starts_with('\u{FEFF}'),
        "BOM should be stripped"
    );
    assert!(!out.report.is_empty());
}

#[test]
fn test_repair_already_clean_fixture() {
    let raw = fixture("already_clean.agm");
    let config = RepairConfig::default();
    let out = repair_text(&raw, &config);

    assert_eq!(out.text, raw, "clean fixture should be unchanged");
    assert!(out.report.is_empty(), "no records for clean input");
    assert!(!out.rolled_back);
}

#[test]
fn test_repair_safety_net_triggers_on_garbage_fence() {
    let raw = fixture("safety_net_triggers.agm");
    let config = RepairConfig {
        safety_net: true,
        ..Default::default()
    };
    let out = repair_text(&raw, &config);

    // The fence rule would strip the fences but leave garbage; safety net should roll back.
    assert!(
        out.rolled_back,
        "safety net should have triggered on unparseable content"
    );
    assert_eq!(out.text, raw, "original should be preserved on rollback");
    // Records from attempted repairs are preserved for debugging.
    assert!(
        !out.report.rewrites.is_empty(),
        "records should be preserved even when rolled back"
    );
}

#[test]
fn test_repair_missing_fence_sample_fixture() {
    let raw = fixture("missing_fence_sample.agm");
    let out = repair_no_safety(&raw);

    // The wrapping fence should be stripped and the AGM content exposed
    let first_non_blank = out.text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    assert!(
        !first_non_blank.starts_with("```"),
        "fence should be stripped, first line: {first_non_blank:?}"
    );
    assert!(!out.report.is_empty());
}

// ---------------------------------------------------------------------------
// Idempotency: repair(repair(x)) == repair(x) for all fixtures
// ---------------------------------------------------------------------------

macro_rules! idempotency_test {
    ($test_name:ident, $fixture_name:literal) => {
        #[test]
        fn $test_name() {
            let raw = fixture($fixture_name);
            let config = RepairConfig {
                safety_net: false,
                ..Default::default()
            };
            let first = repair_text(&raw, &config);
            let second = repair_text(&first.text, &config);
            assert_eq!(
                first.text, second.text,
                "idempotency failed for fixture {}",
                $fixture_name
            );
            assert!(
                second.report.is_empty(),
                "second pass should be a no-op for fixture {}",
                $fixture_name
            );
        }
    };
}

idempotency_test!(test_idempotency_smart_quotes, "smart_quotes.agm");
idempotency_test!(test_idempotency_star_bullets, "star_bullets.agm");
idempotency_test!(test_idempotency_prose_prefix, "prose_prefix.agm");
idempotency_test!(test_idempotency_wrapped_fence, "wrapped_fence.agm");
idempotency_test!(test_idempotency_crlf, "crlf.agm");
idempotency_test!(test_idempotency_tabs, "tabs.agm");
idempotency_test!(test_idempotency_bom, "bom.agm");
idempotency_test!(test_idempotency_already_clean, "already_clean.agm");
idempotency_test!(test_idempotency_missing_fence_sample, "missing_fence_sample.agm");
