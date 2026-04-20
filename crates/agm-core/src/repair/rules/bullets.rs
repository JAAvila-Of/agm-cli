//! `R-BULLET-STAR` rule: replace `*` and `+` list bullets with `-`.
//!
//! Conservative heuristic (D20): only rewrites `*`/`+` bullets that are
//! **list continuations**, defined as:
//!   - The preceding non-blank line ended with `:` (a field assignment), OR
//!   - The preceding non-blank line was itself a `-` bullet at the same or lesser indent.
//!
//! This avoids touching `*` used in other contexts (e.g. markdown emphasis in
//! block-text values, glob patterns, etc.).

use crate::repair::report::RepairRecord;
use crate::repair::rules::RepairRule;

/// Check whether a trimmed line is a `-` bullet.
fn is_dash_bullet(line: &str) -> bool {
    line.starts_with("- ")
        || line == "-"
        || line.starts_with("-\t")
}

/// Return the leading-whitespace character count of a string.
fn indent_len(s: &str) -> usize {
    s.chars().take_while(|c| c.is_ascii_whitespace()).count()
}

pub(crate) struct BulletStarRule;

impl RepairRule for BulletStarRule {
    fn id(&self) -> &'static str {
        "R-BULLET-STAR"
    }

    fn description(&self) -> &'static str {
        "Replace `*` and `+` list bullets with `-` where contextually safe."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        let lines: Vec<&str> = input.lines().collect();
        let mut records: Vec<RepairRecord> = Vec::new();
        let mut result: Vec<String> = Vec::with_capacity(lines.len());

        // Track the last non-blank line content and its index.
        let mut last_non_blank: Option<usize> = None;

        for (idx, &line) in lines.iter().enumerate() {
            let line_no = idx + 1;
            let trimmed = line.trim_start();

            // Check if this line is a `*` or `+` bullet
            let is_star = trimmed.starts_with("* ") || trimmed == "*" || trimmed.starts_with("*\t");
            let is_plus = trimmed.starts_with("+ ") || trimmed == "+" || trimmed.starts_with("+\t");

            if (is_star || is_plus) && should_rewrite_result(last_non_blank, &result) {
                let bullet = if is_star { '*' } else { '+' };
                let new_line = replace_first_char(line, bullet, '-');
                let col = indent_len(line) + 1;
                records.push(RepairRecord {
                    rule_id: self.id().to_owned(),
                    line: line_no,
                    col,
                    before: bullet.to_string(),
                    after: "-".to_owned(),
                });
                result.push(new_line);
            } else {
                result.push(line.to_owned());
            }

            if !line.trim().is_empty() {
                last_non_blank = Some(result.len() - 1);
            }
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

/// Returns `true` if the current bullet should be rewritten, based on the
/// **result buffer** (already-rewritten lines). Using the result buffer ensures
/// that `* second` is also replaced when `* first` was already rewritten to `- first`.
fn should_rewrite_result(last_non_blank_result_idx: Option<usize>, result: &[String]) -> bool {
    let Some(prev_idx) = last_non_blank_result_idx else {
        return false;
    };
    let prev_line = &result[prev_idx];
    let prev_trimmed = prev_line.trim();

    // Condition 1: previous non-blank line ends with ':'
    if prev_trimmed.ends_with(':') {
        return true;
    }

    // Condition 2: previous non-blank line is a `-` bullet
    if is_dash_bullet(prev_trimmed) {
        return true;
    }

    false
}

/// Replace the first occurrence of `from` in `line` with `to`.
fn replace_first_char(line: &str, from: char, to: char) -> String {
    let mut replaced = false;
    line.chars()
        .map(|c| {
            if !replaced && c == from {
                replaced = true;
                to
            } else {
                c
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_star_to_dash_after_key_colon() {
        let input = "items:\n* first\n* second\n";
        let (out, records) = BulletStarRule.apply(input);
        assert_eq!(out, "items:\n- first\n- second\n");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].rule_id, "R-BULLET-STAR");
        assert_eq!(records[0].line, 2);
        assert_eq!(records[1].line, 3);
    }

    #[test]
    fn test_plus_to_dash() {
        let input = "tags:\n+ alpha\n+ beta\n";
        let (out, records) = BulletStarRule.apply(input);
        assert_eq!(out, "tags:\n- alpha\n- beta\n");
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_star_after_dash_bullet() {
        // * following a - bullet at same indent should be rewritten
        let input = "list:\n- first\n* second\n";
        let (out, records) = BulletStarRule.apply(input);
        assert_eq!(out, "list:\n- first\n- second\n");
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_unrelated_star_untouched() {
        // * not after a colon or dash bullet: leave it alone
        let input = "summary: a product of effort\n* not a list item\n";
        let (out, records) = BulletStarRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_indented_bullets() {
        let input = "steps:\n  * do this\n  * do that\n";
        let (out, records) = BulletStarRule.apply(input);
        assert_eq!(out, "steps:\n  - do this\n  - do that\n");
        assert_eq!(records.len(), 2);
        // col should be 3 (2 spaces + bullet position)
        assert_eq!(records[0].col, 3);
    }

    #[test]
    fn test_mixed_bullets() {
        let input = "items:\n* alpha\n+ beta\n- gamma\n";
        let (out, records) = BulletStarRule.apply(input);
        assert_eq!(out, "items:\n- alpha\n- beta\n- gamma\n");
        assert_eq!(records.len(), 2); // only * and + replaced; - already correct
    }

    #[test]
    fn test_idempotency() {
        let input = "items:\n* first\n* second\n";
        let (first, _) = BulletStarRule.apply(input);
        let (second, records2) = BulletStarRule.apply(&first);
        assert_eq!(first, second);
        assert!(records2.is_empty());
    }

    #[test]
    fn test_no_rewrite_at_start_of_file() {
        // No preceding non-blank line → conservative: do not rewrite
        let input = "* standalone bullet\n";
        let (out, records) = BulletStarRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }
}
