//! Whitespace repair rules:
//!   - `R-ZERO-WIDTH`: strip BOM and zero-width characters.
//!   - `R-CRLF`: normalize CRLF and bare CR to LF.
//!   - `R-TABS-TO-SPACES`: convert leading tabs to 2-space indent.
//!   - `R-TRAILING-WS`: strip trailing whitespace from every line.

use crate::repair::report::RepairRecord;
use crate::repair::rules::RepairRule;

// ---------------------------------------------------------------------------
// R-ZERO-WIDTH
// ---------------------------------------------------------------------------

/// Strip BOM (U+FEFF) and zero-width characters (U+200B, U+200C, U+200D).
/// Also strips inline U+FEFF (zero-width no-break space) anywhere in text.
pub(crate) struct ZeroWidthRule;

impl RepairRule for ZeroWidthRule {
    fn id(&self) -> &'static str {
        "R-ZERO-WIDTH"
    }

    fn description(&self) -> &'static str {
        "Strip BOM and zero-width Unicode characters (U+FEFF, U+200B, U+200C, U+200D)."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        const STRIP: &[char] = &[
            '\u{FEFF}', // BOM / zero-width no-break space
            '\u{200B}', // zero-width space
            '\u{200C}', // zero-width non-joiner
            '\u{200D}', // zero-width joiner
        ];

        let mut records: Vec<RepairRecord> = Vec::new();
        let mut line = 1usize;
        let mut col = 1usize;
        let mut out = String::with_capacity(input.len());

        for ch in input.chars() {
            if STRIP.contains(&ch) {
                records.push(RepairRecord {
                    rule_id: self.id().to_owned(),
                    line,
                    col,
                    before: ch.to_string(),
                    after: String::new(),
                });
                // do not advance col — character removed
            } else {
                out.push(ch);
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
        }

        (out, records)
    }
}

// ---------------------------------------------------------------------------
// R-CRLF
// ---------------------------------------------------------------------------

/// Normalize CRLF (\r\n) and bare CR (\r) to LF (\n).
/// Per D16: records ONE RepairRecord per file with line=1, col=1.
pub(crate) struct CrlfRule;

impl RepairRule for CrlfRule {
    fn id(&self) -> &'static str {
        "R-CRLF"
    }

    fn description(&self) -> &'static str {
        "Normalize CRLF and bare CR line endings to LF."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        if !input.contains('\r') {
            return (input.to_owned(), Vec::new());
        }

        let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
        let records = vec![RepairRecord {
            rule_id: self.id().to_owned(),
            line: 1,
            col: 1,
            before: "\\r\\n".to_owned(),
            after: "\\n".to_owned(),
        }];
        (normalized, records)
    }
}

// ---------------------------------------------------------------------------
// R-TABS-TO-SPACES
// ---------------------------------------------------------------------------

/// Convert leading tabs to 2-space indentation.
/// Only touches tabs in the leading whitespace of each line; mid-line tabs
/// are left unchanged (they may be meaningful inside scalars).
pub(crate) struct TabsToSpacesRule;

impl RepairRule for TabsToSpacesRule {
    fn id(&self) -> &'static str {
        "R-TABS-TO-SPACES"
    }

    fn description(&self) -> &'static str {
        "Convert leading tab characters to 2-space indentation (spec P004)."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        let mut records: Vec<RepairRecord> = Vec::new();
        let mut out = String::with_capacity(input.len());

        for (line_idx, line_str) in input.lines().enumerate() {
            let line_no = line_idx + 1;

            // Count leading tabs
            let tab_count = line_str.chars().take_while(|&c| c == '\t').count();
            if tab_count > 0 {
                let spaces = "  ".repeat(tab_count);
                let rest = &line_str[tab_count..]; // byte offset ok since tabs are 1 byte
                out.push_str(&spaces);
                out.push_str(rest);
                records.push(RepairRecord {
                    rule_id: self.id().to_owned(),
                    line: line_no,
                    col: 1,
                    before: "\t".repeat(tab_count),
                    after: spaces,
                });
            } else {
                out.push_str(line_str);
            }
            out.push('\n');
        }

        // `lines()` drops a trailing newline; add it back only if original had one.
        // Actually `lines()` is tricky: we use it but reconstruct.
        // We always add '\n' after each line above, so if original had no trailing
        // newline we strip the last one.
        if !input.is_empty() && !input.ends_with('\n') && out.ends_with('\n') {
            out.pop();
        }

        if records.is_empty() {
            (input.to_owned(), Vec::new())
        } else {
            (out, records)
        }
    }
}

// ---------------------------------------------------------------------------
// R-TRAILING-WS
// ---------------------------------------------------------------------------

/// Strip trailing whitespace (spaces, tabs) from every line.
/// Per D16: records one RepairRecord per affected line.
pub(crate) struct TrailingWsRule;

impl RepairRule for TrailingWsRule {
    fn id(&self) -> &'static str {
        "R-TRAILING-WS"
    }

    fn description(&self) -> &'static str {
        "Strip trailing whitespace from every line."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        let mut records: Vec<RepairRecord> = Vec::new();
        let mut out = String::with_capacity(input.len());

        for (line_idx, line_str) in input.lines().enumerate() {
            let line_no = line_idx + 1;
            let trimmed = line_str.trim_end();
            if trimmed.len() < line_str.len() {
                let trailing = &line_str[trimmed.len()..];
                records.push(RepairRecord {
                    rule_id: self.id().to_owned(),
                    line: line_no,
                    col: trimmed.chars().count() + 1,
                    before: trailing.to_owned(),
                    after: String::new(),
                });
                out.push_str(trimmed);
            } else {
                out.push_str(line_str);
            }
            out.push('\n');
        }

        if !input.is_empty() && !input.ends_with('\n') && out.ends_with('\n') {
            out.pop();
        }

        if records.is_empty() {
            (input.to_owned(), Vec::new())
        } else {
            (out, records)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_width_bom_stripped() {
        let input = "\u{FEFF}agm: 1.0";
        let (out, records) = ZeroWidthRule.apply(input);
        assert_eq!(out, "agm: 1.0");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "R-ZERO-WIDTH");
        assert_eq!(records[0].line, 1);
        assert_eq!(records[0].col, 1);
    }

    #[test]
    fn test_zero_width_inline_stripped() {
        let input = "summary: hello\u{200B}world";
        let (out, records) = ZeroWidthRule.apply(input);
        assert_eq!(out, "summary: helloworld");
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_zero_width_clean_text_unchanged() {
        let input = "agm: 1.0\npackage: foo\n";
        let (out, records) = ZeroWidthRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_crlf_normalized() {
        let input = "line1\r\nline2\r\nline3\r\n";
        let (out, records) = CrlfRule.apply(input);
        assert_eq!(out, "line1\nline2\nline3\n");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "R-CRLF");
        assert_eq!(records[0].line, 1);
        assert_eq!(records[0].col, 1);
    }

    #[test]
    fn test_crlf_bare_cr_normalized() {
        let input = "line1\rline2\r";
        let (out, records) = CrlfRule.apply(input);
        assert_eq!(out, "line1\nline2\n");
        assert!(!records.is_empty());
    }

    #[test]
    fn test_crlf_no_op_on_clean_input() {
        let input = "line1\nline2\n";
        let (out, records) = CrlfRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_tabs_in_indent_converted() {
        let input = "\tsummary: hello\n\t\tnested: value\n";
        let (out, records) = TabsToSpacesRule.apply(input);
        assert_eq!(out, "  summary: hello\n    nested: value\n");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].rule_id, "R-TABS-TO-SPACES");
    }

    #[test]
    fn test_tabs_mid_line_preserved() {
        // Mid-line tabs should NOT be touched
        let input = "summary: hello\tworld\n";
        let (out, records) = TabsToSpacesRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_tabs_no_op_on_spaces() {
        let input = "  indented: value\n";
        let (out, records) = TabsToSpacesRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_trailing_ws_stripped() {
        let input = "summary: hello   \npackage: foo\t\n";
        let (out, records) = TrailingWsRule.apply(input);
        assert_eq!(out, "summary: hello\npackage: foo\n");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].rule_id, "R-TRAILING-WS");
        assert_eq!(records[0].line, 1);
        assert_eq!(records[1].line, 2);
    }

    #[test]
    fn test_trailing_ws_no_op_on_clean() {
        let input = "summary: hello\npackage: foo\n";
        let (out, records) = TrailingWsRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_bom_stripped_at_start() {
        // BOM at file start
        let input = "\u{FEFF}node mynode\ntype: facts\n";
        let (out, records) = ZeroWidthRule.apply(input);
        assert!(!out.starts_with('\u{FEFF}'));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].line, 1);
        assert_eq!(records[0].col, 1);
    }
}
