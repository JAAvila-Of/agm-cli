//! Group F — BuildError API surface.
//!
//! Tests all public methods on BuildError: display formatting, diagnostics
//! accessor, predicate methods, and multi-code collection.

use agm_core::builder::{BuildError, FactsBuilder, TicketBuilder};
use agm_core::error::codes::ErrorCode;
use agm_core::error::diagnostic::{AgmError, DiagnosticCollection, ErrorLocation, Severity};

// ---------------------------------------------------------------------------
// Helper: build a DiagnosticCollection with known codes
// ---------------------------------------------------------------------------

fn collection_with_codes(codes: &[ErrorCode]) -> DiagnosticCollection {
    let mut dc = DiagnosticCollection::new("<test>", "");
    for &code in codes {
        dc.push(AgmError::new(
            code,
            format!("test message for {code}"),
            ErrorLocation::default(),
        ));
    }
    dc
}

// ============================================================================
// F1 — Display format for BuildError::Validation
// ============================================================================

#[test]
fn test_build_error_validation_display_format() {
    let dc = collection_with_codes(&[ErrorCode::V021]);
    let err = BuildError::Validation(Box::new(dc));

    let msg = err.to_string();
    assert!(
        msg.contains("1 error"),
        "validation display must mention error count, got: {msg:?}"
    );
}

#[test]
fn test_build_error_validation_display_format_multiple_errors() {
    let dc = collection_with_codes(&[ErrorCode::V021, ErrorCode::V024, ErrorCode::V002]);
    let err = BuildError::Validation(Box::new(dc));

    let msg = err.to_string();
    assert!(
        msg.contains("3 error"),
        "display must mention 3 errors, got: {msg:?}"
    );
}

// ============================================================================
// F2 — Display format for BuildError::Precondition
// ============================================================================

#[test]
fn test_build_error_precondition_display_format() {
    let err = BuildError::Precondition("anchor and old are mutually exclusive".to_owned());
    let msg = err.to_string();
    assert_eq!(
        msg, "anchor and old are mutually exclusive",
        "precondition message must be the literal string"
    );
}

#[test]
fn test_build_error_precondition_display_format_empty_message() {
    let err = BuildError::Precondition(String::new());
    let msg = err.to_string();
    assert_eq!(msg, "");
}

// ============================================================================
// F3 — diagnostics() returns collection for Validation
// ============================================================================

#[test]
fn test_build_error_diagnostics_returns_collection_for_validation() {
    let dc = collection_with_codes(&[ErrorCode::V021]);
    let err = BuildError::Validation(Box::new(dc));

    let dc_ref = err.diagnostics();
    assert!(
        dc_ref.is_some(),
        "diagnostics() must return Some for Validation"
    );

    let dc_ref = dc_ref.unwrap();
    assert_eq!(dc_ref.diagnostics().len(), 1);
    assert_eq!(dc_ref.diagnostics()[0].code, ErrorCode::V021);
}

// ============================================================================
// F4 — diagnostics() returns None for Precondition
// ============================================================================

#[test]
fn test_build_error_diagnostics_returns_none_for_precondition() {
    let err = BuildError::Precondition("structural failure".to_owned());
    assert!(
        err.diagnostics().is_none(),
        "diagnostics() must return None for Precondition"
    );
}

// ============================================================================
// F5 — is_validation() predicate
// ============================================================================

#[test]
fn test_build_error_is_validation_predicate() {
    let dc = collection_with_codes(&[]);
    let validation_err = BuildError::Validation(Box::new(dc));
    let precondition_err = BuildError::Precondition("x".to_owned());

    assert!(
        validation_err.is_validation(),
        "Validation variant must return true"
    );
    assert!(
        !precondition_err.is_validation(),
        "Precondition must return false for is_validation"
    );
}

// ============================================================================
// F6 — is_precondition() predicate
// ============================================================================

#[test]
fn test_build_error_is_precondition_predicate() {
    let dc = collection_with_codes(&[]);
    let validation_err = BuildError::Validation(Box::new(dc));
    let precondition_err = BuildError::Precondition("something wrong".to_owned());

    assert!(
        precondition_err.is_precondition(),
        "Precondition must return true"
    );
    assert!(
        !validation_err.is_precondition(),
        "Validation must return false for is_precondition"
    );
}

// ============================================================================
// F7 — Debug output contains the kind name
// ============================================================================

#[test]
fn test_build_error_debug_contains_kind() {
    let dc = collection_with_codes(&[]);
    let validation_err = BuildError::Validation(Box::new(dc));
    let precondition_err = BuildError::Precondition("nope".to_owned());

    let debug_val = format!("{validation_err:?}");
    let debug_pre = format!("{precondition_err:?}");

    assert!(
        debug_val.contains("Validation"),
        "Debug output must contain 'Validation', got: {debug_val:?}"
    );
    assert!(
        debug_pre.contains("Precondition"),
        "Debug output must contain 'Precondition', got: {debug_pre:?}"
    );
}

// ============================================================================
// F8 — from a real build failure: preserves multiple codes
// ============================================================================

#[test]
fn test_build_error_from_validation_preserves_codes() {
    // Build a ticket missing title, description, AND priority — produces V024 (×3)
    let result = TicketBuilder::new("err.api.multi")
        .summary("incomplete ticket")
        .build();

    assert!(result.is_err(), "incomplete ticket must fail");
    let err = result.unwrap_err();
    assert!(err.is_validation(), "must be Validation error");

    let dc = err.diagnostics().unwrap();
    let codes: Vec<ErrorCode> = dc.diagnostics().iter().map(|d| d.code).collect();

    // Must have at least V024 for missing required fields (title, description, priority)
    let v024_count = codes.iter().filter(|&&c| c == ErrorCode::V024).count();
    assert!(
        v024_count >= 1,
        "must have at least 1 V024 code, got codes: {:?}",
        codes
    );

    // Total error count must be > 0
    assert!(dc.has_errors(), "must report has_errors");
    assert!(dc.error_count() >= 1, "error_count must be ≥ 1");
}

// ============================================================================
// F9 — Multi-code iteration
// ============================================================================

#[test]
fn test_build_error_from_validation_multi_code_iteration() {
    let dc = collection_with_codes(&[ErrorCode::V002, ErrorCode::V021, ErrorCode::V024]);
    let err = BuildError::Validation(Box::new(dc));

    let dc_ref = err.diagnostics().unwrap();
    let codes: Vec<ErrorCode> = dc_ref.diagnostics().iter().map(|d| d.code).collect();

    assert_eq!(codes.len(), 3, "must have exactly 3 diagnostics");
    assert!(codes.contains(&ErrorCode::V002), "V002 must be present");
    assert!(codes.contains(&ErrorCode::V021), "V021 must be present");
    assert!(codes.contains(&ErrorCode::V024), "V024 must be present");
}

// ============================================================================
// F10 — error_count() and has_errors() on DiagnosticCollection
// ============================================================================

#[test]
fn test_build_error_diagnostic_collection_counts_are_accurate() {
    // 2 errors + 1 warning
    let mut dc = DiagnosticCollection::new("<test>", "");
    dc.push(AgmError::new(
        ErrorCode::V021,
        "err1",
        ErrorLocation::default(),
    ));
    dc.push(AgmError::new(
        ErrorCode::V024,
        "err2",
        ErrorLocation::default(),
    ));
    dc.push(AgmError::with_severity(
        ErrorCode::V010,
        Severity::Warning,
        "warn1",
        ErrorLocation::default(),
    ));

    let err = BuildError::Validation(Box::new(dc));
    let dc_ref = err.diagnostics().unwrap();

    assert_eq!(dc_ref.error_count(), 2, "error_count must be 2");
    assert!(dc_ref.has_errors(), "has_errors must be true");
    assert_eq!(dc_ref.diagnostics().len(), 3, "total diagnostics must be 3");
}

// ============================================================================
// F11 — ValidTicket produces Ok, no BuildError
// ============================================================================

#[test]
fn test_build_error_not_produced_for_valid_node() {
    let result = FactsBuilder::new("valid.facts.api")
        .summary("a valid facts node")
        .items(["item one"])
        .build();

    assert!(result.is_ok(), "valid node must not produce BuildError");
}

// ============================================================================
// F12 — Precondition from CodeBlockBuilder mutual-exclusion
// ============================================================================

#[test]
fn test_build_error_precondition_from_code_block_mutual_exclusion() {
    use agm_core::builder::CodeBlockBuilder;

    let err = CodeBlockBuilder::replace()
        .target("src/lib.rs")
        .anchor("fn x(")
        .old("fn x() {}")
        .body("fn x() { /* done */ }")
        .build()
        .unwrap_err();

    assert!(
        err.is_precondition(),
        "both anchor+old must produce Precondition"
    );
    assert!(!err.is_validation(), "must not be Validation");
    assert!(
        err.diagnostics().is_none(),
        "diagnostics must be None for Precondition"
    );
    assert!(
        err.to_string().contains("does not accept `anchor`"),
        "message must mention 'does not accept `anchor`', got: {}",
        err
    );
}
