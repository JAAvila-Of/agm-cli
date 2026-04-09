//! Parser: converts raw `.agm` text into an unvalidated AST.
//!
//! The parser is hand-written and line-oriented. Each line is classified
//! (scalar field, list, block, node declaration, comment, blank) and
//! indentation tracking determines field boundaries.

pub mod fields;
pub mod header;
pub mod lexer;
pub mod mem;
pub mod node;
pub mod sidecar;
pub mod state;
pub mod structured;

pub use lexer::{Line, LineKind, classify_line, lex};

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::file::AgmFile;

/// Result type for parser operations.
pub type ParseResult<T> = Result<T, Vec<AgmError>>;

/// Parses raw AGM source text into an unvalidated `AgmFile`.
pub fn parse(input: &str) -> ParseResult<AgmFile> {
    let lines = lex(input)?;
    let mut pos = 0;
    let mut errors = Vec::new();

    let header = header::parse_header(&lines, &mut pos, &mut errors);

    let mut nodes = Vec::new();
    while pos < lines.len() {
        match &lines[pos].kind {
            LineKind::Blank | LineKind::Comment | LineKind::TestExpectHeader(_) => {
                pos += 1;
            }
            LineKind::NodeDeclaration(_) => {
                nodes.push(node::parse_node(&lines, &mut pos, &mut errors));
            }
            _ => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Unexpected content at line {}", lines[pos].number),
                    ErrorLocation::new(None, Some(lines[pos].number), None),
                ));
                pos += 1;
            }
        }
    }

    if nodes.is_empty() {
        errors.push(AgmError::new(
            ErrorCode::P008,
            "Empty file (no nodes)",
            ErrorLocation::new(None, None, None),
        ));
    }

    if errors.iter().any(|e| e.is_error()) {
        Err(errors)
    } else {
        Ok(AgmFile { header, nodes })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::model::fields::{FieldValue, NodeType, Priority};

    // -----------------------------------------------------------------------
    // Helper: minimal valid AGM input
    // -----------------------------------------------------------------------

    fn minimal_valid(node_id: &str, node_type: &str) -> String {
        format!(
            "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode {node_id}\ntype: {node_type}\nsummary: A test node\n"
        )
    }

    fn errors_contain(errors: &[AgmError], code: ErrorCode) -> bool {
        errors.iter().any(|e| e.code == code)
    }

    // -----------------------------------------------------------------------
    // A: Minimal valid files
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_minimal_valid_file_returns_ok() {
        let input = minimal_valid("test.node", "facts");
        let result = parse(&input);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        let file = result.unwrap();
        assert_eq!(file.nodes.len(), 1);
        assert_eq!(file.nodes[0].id, "test.node");
    }

    #[test]
    fn test_parse_minimal_header_and_empty_node_returns_ok() {
        // Node without type/summary still parses (validator catches it).
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode bare.node\n";
        let result = parse(input);
        // Parser accepts it; validator (Step 6) would flag missing type/summary.
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        let file = result.unwrap();
        assert_eq!(file.nodes.len(), 1);
        assert_eq!(file.nodes[0].id, "bare.node");
    }

    #[test]
    fn test_parse_multiple_nodes_returns_all() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node a.one\ntype: facts\nsummary: one\n\n\
            node a.two\ntype: rules\nsummary: two\n\n\
            node a.three\ntype: workflow\nsummary: three\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes.len(), 3);
        assert_eq!(file.nodes[0].id, "a.one");
        assert_eq!(file.nodes[1].id, "a.two");
        assert_eq!(file.nodes[2].id, "a.three");
    }

    // -----------------------------------------------------------------------
    // B: Header validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_agm_valid_format_accepted() {
        let input = minimal_valid("n.node", "facts");
        let file = parse(&input).unwrap();
        assert_eq!(file.header.agm, "1.0");
    }

    #[test]
    fn test_parse_agm_invalid_format_returns_p001() {
        let input = "agm: latest\npackage: test.pkg\nversion: 0.1.0\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
    }

    #[test]
    fn test_parse_agm_three_part_version_returns_p001() {
        let input = "agm: 1.0.0\npackage: test.pkg\nversion: 0.1.0\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
    }

    #[test]
    fn test_parse_package_valid_dotted_accepted() {
        let input = minimal_valid("n.node", "facts");
        let file = parse(&input).unwrap();
        assert_eq!(file.header.package, "test.pkg");
    }

    #[test]
    fn test_parse_package_uppercase_returns_p001() {
        let input =
            "agm: 1.0\npackage: Test.pkg\nversion: 0.1.0\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
    }

    #[test]
    fn test_parse_package_with_hyphen_returns_p001() {
        let input =
            "agm: 1.0\npackage: test-pkg\nversion: 0.1.0\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
    }

    #[test]
    fn test_parse_version_valid_semver_accepted() {
        let input = minimal_valid("n.node", "facts");
        let file = parse(&input).unwrap();
        assert_eq!(file.header.version, "0.1.0");
    }

    #[test]
    fn test_parse_version_invalid_semver_returns_p001() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: not-a-version\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
    }

    // -----------------------------------------------------------------------
    // C: Missing required header fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_missing_agm_returns_p001() {
        let input = "package: test.pkg\nversion: 0.1.0\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("'agm'"))
        );
    }

    #[test]
    fn test_parse_missing_package_returns_p001() {
        let input = "agm: 1.0\nversion: 0.1.0\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("'package'"))
        );
    }

    #[test]
    fn test_parse_missing_version_returns_p001() {
        let input = "agm: 1.0\npackage: test.pkg\n\nnode n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("'version'"))
        );
    }

    // -----------------------------------------------------------------------
    // D: Imports
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_imports_inline_with_constraints() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\
            imports: [shared.security@^1.0.0, core.utils]\n\n\
            node n.node\ntype: facts\nsummary: s\n";
        let file = parse(input).unwrap();
        let imports = file.header.imports.unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].package, "shared.security");
        assert_eq!(imports[0].version_constraint.as_deref(), Some("^1.0.0"));
        assert_eq!(imports[1].package, "core.utils");
    }

    #[test]
    fn test_parse_imports_indented_list() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\
            imports:\n  - shared.security@^1.0.0\n  - core.utils\n\n\
            node n.node\ntype: facts\nsummary: s\n";
        let file = parse(input).unwrap();
        let imports = file.header.imports.unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].package, "shared.security");
    }

    #[test]
    fn test_parse_imports_empty_list() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\
            imports: []\n\n\
            node n.node\ntype: facts\nsummary: s\n";
        let file = parse(input).unwrap();
        let imports = file.header.imports.unwrap();
        assert_eq!(imports.len(), 0);
    }

    #[test]
    fn test_parse_imports_invalid_entry_returns_error() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\
            imports: [@bad]\n\n\
            node n.node\ntype: facts\nsummary: s\n";
        // Invalid entries produce P001 errors but parsing continues.
        // The file may still parse successfully if no other hard errors occur.
        let result = parse(input);
        match result {
            Ok(file) => {
                let imports = file.header.imports.unwrap();
                assert_eq!(imports.len(), 0); // bad entry was rejected
            }
            Err(errors) => {
                assert!(errors_contain(&errors, ErrorCode::P001));
            }
        }
    }

    // -----------------------------------------------------------------------
    // E: Scalar fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_node_scalar_type_and_summary() {
        let input = minimal_valid("auth.login", "workflow");
        let file = parse(&input).unwrap();
        assert_eq!(file.nodes[0].node_type, NodeType::Workflow);
        assert_eq!(file.nodes[0].summary, "A test node");
    }

    #[test]
    fn test_parse_node_scalar_priority_valid() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\npriority: critical\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes[0].priority, Some(Priority::Critical));
    }

    #[test]
    fn test_parse_header_scalar_title_and_owner() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\
            title: My Package\nowner: team@example.com\n\n\
            node n.node\ntype: facts\nsummary: s\n";
        let file = parse(input).unwrap();
        assert_eq!(file.header.title.as_deref(), Some("My Package"));
        assert_eq!(file.header.owner.as_deref(), Some("team@example.com"));
    }

    // -----------------------------------------------------------------------
    // F: Inline lists
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_node_inline_list_tags() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\ntags: [auth, security]\n";
        let file = parse(input).unwrap();
        assert_eq!(
            file.nodes[0].tags.as_deref(),
            Some(vec!["auth".to_owned(), "security".to_owned()].as_slice())
        );
    }

    #[test]
    fn test_parse_node_inline_list_depends() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: workflow\nsummary: s\ndepends: [a.one, a.two]\n";
        let file = parse(input).unwrap();
        assert_eq!(
            file.nodes[0].depends.as_deref(),
            Some(vec!["a.one".to_owned(), "a.two".to_owned()].as_slice())
        );
    }

    // -----------------------------------------------------------------------
    // G: Indented lists
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_node_indented_list_items() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\nitems:\n  - item one\n  - item two\n";
        let file = parse(input).unwrap();
        assert_eq!(
            file.nodes[0].items.as_deref(),
            Some(vec!["item one".to_owned(), "item two".to_owned()].as_slice())
        );
    }

    #[test]
    fn test_parse_node_indented_list_with_blanks_between() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\nitems:\n  - item one\n\n  - item two\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes[0].items.as_ref().unwrap().len(), 2);
    }

    // -----------------------------------------------------------------------
    // H: Block fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_node_block_detail() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\ndetail:\n  This is the detail.\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes[0].detail.as_deref(), Some("This is the detail."));
    }

    #[test]
    fn test_parse_node_block_preserves_internal_blanks() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\ndetail:\n  line one\n\n  line two\n";
        let file = parse(input).unwrap();
        let detail = file.nodes[0].detail.as_deref().unwrap();
        assert!(detail.contains('\n'), "expected internal newline");
        assert!(detail.contains("line one"));
        assert!(detail.contains("line two"));
    }

    #[test]
    fn test_parse_node_block_strips_base_indent() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\ndetail:\n    four spaces\n    second line\n";
        let file = parse(input).unwrap();
        let detail = file.nodes[0].detail.as_deref().unwrap();
        assert!(!detail.starts_with(' '), "leading spaces not stripped");
        assert!(detail.starts_with("four"));
    }

    // -----------------------------------------------------------------------
    // I: Node boundaries and IDs
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_two_nodes_boundary_correct() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node a.one\ntype: facts\nsummary: first\n\n\
            node a.two\ntype: rules\nsummary: second\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes.len(), 2);
        assert_eq!(file.nodes[0].id, "a.one");
        assert_eq!(file.nodes[1].id, "a.two");
    }

    #[test]
    fn test_parse_node_id_dotted_valid() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node billing.invoice.create\ntype: facts\nsummary: s\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes[0].id, "billing.invoice.create");
    }

    #[test]
    fn test_parse_node_id_uppercase_returns_p002() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node Auth.Login\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P002));
    }

    #[test]
    fn test_parse_node_id_empty_returns_p002() {
        let input =
            "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P002));
    }

    // -----------------------------------------------------------------------
    // J: Duplicate fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_node_duplicate_scalar_returns_p006() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: first\nsummary: second\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P006));
    }

    #[test]
    fn test_parse_node_duplicate_list_returns_p006() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\ntags: [a]\ntags: [b]\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P006));
    }

    #[test]
    fn test_parse_header_duplicate_field_returns_p006() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\ntitle: A\ntitle: B\n\n\
            node n.node\ntype: facts\nsummary: s\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P006));
    }

    // -----------------------------------------------------------------------
    // K: Unknown fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_node_unknown_scalar_stored_in_extra() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\ncustom_field: some value\n";
        let file = parse(input).unwrap();
        assert_eq!(
            file.nodes[0].extra_fields.get("custom_field"),
            Some(&FieldValue::Scalar("some value".to_owned()))
        );
    }

    #[test]
    fn test_parse_node_unknown_list_stored_in_extra() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\ncustom_list: [x, y]\n";
        let file = parse(input).unwrap();
        assert_eq!(
            file.nodes[0].extra_fields.get("custom_list"),
            Some(&FieldValue::List(vec!["x".to_owned(), "y".to_owned()]))
        );
    }

    // -----------------------------------------------------------------------
    // L: Comments and blanks
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_comments_between_fields_skipped() {
        let input = "agm: 1.0\n# a comment\npackage: test.pkg\n# another\nversion: 0.1.0\n\n\
            node n.node\n# comment inside node\ntype: facts\nsummary: s\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes[0].summary, "s");
    }

    #[test]
    fn test_parse_blanks_between_nodes_skipped() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\n\n\
            node a.one\ntype: facts\nsummary: first\n\n\n\
            node a.two\ntype: rules\nsummary: second\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes.len(), 2);
    }

    // -----------------------------------------------------------------------
    // M: Spans
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_node_span_correct_single_node() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\n";
        let file = parse(input).unwrap();
        let span = &file.nodes[0].span;
        assert!(span.start_line > 0);
        assert!(span.end_line >= span.start_line);
    }

    #[test]
    fn test_parse_node_span_correct_multiple_nodes() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node a.one\ntype: facts\nsummary: first\n\n\
            node a.two\ntype: rules\nsummary: second\n";
        let file = parse(input).unwrap();
        let span0 = &file.nodes[0].span;
        let span1 = &file.nodes[1].span;
        assert!(span0.start_line < span1.start_line);
        assert!(span0.end_line < span1.start_line);
    }

    // -----------------------------------------------------------------------
    // N: Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_no_nodes_returns_p008() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n";
        let errors = parse(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P008));
    }

    #[test]
    fn test_parse_structured_field_parsed_into_typed_field() {
        // After Step 6, `verify:` is parsed into node.verify (not extra_fields).
        // An entry with an unknown type is skipped with an error, but parsing succeeds
        // as long as there are no hard parse errors.
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\nverify:\n  - type: command\n    run: cargo check\n";
        let file = parse(input).unwrap();
        // `verify` is structured — parsed into node.verify, not extra_fields.
        assert!(!file.nodes[0].extra_fields.contains_key("verify"));
        assert!(file.nodes[0].verify.is_some());
    }

    #[test]
    fn test_parse_body_marker_assigns_to_detail() {
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
            node n.node\ntype: facts\nsummary: s\nbody: |\n  This is body text.\n";
        let file = parse(input).unwrap();
        assert_eq!(file.nodes[0].detail.as_deref(), Some("This is body text."));
    }
}
