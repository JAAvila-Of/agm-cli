//! Normalization report types: `NormalizeReport`, `Rewrite`, `NormalizeWarning`.

use serde::{Deserialize, Serialize};

use crate::model::fields::Span;

// ---------------------------------------------------------------------------
// Rewrite
// ---------------------------------------------------------------------------

/// Records a single field-name or type-name rewrite applied by the normalizer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rewrite {
    /// Identifies the rule that triggered this rewrite, e.g. `"field.orchestration.parallel_groups"`.
    pub rule_id: String,
    /// The node ID this rewrite occurred on, if applicable.
    pub node_id: Option<String>,
    /// The original (non-canonical) name.
    pub before: String,
    /// The canonical name it was rewritten to.
    pub after: String,
    /// Source location where the original field appeared.
    pub span: Span,
}

// ---------------------------------------------------------------------------
// NormalizeWarningCode
// ---------------------------------------------------------------------------

/// Classification of a normalization advisory.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeWarningCode {
    /// Both canonical and synonym present on the same node — canonical kept.
    CollisionKeepingCanonical,
    /// A synonym was detected but its value type (scalar/list/block) did not
    /// match the expected canonical field — rewrite skipped.
    TypeMismatch,
    /// A type synonym was rewritten.
    TypeRewritten,
    /// A rule in the user-supplied YAML references a node type not recognised
    /// by the built-in spec — the rule was ignored.
    UnknownNodeType,
}

// ---------------------------------------------------------------------------
// NormalizeWarning
// ---------------------------------------------------------------------------

/// Advisory emitted during normalization (not a hard error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizeWarning {
    /// Classification code.
    pub code: NormalizeWarningCode,
    /// Human-readable message.
    pub message: String,
    /// Node ID context, if applicable.
    pub node_id: Option<String>,
    /// Source location.
    pub span: Span,
}

// ---------------------------------------------------------------------------
// NormalizeReport
// ---------------------------------------------------------------------------

/// Summary of every rewrite applied (and every advisory raised) during one
/// normalization pass.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizeReport {
    /// Every rewrite applied.
    pub rewrites: Vec<Rewrite>,
    /// Advisory warnings that did not block normalization.
    pub warnings: Vec<NormalizeWarning>,
    /// Ordered list of rule IDs applied during this pass (may contain duplicates).
    pub rules_applied: Vec<String>,
}

impl NormalizeReport {
    /// Returns `true` when no rewrites were applied and no warnings were raised.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rewrites.is_empty() && self.warnings.is_empty()
    }

    /// One-line human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        let r = self.rewrites.len();
        let w = self.warnings.len();
        let rewrite_word = if r == 1 { "rewrite" } else { "rewrites" };
        let warning_word = if w == 1 { "warning" } else { "warnings" };
        format!("{r} {rewrite_word}, {w} {warning_word}")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(rewrites: usize, warnings: usize) -> NormalizeReport {
        let mut r = NormalizeReport::default();
        for i in 0..rewrites {
            r.rewrites.push(Rewrite {
                rule_id: format!("rule.{i}"),
                node_id: Some("n".to_owned()),
                before: "old".to_owned(),
                after: "new".to_owned(),
                span: Span::new(1, 1),
            });
        }
        for i in 0..warnings {
            r.warnings.push(NormalizeWarning {
                code: NormalizeWarningCode::CollisionKeepingCanonical,
                message: format!("warn {i}"),
                node_id: None,
                span: Span::new(2, 2),
            });
        }
        r
    }

    #[test]
    fn test_report_serde_roundtrip() {
        let report = make_report(2, 1);
        let json = serde_json::to_string(&report).expect("serialize");
        let back: NormalizeReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.rewrites.len(), 2);
        assert_eq!(back.warnings.len(), 1);
        assert_eq!(back.rewrites[0].rule_id, "rule.0");
        assert_eq!(
            back.warnings[0].code,
            NormalizeWarningCode::CollisionKeepingCanonical
        );
    }

    #[test]
    fn test_summary_formatting() {
        let r0 = make_report(0, 0);
        assert_eq!(r0.summary(), "0 rewrites, 0 warnings");

        let r1 = make_report(1, 1);
        assert_eq!(r1.summary(), "1 rewrite, 1 warning");

        let r4 = make_report(4, 2);
        assert_eq!(r4.summary(), "4 rewrites, 2 warnings");
    }

    #[test]
    fn test_is_empty_true_when_no_rewrites_or_warnings() {
        let report = NormalizeReport::default();
        assert!(report.is_empty());
    }

    #[test]
    fn test_is_empty_false_when_has_rewrite() {
        let report = make_report(1, 0);
        assert!(!report.is_empty());
    }

    #[test]
    fn test_is_empty_false_when_has_warning() {
        let report = make_report(0, 1);
        assert!(!report.is_empty());
    }

    #[test]
    fn test_warning_code_serde_roundtrip() {
        let codes = [
            NormalizeWarningCode::CollisionKeepingCanonical,
            NormalizeWarningCode::TypeMismatch,
            NormalizeWarningCode::TypeRewritten,
            NormalizeWarningCode::UnknownNodeType,
        ];
        for code in codes {
            let json = serde_json::to_string(&code).unwrap();
            let back: NormalizeWarningCode = serde_json::from_str(&json).unwrap();
            assert_eq!(back, code);
        }
    }
}
