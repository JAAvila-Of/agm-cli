//! Lexer: classifies each line of an AGM source file into a `LineKind`.
//!
//! This is a hand-written, line-oriented lexer. No parser combinators are used.
//! Rules are applied in a strict priority order; the first match wins.

use crate::error::{AgmError, ErrorCode, ErrorLocation};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Classification of a single line in an AGM source file.
#[derive(Debug, Clone, PartialEq)]
pub enum LineKind {
    Blank,
    Comment,
    NodeDeclaration(String),
    ScalarField(String, String),
    InlineListField(String, Vec<String>),
    FieldStart(String),
    ListItem(String),
    IndentedLine(String),
    BodyMarker,
    TestExpectHeader(String),
}

/// A single classified line from an AGM source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub kind: LineKind,
    pub number: usize,
    pub indent: usize,
    pub raw: String,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Returns the byte position of the first tab character, or `None`.
fn find_tab(s: &str) -> Option<usize> {
    s.bytes().position(|b| b == b'\t')
}

/// Counts the number of leading ASCII spaces.
fn count_indent(s: &str) -> usize {
    s.bytes().take_while(|&b| b == b' ').count()
}

/// Returns `true` if `key` matches `[a-zA-Z_][a-zA-Z0-9_]*` and is non-empty.
fn is_valid_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parses an inline list value such as `[a, b, c]`.
///
/// Strips outer `[]`, splits by `,`, trims each item, and filters empty strings.
/// Returns `Err` with P007 if the value starts with `[` but does not end with `]`.
fn parse_inline_list(value: &str, line_number: usize) -> Result<Vec<String>, AgmError> {
    // value has already been confirmed to start with '[' by caller
    if !value.ends_with(']') {
        return Err(AgmError::new(
            ErrorCode::P007,
            "Invalid inline list syntax",
            ErrorLocation::new(None, Some(line_number), None),
        ));
    }
    let inner = &value[1..value.len() - 1];
    let items: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(items)
}

// ---------------------------------------------------------------------------
// Core classification
// ---------------------------------------------------------------------------

/// Classifies a single raw line.
///
/// Rules are applied in strict priority order; the first match wins.
pub fn classify_line(raw: &str, line_number: usize) -> Result<Line, AgmError> {
    // Rule 1 — Tab check
    if find_tab(raw).is_some() {
        return Err(AgmError::new(
            ErrorCode::P004,
            "Tab character in indentation (spaces required)",
            ErrorLocation::new(None, Some(line_number), None),
        ));
    }

    let trimmed = raw.trim();
    let indent = count_indent(raw);

    // Rule 2 — Blank
    if trimmed.is_empty() {
        return Ok(Line {
            kind: LineKind::Blank,
            number: line_number,
            indent: 0,
            raw: raw.to_string(),
        });
    }

    // Rule 3 — TestExpectHeader: starts with "# expect:"
    if let Some(rest) = trimmed.strip_prefix("# expect:") {
        return Ok(Line {
            kind: LineKind::TestExpectHeader(rest.trim().to_string()),
            number: line_number,
            indent: 0,
            raw: raw.to_string(),
        });
    }

    // Rule 4 — Comment: starts with '#'
    if trimmed.starts_with('#') {
        return Ok(Line {
            kind: LineKind::Comment,
            number: line_number,
            indent,
            raw: raw.to_string(),
        });
    }

    // Rule 5 — NodeDeclaration: trimmed == "node" OR starts with "node "
    if trimmed == "node" || trimmed.starts_with("node ") {
        let id = if trimmed == "node" {
            ""
        } else {
            trimmed["node ".len()..].trim()
        };
        return Ok(Line {
            kind: LineKind::NodeDeclaration(id.to_string()),
            number: line_number,
            indent,
            raw: raw.to_string(),
        });
    }

    // Rule 6 — BodyMarker: starts with "body:" AND rest after "body:" trimmed == "|"
    if let Some(rest) = trimmed.strip_prefix("body:") {
        if rest.trim() == "|" {
            return Ok(Line {
                kind: LineKind::BodyMarker,
                number: line_number,
                indent,
                raw: raw.to_string(),
            });
        }
        // Fall through to field rules below
    }

    // Rules 7-9: colon-based field rules
    if let Some(colon_pos) = raw.find(':') {
        let key_raw = &raw[..colon_pos];
        let key = key_raw.trim();
        let value_raw = &raw[colon_pos + 1..];
        let value = value_raw.trim();

        if is_valid_field_key(key) {
            // Rule 7 — InlineListField: value starts with '['
            if value.starts_with('[') {
                if !value.ends_with(']') {
                    return Err(AgmError::new(
                        ErrorCode::P007,
                        "Invalid inline list syntax",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                }
                let items = parse_inline_list(value, line_number)?;
                return Ok(Line {
                    kind: LineKind::InlineListField(key.to_string(), items),
                    number: line_number,
                    indent,
                    raw: raw.to_string(),
                });
            }

            // Rule 8 — ScalarField: value is non-empty
            if !value.is_empty() {
                return Ok(Line {
                    kind: LineKind::ScalarField(key.to_string(), value.to_string()),
                    number: line_number,
                    indent,
                    raw: raw.to_string(),
                });
            }

            // Rule 9 — FieldStart: value is empty
            return Ok(Line {
                kind: LineKind::FieldStart(key.to_string()),
                number: line_number,
                indent,
                raw: raw.to_string(),
            });
        }
    }

    // Rule 10 — ListItem: raw stripped of leading spaces starts with "- " or equals "-"
    let stripped = raw.trim_start_matches(' ');
    if stripped.starts_with("- ") || stripped == "-" {
        let value = if stripped == "-" {
            ""
        } else {
            &stripped["- ".len()..]
        };
        return Ok(Line {
            kind: LineKind::ListItem(value.to_string()),
            number: line_number,
            indent,
            raw: raw.to_string(),
        });
    }

    // Rule 11 — IndentedLine: indent > 0
    if indent > 0 {
        return Ok(Line {
            kind: LineKind::IndentedLine(trimmed.to_string()),
            number: line_number,
            indent,
            raw: raw.to_string(),
        });
    }

    // Rule 12 — Fallback
    Ok(Line {
        kind: LineKind::IndentedLine(trimmed.to_string()),
        number: line_number,
        indent: 0,
        raw: raw.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Lex function
// ---------------------------------------------------------------------------

/// Lexes an entire AGM source string into a vector of classified lines.
///
/// Returns `Ok(lines)` if all lines are valid, or `Err(errors)` listing every
/// line that failed classification.
pub fn lex(input: &str) -> Result<Vec<Line>, Vec<AgmError>> {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for (idx, raw_line) in input.lines().enumerate() {
        match classify_line(raw_line, idx + 1) {
            Ok(line) => lines.push(line),
            Err(err) => errors.push(err),
        }
    }
    if errors.is_empty() {
        Ok(lines)
    } else {
        Err(errors)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    // ---- A: Blank Lines ----

    #[test]
    fn test_classify_empty_string_returns_blank() {
        let line = classify_line("", 1).unwrap();
        assert_eq!(line.kind, LineKind::Blank);
        assert_eq!(line.indent, 0);
    }

    #[test]
    fn test_classify_spaces_only_returns_blank() {
        let line = classify_line("   ", 1).unwrap();
        assert_eq!(line.kind, LineKind::Blank);
        assert_eq!(line.indent, 0);
    }

    #[test]
    fn test_classify_single_space_returns_blank() {
        let line = classify_line(" ", 1).unwrap();
        assert_eq!(line.kind, LineKind::Blank);
        assert_eq!(line.indent, 0);
    }

    // ---- B: Comments ----

    #[test]
    fn test_classify_hash_comment_returns_comment() {
        let line = classify_line("# comment", 1).unwrap();
        assert_eq!(line.kind, LineKind::Comment);
    }

    #[test]
    fn test_classify_hash_only_returns_comment() {
        let line = classify_line("#", 1).unwrap();
        assert_eq!(line.kind, LineKind::Comment);
    }

    #[test]
    fn test_classify_indented_comment_returns_comment_with_indent() {
        let line = classify_line("  # indented comment", 1).unwrap();
        assert_eq!(line.kind, LineKind::Comment);
        assert_eq!(line.indent, 2);
    }

    // ---- C: TestExpectHeader ----

    #[test]
    fn test_classify_expect_header_with_content_returns_test_expect_header() {
        let line = classify_line("# expect: error AGM-P004", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::TestExpectHeader("error AGM-P004".to_string())
        );
    }

    #[test]
    fn test_classify_expect_header_empty_rest_returns_test_expect_header() {
        let line = classify_line("# expect:", 1).unwrap();
        assert_eq!(line.kind, LineKind::TestExpectHeader("".to_string()));
    }

    #[test]
    fn test_classify_expect_without_space_returns_comment_not_test_expect() {
        let line = classify_line("#expect: foo", 1).unwrap();
        assert_eq!(line.kind, LineKind::Comment);
    }

    // ---- D: Node Declarations ----

    #[test]
    fn test_classify_node_with_id_returns_node_declaration() {
        let line = classify_line("node auth.login", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::NodeDeclaration("auth.login".to_string())
        );
    }

    #[test]
    fn test_classify_node_with_dotted_id_returns_node_declaration() {
        let line = classify_line("node billing.invoice.create", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::NodeDeclaration("billing.invoice.create".to_string())
        );
    }

    #[test]
    fn test_classify_node_alone_returns_node_declaration_empty_id() {
        let line = classify_line("node", 1).unwrap();
        assert_eq!(line.kind, LineKind::NodeDeclaration("".to_string()));
    }

    #[test]
    fn test_classify_node_with_extra_spaces_trims_id() {
        let line = classify_line("node   auth.login  ", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::NodeDeclaration("auth.login".to_string())
        );
    }

    // ---- E: BodyMarker ----

    #[test]
    fn test_classify_body_pipe_returns_body_marker() {
        let line = classify_line("body: |", 1).unwrap();
        assert_eq!(line.kind, LineKind::BodyMarker);
    }

    #[test]
    fn test_classify_body_pipe_with_spaces_returns_body_marker() {
        let line = classify_line("body:  |  ", 1).unwrap();
        assert_eq!(line.kind, LineKind::BodyMarker);
    }

    #[test]
    fn test_classify_body_pipe_with_suffix_returns_scalar_field() {
        let line = classify_line("body: |something", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::ScalarField("body".to_string(), "|something".to_string())
        );
    }

    // ---- F: Inline Lists ----

    #[test]
    fn test_classify_inline_list_multiple_items_returns_inline_list_field() {
        let line = classify_line("tags: [auth, security]", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::InlineListField(
                "tags".to_string(),
                vec!["auth".to_string(), "security".to_string()]
            )
        );
    }

    #[test]
    fn test_classify_inline_list_single_item_returns_inline_list_field() {
        let line = classify_line("tags: [auth]", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::InlineListField("tags".to_string(), vec!["auth".to_string()])
        );
    }

    #[test]
    fn test_classify_inline_list_empty_returns_inline_list_field_empty() {
        let line = classify_line("tags: []", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::InlineListField("tags".to_string(), vec![])
        );
    }

    #[test]
    fn test_classify_inline_list_unclosed_returns_err_p007() {
        let err = classify_line("tags: [auth, security", 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::P007);
    }

    // ---- G: Scalar Fields ----

    #[test]
    fn test_classify_scalar_field_simple_returns_scalar_field() {
        let line = classify_line("type: workflow", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::ScalarField("type".to_string(), "workflow".to_string())
        );
    }

    #[test]
    fn test_classify_scalar_field_with_colon_in_value_keeps_rest() {
        let line = classify_line("summary: Rule: no tabs allowed", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::ScalarField("summary".to_string(), "Rule: no tabs allowed".to_string())
        );
    }

    #[test]
    fn test_classify_scalar_field_trims_value_whitespace() {
        let line = classify_line("type:   workflow  ", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::ScalarField("type".to_string(), "workflow".to_string())
        );
    }

    // ---- H: Field Start ----

    #[test]
    fn test_classify_field_start_no_value_returns_field_start() {
        let line = classify_line("items:", 1).unwrap();
        assert_eq!(line.kind, LineKind::FieldStart("items".to_string()));
    }

    #[test]
    fn test_classify_field_start_with_trailing_spaces_returns_field_start() {
        let line = classify_line("items:   ", 1).unwrap();
        assert_eq!(line.kind, LineKind::FieldStart("items".to_string()));
    }

    // ---- I: List Items ----

    #[test]
    fn test_classify_list_item_with_content_returns_list_item_with_indent() {
        let line = classify_line("  - first item", 1).unwrap();
        assert_eq!(line.kind, LineKind::ListItem("first item".to_string()));
        assert_eq!(line.indent, 2);
    }

    #[test]
    fn test_classify_list_item_dash_only_returns_list_item_empty() {
        let line = classify_line("  -", 1).unwrap();
        assert_eq!(line.kind, LineKind::ListItem("".to_string()));
        assert_eq!(line.indent, 2);
    }

    #[test]
    fn test_classify_list_item_no_space_after_dash_returns_indented_line() {
        let line = classify_line("  -value", 1).unwrap();
        assert_eq!(line.kind, LineKind::IndentedLine("-value".to_string()));
        assert_eq!(line.indent, 2);
    }

    // ---- J: Indented Lines ----

    #[test]
    fn test_classify_indented_text_returns_indented_line_with_indent() {
        let line = classify_line("  Some block text", 1).unwrap();
        assert_eq!(
            line.kind,
            LineKind::IndentedLine("Some block text".to_string())
        );
        assert_eq!(line.indent, 2);
    }

    #[test]
    fn test_classify_deeply_indented_text_returns_indented_line() {
        let line = classify_line("      deep text", 1).unwrap();
        assert_eq!(line.kind, LineKind::IndentedLine("deep text".to_string()));
        assert_eq!(line.indent, 6);
    }

    // ---- K: Tab Rejection ----

    #[test]
    fn test_classify_tab_at_start_returns_err_p004() {
        let err = classify_line("\ttype: workflow", 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::P004);
    }

    #[test]
    fn test_classify_tab_in_middle_returns_err_p004() {
        let err = classify_line("type:\tworkflow", 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::P004);
    }

    #[test]
    fn test_classify_tab_only_returns_err_p004() {
        let err = classify_line("\t", 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::P004);
    }

    // ---- L: lex() Integration ----

    #[test]
    fn test_lex_valid_snippet_returns_ok_with_correct_lines() {
        let input = "node auth.login\ntype: workflow\nsummary: Login flow\n";
        let lines = lex(input).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[0].kind,
            LineKind::NodeDeclaration("auth.login".to_string())
        );
        assert_eq!(
            lines[1].kind,
            LineKind::ScalarField("type".to_string(), "workflow".to_string())
        );
        assert_eq!(
            lines[2].kind,
            LineKind::ScalarField("summary".to_string(), "Login flow".to_string())
        );
    }

    #[test]
    fn test_lex_two_tab_lines_returns_err_with_two_p004_errors() {
        let input = "\ttype: workflow\nsummary: ok\n\tversion: 1\n";
        let errors = lex(input).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|e| e.code == ErrorCode::P004));
    }

    #[test]
    fn test_lex_empty_input_returns_ok_empty_vec() {
        let lines = lex("").unwrap();
        assert_eq!(lines, vec![]);
    }
}
