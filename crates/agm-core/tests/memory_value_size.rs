//! Integration tests for memory value size limit (V027, spec S28.3).
//!
//! Tests use both real fixture `.agm` files and programmatically constructed
//! AGM files with values at and above the 32 KiB boundary.

use agm_core::error::codes::ErrorCode;
use agm_core::memory::schema::MAX_MEMORY_VALUE_BYTES;
use agm_core::parser;
use agm_core::validator::{self, ValidateOptions};

fn fixture(relative: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

fn parse_and_validate(
    src: &str,
    file_name: &str,
) -> agm_core::error::diagnostic::DiagnosticCollection {
    let file = parser::parse(src).expect("parse should succeed");
    validator::validate(&file, src, file_name, &ValidateOptions::default())
}

/// Builds a complete `.agm` file string with a single node containing a
/// memory upsert entry whose value is `value_content`.
///
/// Note: the AGM parser preserves surrounding quotes as part of the value.
/// We emit the value without quotes so byte counts match exactly.
fn build_agm_with_memory_value(value_content: &str) -> String {
    format!(
        "agm: 1.0\n\
         package: test.memory\n\
         version: 0.1.0\n\
         \n\
         node data.size_test\n\
         type: facts\n\
         summary: test memory value size limit\n\
         memory:\n\
         {INDENT}- key: test.large_value\n\
         {INDENT}  topic: infrastructure\n\
         {INDENT}  action: upsert\n\
         {INDENT}  value: {value_content}\n\
         {INDENT}  scope: project\n\
         {INDENT}  ttl: permanent\n",
        INDENT = "  ",
    )
}

// ---------------------------------------------------------------------------
// Fixture-based tests
// ---------------------------------------------------------------------------

#[test]
fn test_memory_value_large_valid_fixture_produces_no_errors() {
    let src = fixture("valid/memory_value_large.agm");
    let result = parse_and_validate(&src, "memory_value_large.agm");
    assert!(
        !result.has_errors(),
        "Expected no errors for valid large memory value, got: {:?}",
        result.diagnostics()
    );
}

// ---------------------------------------------------------------------------
// Programmatic boundary tests with real AGM parsing
// ---------------------------------------------------------------------------

#[test]
fn test_memory_value_at_exact_limit_produces_no_errors() {
    let value = "a".repeat(MAX_MEMORY_VALUE_BYTES);
    let src = build_agm_with_memory_value(&value);
    let result = parse_and_validate(&src, "at_limit.agm");
    let v027_errors: Vec<_> = result
        .diagnostics()
        .iter()
        .filter(|d| d.code == ErrorCode::V027)
        .collect();
    assert!(
        v027_errors.is_empty(),
        "Value at exactly 32 KiB should NOT produce V027, got: {v027_errors:?}"
    );
}

#[test]
fn test_memory_value_one_byte_over_limit_produces_v027() {
    let value = "a".repeat(MAX_MEMORY_VALUE_BYTES + 1);
    let src = build_agm_with_memory_value(&value);
    let result = parse_and_validate(&src, "over_limit.agm");
    assert!(
        result
            .diagnostics()
            .iter()
            .any(|d| d.code == ErrorCode::V027),
        "Value at 32 KiB + 1 byte should produce V027, got: {:?}",
        result.diagnostics()
    );
}

#[test]
fn test_memory_value_way_over_limit_produces_v027() {
    let value = "b".repeat(MAX_MEMORY_VALUE_BYTES * 2);
    let src = build_agm_with_memory_value(&value);
    let result = parse_and_validate(&src, "way_over.agm");
    assert!(
        result
            .diagnostics()
            .iter()
            .any(|d| d.code == ErrorCode::V027),
        "Value at 64 KiB should produce V027, got: {:?}",
        result.diagnostics()
    );
}

#[test]
fn test_memory_value_v027_message_contains_byte_counts() {
    let value = "c".repeat(MAX_MEMORY_VALUE_BYTES + 500);
    let src = build_agm_with_memory_value(&value);
    let result = parse_and_validate(&src, "message_check.agm");
    let v027 = result
        .diagnostics()
        .iter()
        .find(|d| d.code == ErrorCode::V027)
        .expect("should produce V027");
    assert!(
        v027.message.contains("exceeds maximum size"),
        "V027 message should mention size: {}",
        v027.message
    );
    assert!(
        v027.message
            .contains(&format!("{}", MAX_MEMORY_VALUE_BYTES)),
        "V027 message should mention the limit ({}): {}",
        MAX_MEMORY_VALUE_BYTES,
        v027.message
    );
}

#[test]
fn test_memory_value_v027_is_severity_error() {
    assert_eq!(
        ErrorCode::V027.default_severity(),
        agm_core::error::diagnostic::Severity::Error
    );
}

#[test]
fn test_memory_value_empty_does_not_trigger_v027() {
    let src = build_agm_with_memory_value("");
    let result = parse_and_validate(&src, "empty_value.agm");
    let v027_errors: Vec<_> = result
        .diagnostics()
        .iter()
        .filter(|d| d.code == ErrorCode::V027)
        .collect();
    assert!(
        v027_errors.is_empty(),
        "Empty value should not produce V027, got: {v027_errors:?}"
    );
}

#[test]
fn test_memory_value_just_under_limit_does_not_trigger_v027() {
    let value = "d".repeat(MAX_MEMORY_VALUE_BYTES - 1);
    let src = build_agm_with_memory_value(&value);
    let result = parse_and_validate(&src, "under_limit.agm");
    let v027_errors: Vec<_> = result
        .diagnostics()
        .iter()
        .filter(|d| d.code == ErrorCode::V027)
        .collect();
    assert!(
        v027_errors.is_empty(),
        "Value at 32 KiB - 1 byte should not produce V027, got: {v027_errors:?}"
    );
}

// ---------------------------------------------------------------------------
// Multi-byte UTF-8 boundary test
// ---------------------------------------------------------------------------

#[test]
fn test_memory_value_multibyte_utf8_counts_bytes_not_chars() {
    // Each emoji is 4 bytes in UTF-8. 8193 emojis = 32772 bytes > 32768 limit.
    let value = "\u{1F600}".repeat(MAX_MEMORY_VALUE_BYTES / 4 + 1);
    assert!(
        value.len() > MAX_MEMORY_VALUE_BYTES,
        "precondition: value should exceed byte limit"
    );
    let src = build_agm_with_memory_value(&value);
    let result = parse_and_validate(&src, "utf8_boundary.agm");
    assert!(
        result
            .diagnostics()
            .iter()
            .any(|d| d.code == ErrorCode::V027),
        "Multi-byte UTF-8 value exceeding byte limit should produce V027"
    );
}
