//! Parser error-path coverage for `parser/node.rs`, `parser/fields.rs`,
//! `parser/header.rs`.
//!
//! Exercises:
//!  * Invalid enum values (priority / stability / confidence / status /
//!    execution_status / retry_count) → P003
//!  * Duplicate fields in header and node → P006
//!  * Unknown header / node fields → graceful skip via `skip_field_body`
//!  * BodyMarker duplicates
//!  * Missing required header fields → P001
//!  * Invalid import entries → P001
//!  * Unexpected indentation in node → P003
//!  * Block / list parsing edge cases (comments embedded in lists, blank
//!    lines bridging block lines, trailing blanks stripped)

use agm_core::error::ErrorCode;
use agm_core::parser::parse;

#[test]
fn test_parser_invalid_priority_emits_p003() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
priority: super-urgent
"#;
    let errors = parse(input).unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::P003),
        "expected P003 for invalid priority, got: {errors:?}"
    );
}

#[test]
fn test_parser_invalid_stability_emits_p003() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
stability: unknown
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
}

#[test]
fn test_parser_invalid_confidence_emits_p003() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
confidence: maybe
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
}

#[test]
fn test_parser_invalid_node_status_emits_p003() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
status: not-real
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
}

#[test]
fn test_parser_invalid_execution_status_emits_p003() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
execution_status: never-ran
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
}

#[test]
fn test_parser_invalid_retry_count_emits_p003() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
retry_count: lots
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
}

#[test]
fn test_parser_duplicate_scalar_in_node_emits_p006() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: first
summary: second
"#;
    let errors = parse(input).unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::P006),
        "expected P006 for duplicate summary, got {errors:?}"
    );
}

#[test]
fn test_parser_duplicate_inline_list_in_node_emits_p006() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
tags: [a]
tags: [b]
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P006));
}

#[test]
fn test_parser_duplicate_field_start_in_node_emits_p006() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
items:
  - one
items:
  - two
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P006));
}

#[test]
fn test_parser_duplicate_scalar_in_header_emits_p006() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0
owner: first
owner: second

node test.node
type: facts
summary: s
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P006));
}

#[test]
fn test_parser_duplicate_inline_list_in_header_emits_p006() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0
tags: [a]
tags: [b]

node test.node
type: facts
summary: s
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P006));
}

#[test]
fn test_parser_duplicate_field_start_in_header_emits_p006() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0
description:
  Line one
description:
  Line two

node test.node
type: facts
summary: s
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P006));
}

#[test]
fn test_parser_unknown_header_field_start_is_skipped_without_error() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0
unknown_field:
  child_a: 1
  child_b: 2

node test.node
type: facts
summary: s
"#;
    let result = parse(input);
    // Unknown header fields must not produce a parse error (forward-compat).
    assert!(result.is_ok(), "unexpected errors: {result:?}");
}

#[test]
fn test_parser_missing_header_fields_emit_p001() {
    // No `package` or `version` → at least two P001 errors.
    let input = r#"agm: 1.0

node test.node
type: facts
summary: s
"#;
    let errors = parse(input).unwrap_err();
    let p001_count = errors.iter().filter(|e| e.code == ErrorCode::P001).count();
    assert!(
        p001_count >= 2,
        "expected >=2 P001 errors, got {p001_count}: {errors:?}"
    );
}

#[test]
fn test_parser_invalid_import_entry_emits_p001() {
    // Empty package before '@' is rejected by `ImportEntry::from_str`.
    let input = "agm: 1.0\npackage: test.pkg\nversion: 1.0.0\nimports:\n  - @^1.0.0\n\nnode test.node\ntype: facts\nsummary: s\n";
    let errors = parse(input).unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::P001),
        "expected P001 for invalid import, got {errors:?}"
    );
}

#[test]
fn test_parser_unexpected_indentation_in_node_emits_p003() {
    let input = "agm: 1.0\npackage: test.pkg\nversion: 1.0.0\n\nnode test.node\ntype: facts\nsummary: s\n  - spurious list item at wrong indent\n";
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
}

#[test]
fn test_parser_body_marker_assigns_to_detail() {
    // A bare `body: |` in a node populates detail (the canonical target).
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
body: |
  line one
  line two
"#;
    let file = parse(input).expect("must parse");
    assert_eq!(
        file.nodes[0].detail.as_deref(),
        Some("line one\nline two")
    );
}

#[test]
fn test_parser_body_marker_second_goes_to_extra_fields_body() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
detail: existing detail
body: |
  some body content
"#;
    let file = parse(input).expect("must parse");
    // detail stays — body lands in extra_fields
    assert_eq!(file.nodes[0].detail.as_deref(), Some("existing detail"));
    assert!(file.nodes[0].extra_fields.contains_key("body"));
}

#[test]
fn test_parser_block_field_with_internal_blank_line_preserved() {
    // Blank line inside a block is preserved if more block content follows.
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0
description:
  line one

  line three

node test.node
type: facts
summary: s
"#;
    let file = parse(input).expect("must parse");
    let desc = file.header.description.as_deref().unwrap();
    assert!(desc.contains("line one"));
    assert!(desc.contains("line three"));
}

#[test]
fn test_parser_indented_list_with_embedded_comments_is_consumed() {
    // Comments inside a list are skipped without breaking parsing.
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
items:
  - one
  # inline comment
  - two
"#;
    let file = parse(input).expect("must parse");
    let items = file.nodes[0].items.as_deref().unwrap();
    assert_eq!(items, &["one", "two"]);
}

#[test]
fn test_parser_unknown_structured_field_captured_as_extra_block() {
    // An unknown FieldStart at node level lands in extra_fields as Block.
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node test.node
type: facts
summary: s
custom_block:
  arbitrary: content
  more: lines
"#;
    let file = parse(input).expect("must parse");
    assert!(file.nodes[0].extra_fields.contains_key("custom_block"));
}

#[test]
fn test_parser_invalid_node_id_emits_p002() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node Bad_ID
type: facts
summary: s
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P002));
}

#[test]
fn test_parser_empty_node_id_emits_p002() {
    let input = r#"agm: 1.0
package: test.pkg
version: 1.0.0

node
type: facts
summary: s
"#;
    let errors = parse(input).unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::P002));
}
