//! Shared field-parsing helpers used by both header and node parsers.

use std::collections::HashSet;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::imports::ImportEntry;

use super::lexer::{Line, LineKind};

// ---------------------------------------------------------------------------
// FieldTracker
// ---------------------------------------------------------------------------

/// Tracks seen field names for duplicate detection (P006).
pub(crate) struct FieldTracker {
    seen: HashSet<String>,
}

impl FieldTracker {
    pub(crate) fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Returns `true` if the field was already seen (duplicate).
    pub(crate) fn track(&mut self, field_name: &str) -> bool {
        !self.seen.insert(field_name.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Structured field detection
// ---------------------------------------------------------------------------

/// Field names that are parsed by Step 6 (structured fields).
pub(crate) const STRUCTURED_FIELD_NAMES: &[&str] = &[
    "code",
    "code_blocks",
    "verify",
    "agent_context",
    "parallel_groups",
    "memory",
    "load_profiles",
];

pub(crate) fn is_structured_field(name: &str) -> bool {
    STRUCTURED_FIELD_NAMES.contains(&name)
}

// ---------------------------------------------------------------------------
// parse_indented_list
// ---------------------------------------------------------------------------

/// Consumes consecutive `ListItem` lines from `lines` starting at `*pos`.
///
/// Blank lines between items are skipped if followed by more `ListItem`s.
/// Returns the collected item strings.
pub(crate) fn parse_indented_list(lines: &[Line], pos: &mut usize) -> Vec<String> {
    let mut items = Vec::new();
    while *pos < lines.len() {
        match &lines[*pos].kind {
            LineKind::ListItem(value) => {
                items.push(value.clone());
                *pos += 1;
            }
            // Comments inside indented lists are skipped (spec S16.7).
            LineKind::Comment | LineKind::TestExpectHeader(_) => {
                *pos += 1;
            }
            LineKind::Blank => {
                // Peek ahead: only skip blank if a ListItem or Comment follows.
                let mut lookahead = *pos + 1;
                while lookahead < lines.len() {
                    match &lines[lookahead].kind {
                        LineKind::Blank => lookahead += 1,
                        LineKind::Comment | LineKind::TestExpectHeader(_) => {
                            lookahead += 1;
                        }
                        LineKind::ListItem(_) => break,
                        _ => {
                            // No more list items — stop consuming.
                            return items;
                        }
                    }
                }
                if lookahead < lines.len() {
                    if let LineKind::ListItem(_) = &lines[lookahead].kind {
                        *pos += 1; // skip the blank
                        continue;
                    }
                }
                break;
            }
            _ => break,
        }
    }
    items
}

// ---------------------------------------------------------------------------
// parse_block
// ---------------------------------------------------------------------------

/// Consumes consecutive `IndentedLine` and `ListItem` lines (and blank lines
/// within) that form a block body.
///
/// Strips `base_indent` leading spaces from each raw line.
/// Trailing empty lines are removed before returning.
pub(crate) fn parse_block(lines: &[Line], pos: &mut usize) -> String {
    // Determine base indent from the first non-blank indented line.
    let base_indent = {
        let mut base = 0usize;
        let mut i = *pos;
        while i < lines.len() {
            match &lines[i].kind {
                LineKind::IndentedLine(_) | LineKind::ListItem(_) => {
                    base = lines[i].indent;
                    break;
                }
                LineKind::Blank => {
                    i += 1;
                }
                _ => break,
            }
        }
        base
    };

    let mut parts: Vec<String> = Vec::new();

    while *pos < lines.len() {
        match &lines[*pos].kind {
            LineKind::IndentedLine(_) | LineKind::ListItem(_) => {
                // Strip base_indent leading spaces from the raw line.
                let raw = &lines[*pos].raw;
                let stripped = if raw.len() >= base_indent {
                    raw[base_indent..].to_owned()
                } else {
                    raw.trim_start().to_owned()
                };
                parts.push(stripped);
                *pos += 1;
            }
            LineKind::Blank => {
                // Peek ahead to see if more block content follows.
                let mut lookahead = *pos + 1;
                while lookahead < lines.len() {
                    match &lines[lookahead].kind {
                        LineKind::Blank => lookahead += 1,
                        LineKind::IndentedLine(_) | LineKind::ListItem(_) => break,
                        _ => {
                            return finish_block(parts);
                        }
                    }
                }
                if lookahead < lines.len() {
                    match &lines[lookahead].kind {
                        LineKind::IndentedLine(_) | LineKind::ListItem(_) => {
                            parts.push(String::new()); // represent blank line
                            *pos += 1;
                            continue;
                        }
                        _ => {}
                    }
                }
                break;
            }
            _ => break,
        }
    }

    finish_block(parts)
}

/// Joins block lines with `\n`, trims trailing empty lines.
fn finish_block(mut parts: Vec<String>) -> String {
    // Trim trailing empty strings.
    while parts.last().is_some_and(|s| s.is_empty()) {
        parts.pop();
    }
    parts.join("\n")
}

// ---------------------------------------------------------------------------
// parse_imports
// ---------------------------------------------------------------------------

/// Parses a list of raw import strings into `ImportEntry` values.
///
/// Invalid entries emit a P001 error but do not stop parsing.
pub(crate) fn parse_imports(
    items: &[String],
    line_number: usize,
    errors: &mut Vec<AgmError>,
) -> Vec<crate::model::imports::ImportEntry> {
    let mut result = Vec::new();
    for item in items {
        match item.parse::<ImportEntry>() {
            Ok(entry) => result.push(entry),
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P001,
                    format!("Invalid import entry: {item:?}"),
                    ErrorLocation::new(None, Some(line_number), None),
                ));
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// collect_structured_raw
// ---------------------------------------------------------------------------

/// Consumes all lines with indent > 0 (including blank lines within the block).
///
/// Used as a stub for structured fields that will be parsed in Step 6.
/// Returns the raw text joined with `\n`.
pub(crate) fn collect_structured_raw(lines: &[Line], pos: &mut usize) -> String {
    let mut parts: Vec<String> = Vec::new();

    while *pos < lines.len() {
        match &lines[*pos].kind {
            LineKind::ScalarField(_, _)
            | LineKind::InlineListField(_, _)
            | LineKind::FieldStart(_)
            | LineKind::ListItem(_)
            | LineKind::IndentedLine(_)
            | LineKind::BodyMarker => {
                if lines[*pos].indent > 0
                    || matches!(
                        &lines[*pos].kind,
                        LineKind::ListItem(_) | LineKind::IndentedLine(_)
                    )
                {
                    parts.push(lines[*pos].raw.clone());
                    *pos += 1;
                } else {
                    break;
                }
            }
            LineKind::Blank => {
                // Peek ahead to see if indented content follows.
                let mut lookahead = *pos + 1;
                while lookahead < lines.len() {
                    if matches!(&lines[lookahead].kind, LineKind::Blank) {
                        lookahead += 1;
                    } else {
                        break;
                    }
                }
                let has_more = lookahead < lines.len()
                    && matches!(
                        &lines[lookahead].kind,
                        LineKind::ScalarField(_, _)
                            | LineKind::InlineListField(_, _)
                            | LineKind::FieldStart(_)
                            | LineKind::ListItem(_)
                            | LineKind::IndentedLine(_)
                    )
                    && lines[lookahead].indent > 0;

                if has_more {
                    parts.push(lines[*pos].raw.clone());
                    *pos += 1;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    parts.join("\n")
}

// ---------------------------------------------------------------------------
// skip_field_body
// ---------------------------------------------------------------------------

/// Like `collect_structured_raw` but discards the content.
///
/// Used for duplicate field bodies.
pub(crate) fn skip_field_body(lines: &[Line], pos: &mut usize) {
    collect_structured_raw(lines, pos);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::lexer::lex;

    #[test]
    fn test_field_tracker_new_not_duplicate() {
        let mut tracker = FieldTracker::new();
        assert!(!tracker.track("summary"));
    }

    #[test]
    fn test_field_tracker_second_call_is_duplicate() {
        let mut tracker = FieldTracker::new();
        tracker.track("summary");
        assert!(tracker.track("summary"));
    }

    #[test]
    fn test_field_tracker_different_fields_not_duplicate() {
        let mut tracker = FieldTracker::new();
        tracker.track("summary");
        assert!(!tracker.track("detail"));
    }

    #[test]
    fn test_is_structured_field_known_returns_true() {
        assert!(is_structured_field("code"));
        assert!(is_structured_field("verify"));
        assert!(is_structured_field("memory"));
    }

    #[test]
    fn test_is_structured_field_unknown_returns_false() {
        assert!(!is_structured_field("summary"));
        assert!(!is_structured_field("detail"));
    }

    #[test]
    fn test_parse_indented_list_basic() {
        let input = "  - item1\n  - item2\n  - item3\n";
        let lines = lex(input).unwrap();
        let mut pos = 0;
        let items = parse_indented_list(&lines, &mut pos);
        assert_eq!(items, vec!["item1", "item2", "item3"]);
        assert_eq!(pos, 3);
    }

    #[test]
    fn test_parse_indented_list_stops_at_non_list() {
        let input = "  - item1\nsummary: foo\n";
        let lines = lex(input).unwrap();
        let mut pos = 0;
        let items = parse_indented_list(&lines, &mut pos);
        assert_eq!(items, vec!["item1"]);
        assert_eq!(pos, 1);
    }

    #[test]
    fn test_parse_block_basic() {
        let input = "  This is block text.\n  Second line.\n";
        let lines = lex(input).unwrap();
        let mut pos = 0;
        let text = parse_block(&lines, &mut pos);
        assert_eq!(text, "This is block text.\nSecond line.");
    }

    #[test]
    fn test_parse_block_strips_base_indent() {
        let input = "    indented four\n    second line\n";
        let lines = lex(input).unwrap();
        let mut pos = 0;
        let text = parse_block(&lines, &mut pos);
        assert_eq!(text, "indented four\nsecond line");
    }
}
