//! `R-BARE-CODE-BLOCK` rule (disabled by default): wrap bare code blocks in
//! explicit triple-backtick fences when they follow a `code:` field.
//!
//! Heuristic: after a line containing only `code:` (or indented `code:`),
//! if the immediately following lines are indented (at least 2 spaces) and
//! the first of those lines matches a known language identifier pattern,
//! and there are at least 3 such indented lines, wrap them in a code fence.
//!
//! This rule is **disabled by default** and must be explicitly enabled via
//! `--enable-only R-BARE-CODE-BLOCK`. It is classified as risky because the
//! heuristic may produce false positives on block-text values that happen to
//! look like code.

use crate::repair::report::RepairRecord;
use crate::repair::rules::RepairRule;

/// Known language identifiers (subset) used as a confidence heuristic.
const LANG_TOKENS: &[&str] = &[
    "python",
    "rust",
    "javascript",
    "typescript",
    "java",
    "go",
    "bash",
    "sh",
    "sql",
    "yaml",
    "json",
    "toml",
    "c",
    "cpp",
    "c++",
    "ruby",
    "php",
    "kotlin",
    "swift",
];

/// Returns `true` if `line` (trimmed) looks like a language identifier.
fn looks_like_lang(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    LANG_TOKENS.iter().any(|&l| lower == l)
}

pub(crate) struct BareCodeBlockRule;

impl RepairRule for BareCodeBlockRule {
    fn id(&self) -> &'static str {
        "R-BARE-CODE-BLOCK"
    }

    fn description(&self) -> &'static str {
        "Wrap bare indented code blocks after `code:` in explicit triple-backtick fences. DISABLED BY DEFAULT."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        let lines: Vec<&str> = input.lines().collect();
        let mut records: Vec<RepairRecord> = Vec::new();
        let mut result: Vec<String> = Vec::with_capacity(lines.len() + 8);
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();
            let line_no = i + 1;

            // Detect a bare `code:` line (no value on same line, just the key)
            if trimmed == "code:" {
                let code_indent = indent_len(line);
                let block_indent = code_indent + 2; // expected indent for the code block

                // Collect following indented lines
                let mut j = i + 1;
                let mut block_lines: Vec<&str> = Vec::new();
                while j < lines.len() {
                    let next = lines[j];
                    let next_trimmed = next.trim();
                    // Stop on blank lines or lines with less indent
                    if next_trimmed.is_empty() {
                        break;
                    }
                    if indent_len(next) < block_indent {
                        break;
                    }
                    // Stop if we see another field at the same indent level as code:
                    if indent_len(next) == code_indent && next_trimmed.contains(':') {
                        break;
                    }
                    block_lines.push(next);
                    j += 1;
                }

                // Apply heuristic: need >=3 lines and first line looks like a language id
                if block_lines.len() >= 3 {
                    let first_content = block_lines[0].trim();
                    if looks_like_lang(first_content) {
                        // The first line is the language tag; the rest are actual code
                        let lang = first_content;
                        let code_body = &block_lines[1..];
                        let base_prefix = " ".repeat(block_indent);

                        result.push(line.to_owned()); // `code:` line
                        result.push(format!("{base_prefix}```{lang}"));
                        for &code_line in code_body {
                            result.push(code_line.to_owned());
                        }
                        result.push(format!("{base_prefix}```"));

                        records.push(RepairRecord {
                            rule_id: self.id().to_owned(),
                            line: line_no,
                            col: 1,
                            before: block_lines.join("\n"),
                            after: format!(
                                "{base_prefix}```{lang}\n{body}\n{base_prefix}```",
                                body = code_body.join("\n")
                            ),
                        });

                        i = j; // skip the consumed block lines
                        continue;
                    }
                }
            }

            result.push(line.to_owned());
            i += 1;
        }

        if records.is_empty() {
            return (input.to_owned(), Vec::new());
        }

        let mut out = result.join("\n");
        if input.ends_with('\n') {
            out.push('\n');
        }
        (out, records)
    }
}

fn indent_len(s: &str) -> usize {
    s.chars().take_while(|c| c.is_ascii_whitespace()).count()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_code_block_wrapped() {
        let input = "node x\ncode:\n  python\n  x = 1\n  y = 2\n  print(x)\n";
        let (out, records) = BareCodeBlockRule.apply(input);
        assert!(out.contains("```python"), "should insert opening fence");
        assert!(
            out.contains("```\n") || out.ends_with("```"),
            "should insert closing fence"
        );
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "R-BARE-CODE-BLOCK");
    }

    #[test]
    fn test_too_few_lines_not_wrapped() {
        // Only 2 lines after code: → do not wrap (need >= 3)
        let input = "node x\ncode:\n  python\n  x = 1\n";
        let (out, records) = BareCodeBlockRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_no_lang_identifier_not_wrapped() {
        // First line does not look like a language identifier
        let input = "node x\ncode:\n  this is prose\n  and more prose\n  still prose\n";
        let (out, records) = BareCodeBlockRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_already_fenced_not_double_wrapped() {
        // Already wrapped — rule does not trigger (no bare `code:` alone on a line)
        let input = "node x\ncode:\n  ```python\n  x = 1\n  ```\n";
        let (out, records) = BareCodeBlockRule.apply(input);
        // The block starts with ```python, not a lang token, so no wrapping
        assert_eq!(out, input);
        assert!(records.is_empty());
    }
}
