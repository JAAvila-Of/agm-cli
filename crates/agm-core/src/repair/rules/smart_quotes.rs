//! `R-SMART-QUOTES` rule: replace typographic quote characters with ASCII equivalents.
//!
//! Replaced characters:
//!   - U+201C `"` (left double quotation mark) → `"`
//!   - U+201D `"` (right double quotation mark) → `"`
//!   - U+2018 `'` (left single quotation mark) → `'`
//!   - U+2019 `'` (right single quotation mark) → `'`
//!   - U+00AB `«` (left-pointing double angle quotation mark) → `"`
//!   - U+00BB `»` (right-pointing double angle quotation mark) → `"`
//!
//! Quotes **inside triple-backtick fences** are preserved (D19).
//! Fence tracking uses a simple toggle on lines matching `^\s*```.*$`.

use crate::repair::report::RepairRecord;
use crate::repair::rules::RepairRule;

/// Map a typographic quote character to its ASCII replacement, if applicable.
fn ascii_replacement(ch: char) -> Option<char> {
    match ch {
        '\u{201C}' | '\u{201D}' | '\u{00AB}' | '\u{00BB}' => Some('"'),
        '\u{2018}' | '\u{2019}' => Some('\''),
        _ => None,
    }
}

pub(crate) struct SmartQuotesRule;

impl RepairRule for SmartQuotesRule {
    fn id(&self) -> &'static str {
        "R-SMART-QUOTES"
    }

    fn description(&self) -> &'static str {
        "Replace typographic (smart) quote characters with ASCII equivalents outside code fences."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        let mut records: Vec<RepairRecord> = Vec::new();
        let mut out = String::with_capacity(input.len());
        let mut in_fence = false;

        for (line_idx, line_str) in input.lines().enumerate() {
            let line_no = line_idx + 1;

            // Toggle fence state if the line matches ^\s*```.*$
            let trimmed = line_str.trim_start();
            if trimmed.starts_with("```") {
                in_fence = !in_fence;
                out.push_str(line_str);
                out.push('\n');
                continue;
            }

            if in_fence {
                // Inside a fence: preserve all characters
                out.push_str(line_str);
                out.push('\n');
                continue;
            }

            // Outside fence: replace smart quotes character by character
            let mut col = 1usize;
            for ch in line_str.chars() {
                if let Some(replacement) = ascii_replacement(ch) {
                    records.push(RepairRecord {
                        rule_id: self.id().to_owned(),
                        line: line_no,
                        col,
                        before: ch.to_string(),
                        after: replacement.to_string(),
                    });
                    out.push(replacement);
                } else {
                    out.push(ch);
                }
                col += 1;
            }
            out.push('\n');
        }

        // Restore absence of trailing newline if original lacked one.
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
    fn test_basic_rewrite_double_quotes() {
        let input = "summary: \u{201C}hello world\u{201D}";
        let (out, records) = SmartQuotesRule.apply(input);
        assert_eq!(out, "summary: \"hello world\"");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].rule_id, "R-SMART-QUOTES");
        assert_eq!(records[0].before, "\u{201C}");
        assert_eq!(records[0].after, "\"");
    }

    #[test]
    fn test_basic_rewrite_single_quotes() {
        let input = "summary: \u{2018}it\u{2019}s fine";
        let (out, records) = SmartQuotesRule.apply(input);
        assert_eq!(out, "summary: 'it's fine");
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_angle_quotes_replaced() {
        let input = "note: \u{00AB}see also\u{00BB}";
        let (out, records) = SmartQuotesRule.apply(input);
        assert_eq!(out, "note: \"see also\"");
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_inside_fence_preserved() {
        let input = "code:\n```\n\u{201C}inside fence\u{201D}\n```\n";
        let (out, records) = SmartQuotesRule.apply(input);
        assert!(out.contains('\u{201C}'), "smart quote inside fence must be preserved");
        assert!(records.is_empty(), "no records should be produced for in-fence content");
    }

    #[test]
    fn test_outside_fence_rewritten_inside_preserved() {
        let input =
            "summary: \u{201C}outside\u{201D}\n```\n\u{201C}inside\u{201D}\n```\nafter: \u{201C}again\u{201D}\n";
        let (out, records) = SmartQuotesRule.apply(input);
        // outside and after should be replaced; inside should be preserved
        assert!(out.contains("\"outside\""));
        assert!(out.contains('\u{201C}'), "inside-fence quote preserved");
        assert!(out.contains("\"again\""));
        assert_eq!(records.len(), 4, "2 outside + 2 after");
    }

    #[test]
    fn test_clean_input_unchanged() {
        let input = "summary: \"hello world\"\n";
        let (out, records) = SmartQuotesRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_multiline_rewrites_correct_line_numbers() {
        let input = "a: \u{201C}one\u{201D}\nb: normal\nc: \u{201C}three\u{201D}\n";
        let (_, records) = SmartQuotesRule.apply(input);
        assert_eq!(records.len(), 4);
        assert_eq!(records[0].line, 1);
        assert_eq!(records[1].line, 1);
        assert_eq!(records[2].line, 3);
        assert_eq!(records[3].line, 3);
    }

    #[test]
    fn test_idempotency() {
        let input = "summary: \u{201C}hello\u{201D}";
        let (first, _) = SmartQuotesRule.apply(input);
        let (second, records2) = SmartQuotesRule.apply(&first);
        assert_eq!(first, second);
        assert!(records2.is_empty(), "second pass must be a no-op");
    }

    #[test]
    fn test_right_single_quote_in_contraction() {
        let input = "summary: it\u{2019}s a test";
        let (out, records) = SmartQuotesRule.apply(input);
        assert_eq!(out, "summary: it's a test");
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_fence_toggle_resets_correctly() {
        // Two fences: content inside first preserved, outside second replaced
        let input = "```\n\u{201C}in1\u{201D}\n```\n\u{201C}out\u{201D}\n```\n\u{201C}in2\u{201D}\n```\n";
        let (out, records) = SmartQuotesRule.apply(input);
        assert!(out.contains('\u{201C}'), "in-fence quotes preserved");
        assert!(out.contains("\"out\""), "out-of-fence replaced");
        // Only "out" is replaced (2 chars); the in-fence quotes should stay
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_col_tracking() {
        let input = "ab\u{201C}cd";
        let (_, records) = SmartQuotesRule.apply(input);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].col, 3); // 'a'=1, 'b'=2, quote=3
    }
}
