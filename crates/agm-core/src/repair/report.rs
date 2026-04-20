//! Repair report types: `RepairReport` and `RepairRecord`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// RepairRecord
// ---------------------------------------------------------------------------

/// Records a single textual rewrite applied by the repair engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairRecord {
    /// Stable rule identifier (e.g. `"R-SMART-QUOTES"`).
    pub rule_id: String,
    /// 1-indexed line number (character position) in the **original** text.
    pub line: usize,
    /// 1-indexed column (character position) in the **original** text.
    pub col: usize,
    /// Original text fragment.
    pub before: String,
    /// Replacement text fragment.
    pub after: String,
}

// ---------------------------------------------------------------------------
// RepairReport
// ---------------------------------------------------------------------------

/// Summary of every rewrite applied during one repair pass.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepairReport {
    /// Ordered list of rewrites applied.
    pub rewrites: Vec<RepairRecord>,
    /// Whether the safety net rolled back all changes.
    pub rolled_back: bool,
}

impl RepairReport {
    /// Returns `true` when no rewrites were recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rewrites.is_empty()
    }

    /// One-line human-readable summary.
    ///
    /// Example: `"7 rewrites (3 rules): R-CRLF ×1, R-SMART-QUOTES ×3, R-TRAILING-WS ×3"`
    #[must_use]
    pub fn summary(&self) -> String {
        if self.rewrites.is_empty() {
            return "0 rewrites".to_owned();
        }

        // Count occurrences per rule in insertion order of first occurrence.
        let mut order: Vec<String> = Vec::new();
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for rec in &self.rewrites {
            if !counts.contains_key(&rec.rule_id) {
                order.push(rec.rule_id.clone());
            }
            *counts.entry(rec.rule_id.clone()).or_insert(0) += 1;
        }

        let total = self.rewrites.len();
        let rule_count = order.len();
        let parts: Vec<String> = order
            .iter()
            .map(|id| {
                let n = counts[id];
                format!("{id} \u{00d7}{n}")
            })
            .collect();
        let rewrite_word = if total == 1 { "rewrite" } else { "rewrites" };
        let rule_word = if rule_count == 1 { "rule" } else { "rules" };
        format!(
            "{total} {rewrite_word} ({rule_count} {rule_word}): {}",
            parts.join(", ")
        )
    }

    /// Return all line numbers for records matching `rule_id`, sorted.
    #[must_use]
    pub fn lines_for_rule(&self, rule_id: &str) -> Vec<usize> {
        let mut lines: Vec<usize> = self
            .rewrites
            .iter()
            .filter(|r| r.rule_id == rule_id)
            .map(|r| r.line)
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(rule_id: &str, line: usize) -> RepairRecord {
        RepairRecord {
            rule_id: rule_id.to_owned(),
            line,
            col: 1,
            before: "x".to_owned(),
            after: "y".to_owned(),
        }
    }

    #[test]
    fn test_is_empty_true_when_empty() {
        let r = RepairReport::default();
        assert!(r.is_empty());
    }

    #[test]
    fn test_is_empty_false_when_has_record() {
        let mut r = RepairReport::default();
        r.rewrites.push(rec("R-CRLF", 1));
        assert!(!r.is_empty());
    }

    #[test]
    fn test_summary_empty() {
        let r = RepairReport::default();
        assert_eq!(r.summary(), "0 rewrites");
    }

    #[test]
    fn test_summary_single_rule() {
        let mut r = RepairReport::default();
        r.rewrites.push(rec("R-CRLF", 1));
        assert_eq!(r.summary(), "1 rewrite (1 rule): R-CRLF \u{00d7}1");
    }

    #[test]
    fn test_summary_multiple_rules() {
        let mut r = RepairReport::default();
        r.rewrites.push(rec("R-CRLF", 1));
        r.rewrites.push(rec("R-SMART-QUOTES", 4));
        r.rewrites.push(rec("R-SMART-QUOTES", 7));
        let s = r.summary();
        assert!(s.contains("3 rewrites"));
        assert!(s.contains("2 rules"));
        assert!(s.contains("R-CRLF \u{00d7}1"));
        assert!(s.contains("R-SMART-QUOTES \u{00d7}2"));
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut r = RepairReport::default();
        r.rewrites.push(rec("R-TRAILING-WS", 10));
        let json = serde_json::to_string(&r).expect("serialize");
        let back: RepairReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.rewrites.len(), 1);
        assert_eq!(back.rewrites[0].rule_id, "R-TRAILING-WS");
        assert_eq!(back.rewrites[0].line, 10);
    }
}
