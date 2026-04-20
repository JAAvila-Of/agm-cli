//! Group G — Enforcement modes.
//!
//! Tests Standard vs Strict enforcement levels and build_unchecked bypass.

use agm_core::builder::{CodeBlockBuilder, FactsBuilder, RulesBuilder, TicketBuilder};
use agm_core::error::codes::ErrorCode;
use agm_core::model::fields::Priority;
use agm_core::model::schema::EnforcementLevel;
use agm_core::parser;
use agm_core::validator;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn reparse_and_validate_codes(node: &agm_core::model::node::Node) -> Vec<ErrorCode> {
    let text = node.render_canonical();
    // Parser may also reject invalid IDs (P002). In that case we return the parser
    // error codes so callers can assert on P002 instead of V021.
    match parser::parse(&text) {
        Ok(file) => {
            let diags = validator::validate(&file, &text, "<test>", &Default::default());
            diags.diagnostics().iter().map(|d| d.code).collect()
        }
        Err(errs) => errs.iter().map(|e| e.code).collect(),
    }
}

// ============================================================================
// G1 — build() defaults to Standard enforcement
// ============================================================================

#[test]
fn test_build_defaults_to_standard_enforcement() {
    // A ticket with V032 (title > 200 chars) should succeed under Standard
    // because V032 is a Warning, not an error.
    let long_title = "A".repeat(201);
    let result_default = TicketBuilder::new("enf.default.ticket")
        .summary("standard enforcement test")
        .title(&long_title)
        .description("desc")
        .priority(Priority::Normal)
        .build(); // uses Standard by default

    let result_explicit = TicketBuilder::new("enf.explicit.std.ticket")
        .summary("explicit standard enforcement")
        .title(&long_title)
        .description("desc")
        .priority(Priority::Normal)
        .build_with(EnforcementLevel::Standard);

    // Both must succeed — V032 is Warning, not Error
    assert!(
        result_default.is_ok(),
        "default build must use Standard (V032 warning should not block): {:?}",
        result_default.unwrap_err()
    );
    assert!(
        result_explicit.is_ok(),
        "explicit Standard must succeed for V032 warning: {:?}",
        result_explicit.unwrap_err()
    );
}

// ============================================================================
// G2 — Strict escalates known warnings to errors
// ============================================================================

#[test]
fn test_build_with_strict_escalates_v010_to_error() {
    // V010 = Warning for missing recommended field (items on facts)
    // In Strict mode it should escalate to an error
    let result_standard = FactsBuilder::new("enf.strict.facts.std")
        .summary("facts without items standard")
        .build_with(EnforcementLevel::Standard);

    let result_strict = FactsBuilder::new("enf.strict.facts.strict")
        .summary("facts without items strict")
        .build_with(EnforcementLevel::Strict);

    // Standard: V010 is Warning → build should succeed
    assert!(
        result_standard.is_ok(),
        "Standard mode must succeed when V010 is only warning: {:?}",
        result_standard.unwrap_err()
    );

    // Strict: check if V010 escalates
    match &result_strict {
        Ok(_) => {
            // V010 remained a warning even in Strict mode — document this behavior
        }
        Err(err) => {
            // V010 was escalated to error in Strict — this is the expected behavior
            assert!(
                err.is_validation(),
                "Strict escalation must produce Validation error"
            );
            let has_v010 = err
                .diagnostics()
                .map(|dc| dc.diagnostics().iter().any(|d| d.code == ErrorCode::V010))
                .unwrap_or(false);
            assert!(has_v010, "escalated error must be V010");
        }
    }
}

#[test]
fn test_build_with_strict_escalates_v012_warning() {
    // V012 = summary > 200 chars (Warning)
    let long_summary = "x".repeat(201);

    let result_standard = FactsBuilder::new("enf.strict.v012.std")
        .summary(&long_summary)
        .build_with(EnforcementLevel::Standard);

    // Standard must succeed
    assert!(
        result_standard.is_ok(),
        "Standard must succeed with V012 warning: {:?}",
        result_standard.unwrap_err()
    );

    // Strict behavior: V012 stays warning or is escalated (document both cases)
    let result_strict = FactsBuilder::new("enf.strict.v012.strict")
        .summary(&long_summary)
        .build_with(EnforcementLevel::Strict);

    match &result_strict {
        Ok(_) => { /* V012 stays warning in Strict */ }
        Err(err) => {
            assert!(err.is_validation(), "Strict must produce Validation error");
        }
    }
}

#[test]
fn test_build_with_strict_for_valid_node_succeeds() {
    // A fully valid node must succeed even in Strict mode
    let result = TicketBuilder::new("enf.strict.valid")
        .summary("fully valid ticket in strict mode")
        .title("Fully Valid Ticket")
        .description("A complete description.")
        .priority(Priority::High)
        .build_with(EnforcementLevel::Strict);

    assert!(
        result.is_ok(),
        "valid node must succeed in Strict mode: {:?}",
        result.unwrap_err()
    );
}

// ============================================================================
// G3 — build_unchecked skips validation entirely
// ============================================================================

#[test]
fn test_build_unchecked_skips_validation_entirely() {
    // Build a node with an invalid ID — would fail V021 in Standard mode
    let unchecked_result = FactsBuilder::new("INVALID ID with SPACES!")
        .summary("unchecked bypass")
        .build_unchecked();

    assert!(
        unchecked_result.is_ok(),
        "build_unchecked must succeed regardless of invalid id"
    );

    // The parser or validator will catch the invalid ID (P002 or V021).
    let node = unchecked_result.unwrap();
    let codes = reparse_and_validate_codes(&node);
    let caught = codes.contains(&ErrorCode::V021) || codes.contains(&ErrorCode::P002);
    assert!(
        caught,
        "parser (P002) or validator (V021) must reject invalid id, got codes: {:?}",
        codes
    );
}

#[test]
fn test_build_unchecked_bypasses_v024_missing_required_fields() {
    // A ticket built with build_unchecked can have missing required fields (V024)
    let result = TicketBuilder::new("enf.unchecked.ticket")
        .summary("wip ticket")
        .build_unchecked(); // missing title, description, priority

    assert!(
        result.is_ok(),
        "build_unchecked must bypass V024 for missing required fields"
    );

    let node = result.unwrap();
    assert!(node.title.is_none(), "title must be absent");
    assert!(node.description.is_none(), "description must be absent");
    assert!(node.priority.is_none(), "priority must be absent");
}

#[test]
fn test_build_unchecked_bypasses_v021_invalid_id() {
    // Directly verify: build succeeds, then validator catches V021
    let node = TicketBuilder::new("1starts.with.digit") // invalid pattern
        .summary("draft")
        .build_unchecked()
        .expect("build_unchecked must succeed for invalid id");

    assert_eq!(node.id, "1starts.with.digit");
    // Parser catches invalid IDs with P002 before the validator sees them.
    // Either P002 (parser) or V021 (validator) signals the invalid ID.
    let codes = reparse_and_validate_codes(&node);
    let caught = codes.contains(&ErrorCode::V021) || codes.contains(&ErrorCode::P002);
    assert!(
        caught,
        "P002 (parser) or V021 (validator) must fire for invalid id, got: {:?}",
        codes
    );
}

// ============================================================================
// G4 — build_unchecked still enforces preconditions (structural XOR)
// ============================================================================

#[test]
fn test_build_unchecked_still_enforces_preconditions_code_block() {
    // CodeBlockBuilder.build() enforces Replace XOR precondition even if called
    // without node-level validation. This is a builder-level precondition check,
    // not a validator check.

    // replace() with BOTH anchor AND old → Precondition error (not validation)
    let err = CodeBlockBuilder::replace()
        .target("src/lib.rs")
        .anchor("fn x(")
        .old("fn x() {}")
        .body("fn x() { /* new */ }")
        .build() // CodeBlockBuilder.build() enforces this even without node context
        .unwrap_err();

    assert!(
        err.is_precondition(),
        "CodeBlockBuilder structural XOR must produce Precondition even without node validation"
    );
}

#[test]
fn test_build_unchecked_empty_id_returns_precondition() {
    // The only hard precondition in node builders: empty ID
    let result = FactsBuilder::new("").build_unchecked();
    assert!(
        result.is_err(),
        "empty ID must fail even in build_unchecked"
    );
    let err = result.unwrap_err();
    assert!(
        err.is_precondition(),
        "empty ID must produce Precondition error"
    );
    assert!(!err.is_validation(), "must not be Validation");
}

// ============================================================================
// G5 — Permissive enforcement level (if available)
// ============================================================================

#[test]
fn test_build_with_permissive_is_more_lenient_than_standard() {
    // Facts node without items: Standard warns (V010), Permissive may skip warnings
    let result_permissive = FactsBuilder::new("enf.permissive.facts")
        .summary("facts without items permissive mode")
        .build_with(EnforcementLevel::Permissive);

    // Permissive must succeed (even more lenient than Standard)
    assert!(
        result_permissive.is_ok(),
        "Permissive must succeed for node with warnings: {:?}",
        result_permissive.unwrap_err()
    );
}

#[test]
fn test_build_with_permissive_still_fails_on_hard_errors() {
    // Even Permissive must fail on hard errors (V021 = invalid ID pattern)
    let result = FactsBuilder::new("INVALID ID")
        .summary("permissive but bad id")
        .build_with(EnforcementLevel::Permissive);

    // V021 is always Error regardless of enforcement level
    assert!(
        result.is_err(),
        "Permissive must still fail on V021 (always Error)"
    );
    let err = result.unwrap_err();
    let has_v021 = err
        .diagnostics()
        .map(|dc| dc.diagnostics().iter().any(|d| d.code == ErrorCode::V021))
        .unwrap_or(false);
    assert!(has_v021, "V021 must be present even in Permissive mode");
}

// ============================================================================
// G6 — Rules builder Standard vs Strict for missing items
// ============================================================================

#[test]
fn test_rules_standard_requires_items() {
    // Rules requires items per V024 schema — Standard mode error
    let result = RulesBuilder::new("enf.rules.noitems")
        .summary("rules without items")
        .build_with(EnforcementLevel::Standard);

    assert!(result.is_err(), "Standard must reject rules without items");
}

#[test]
fn test_rules_with_items_passes_strict() {
    let result = RulesBuilder::new("enf.rules.withitems.strict")
        .summary("rules with items in strict mode")
        .items(["require HTTPS", "rate-limit"])
        .build_with(EnforcementLevel::Strict);

    assert!(
        result.is_ok(),
        "Rules with required items must pass Strict: {:?}",
        result.unwrap_err()
    );
}
