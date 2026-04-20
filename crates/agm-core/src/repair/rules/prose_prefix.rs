//! `R-PROSE-BEFORE-HEADER` and `R-MISSING-AGM-FENCE` repair rules.
//!
//! `R-PROSE-BEFORE-HEADER`: Strip leading prose lines before the AGM header.
//! Heuristic (D17): look for a valid AGM marker within the first 20 non-blank
//! lines (`agm:`, `package:`, `version:`, `# `, or `node `). If found, strip
//! everything before it; otherwise leave input unchanged (conservative).
//!
//! `R-MISSING-AGM-FENCE` (D18): If the entire input is wrapped in a code fence
//! (first non-blank line matches `^```(agm)?$` and last non-blank line is
//! `` ``` ``), strip both fence lines and record one RepairRecord at line 1.

use crate::repair::report::RepairRecord;
use crate::repair::rules::RepairRule;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Returns `true` if `line` looks like the start of an AGM header or node.
fn is_agm_marker(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("agm:")
        || trimmed.starts_with("package:")
        || trimmed.starts_with("version:")
        || trimmed.starts_with("# ")
        || trimmed == "#"
        || trimmed.starts_with("node ")
}

// ---------------------------------------------------------------------------
// R-PROSE-BEFORE-HEADER
// ---------------------------------------------------------------------------

pub(crate) struct ProsePrefixRule;

impl RepairRule for ProsePrefixRule {
    fn id(&self) -> &'static str {
        "R-PROSE-BEFORE-HEADER"
    }

    fn description(&self) -> &'static str {
        "Strip leading prose lines before the AGM header (handles LLM preamble)."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        let lines: Vec<&str> = input.lines().collect();

        // Find the index of the first AGM marker within the first 20 non-blank lines.
        let mut non_blank_seen = 0;
        let mut header_line_idx: Option<usize> = None;

        for (idx, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            non_blank_seen += 1;
            if is_agm_marker(line) {
                header_line_idx = Some(idx);
                break;
            }
            if non_blank_seen >= 20 {
                break;
            }
        }

        let Some(start_idx) = header_line_idx else {
            // No marker found — conservative: do not strip anything.
            return (input.to_owned(), Vec::new());
        };

        if start_idx == 0 {
            // Nothing to strip — header is already at the start.
            return (input.to_owned(), Vec::new());
        }

        // Strip lines[0..start_idx] (the prose prefix).
        let stripped_lines = &lines[start_idx..];
        let mut out = stripped_lines.join("\n");
        if input.ends_with('\n') {
            out.push('\n');
        }

        let records = vec![RepairRecord {
            rule_id: self.id().to_owned(),
            line: 1,
            col: 1,
            before: lines[..start_idx].join("\n"),
            after: String::new(),
        }];

        (out, records)
    }
}

// ---------------------------------------------------------------------------
// R-MISSING-AGM-FENCE
// ---------------------------------------------------------------------------

pub(crate) struct MissingAgmFenceRule;

impl RepairRule for MissingAgmFenceRule {
    fn id(&self) -> &'static str {
        "R-MISSING-AGM-FENCE"
    }

    fn description(&self) -> &'static str {
        "Strip wrapping ```agm code fence when entire input is enclosed in one."
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        let lines: Vec<&str> = input.lines().collect();

        // Find first non-blank line index.
        let first_non_blank = lines.iter().position(|l| !l.trim().is_empty());
        // Find last non-blank line index.
        let last_non_blank = lines.iter().rposition(|l| !l.trim().is_empty());

        let (Some(first_idx), Some(last_idx)) = (first_non_blank, last_non_blank) else {
            return (input.to_owned(), Vec::new());
        };

        if first_idx == last_idx {
            // Only one non-blank line: can't be an open+close fence pair.
            return (input.to_owned(), Vec::new());
        }

        let first_line = lines[first_idx].trim();
        let last_line = lines[last_idx].trim();

        // First non-blank line must match ^```(agm)?$
        let first_is_fence = first_line == "```" || first_line == "```agm";
        // Last non-blank line must be exactly ```
        let last_is_fence = last_line == "```";

        if !first_is_fence || !last_is_fence {
            return (input.to_owned(), Vec::new());
        }

        // Strip first and last fence lines.
        let mut result_lines: Vec<&str> = Vec::with_capacity(lines.len().saturating_sub(2));
        for (idx, &line) in lines.iter().enumerate() {
            if idx == first_idx || idx == last_idx {
                continue;
            }
            result_lines.push(line);
        }

        let mut out = result_lines.join("\n");
        if input.ends_with('\n') {
            out.push('\n');
        }

        let records = vec![RepairRecord {
            rule_id: self.id().to_owned(),
            line: 1,
            col: 1,
            before: lines[first_idx].to_owned(),
            after: String::new(),
        }];

        (out, records)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- R-PROSE-BEFORE-HEADER ----

    #[test]
    fn test_leading_prose_stripped() {
        let input = "Here's your AGM file:\n\nagm: 1.0\npackage: test\nversion: 0.1\n";
        let (out, records) = ProsePrefixRule.apply(input);
        assert!(out.starts_with("agm: 1.0"), "should start with header");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "R-PROSE-BEFORE-HEADER");
    }

    #[test]
    fn test_no_prose_untouched() {
        let input = "agm: 1.0\npackage: test\nversion: 0.1\n";
        let (out, records) = ProsePrefixRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_no_agm_marker_untouched() {
        // No AGM marker within 20 non-blank lines → do not strip
        let input = "random prose\nmore prose\nand more\n";
        let (out, records) = ProsePrefixRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_node_marker_triggers_strip() {
        let input = "Preamble text\nnode mynode\ntype: facts\n";
        let (out, records) = ProsePrefixRule.apply(input);
        assert!(out.starts_with("node mynode"));
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_hash_comment_triggers_strip() {
        let input = "intro text\n# A comment\nagm: 1.0\n";
        let (out, records) = ProsePrefixRule.apply(input);
        assert!(out.starts_with("# A comment"));
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_prose_exceeds_20_lines_conservative() {
        // 21 prose lines before any marker → do not strip
        let mut input = String::new();
        for i in 0..21 {
            input.push_str(&format!("prose line {i}\n"));
        }
        input.push_str("agm: 1.0\n");
        let (out, records) = ProsePrefixRule.apply(&input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_idempotency_prose_prefix() {
        let input = "intro:\n\nagm: 1.0\npackage: x\n";
        let (first, _) = ProsePrefixRule.apply(input);
        let (second, records2) = ProsePrefixRule.apply(&first);
        assert_eq!(first, second);
        assert!(records2.is_empty());
    }

    // ---- R-MISSING-AGM-FENCE ----

    #[test]
    fn test_wrapped_fence_stripped() {
        let input = "```agm\nagm: 1.0\npackage: test\nversion: 0.1\n```\n";
        let (out, records) = MissingAgmFenceRule.apply(input);
        assert!(!out.contains("```"), "fences should be stripped");
        assert!(out.contains("agm: 1.0"));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].rule_id, "R-MISSING-AGM-FENCE");
        assert_eq!(records[0].line, 1);
    }

    #[test]
    fn test_bare_fence_stripped() {
        let input = "```\nagm: 1.0\npackage: test\n```\n";
        let (out, records) = MissingAgmFenceRule.apply(input);
        assert!(!out.contains("```"));
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_no_fence_untouched() {
        let input = "agm: 1.0\npackage: test\nversion: 0.1\n";
        let (out, records) = MissingAgmFenceRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_partial_fence_not_stripped() {
        // Only opening fence, no closing → do not strip
        let input = "```agm\nagm: 1.0\npackage: test\n";
        let (out, records) = MissingAgmFenceRule.apply(input);
        assert_eq!(out, input);
        assert!(records.is_empty());
    }

    #[test]
    fn test_idempotency_fence() {
        let input = "```agm\nagm: 1.0\npackage: test\n```\n";
        let (first, _) = MissingAgmFenceRule.apply(input);
        let (second, records2) = MissingAgmFenceRule.apply(&first);
        assert_eq!(first, second);
        assert!(records2.is_empty());
    }
}
