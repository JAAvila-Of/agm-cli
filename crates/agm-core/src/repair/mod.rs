//! Pre-parse textual repair for non-canonical LLM output.
//!
//! `repair_text` applies a curated set of conservative text-level rewrite rules
//! to raw AGM text that may contain syntax errors such as smart quotes, missing
//! fences, CRLF line endings, tab indentation, or asterisk bullets.
//!
//! After all active rules are applied, a safety-net re-parse step validates the
//! result. If the repaired text still fails to parse, the original is returned
//! and `RepairOutput::rolled_back` is set to `true`.

pub mod report;
pub mod rules;

pub use report::{RepairRecord, RepairReport};

use crate::parser;
use rules::{RepairRule, builtin_rules};

// ---------------------------------------------------------------------------
// RepairConfig
// ---------------------------------------------------------------------------

/// Configuration for a repair pass.
#[derive(Debug, Clone)]
pub struct RepairConfig {
    /// Rule IDs that are explicitly disabled (no-op when `only_rules` is set).
    pub disabled_rules: Vec<&'static str>,
    /// If set, **only** these rule IDs run (overrides `disabled_rules`).
    pub only_rules: Option<Vec<&'static str>>,
    /// When `true` (default), re-parse the repaired text and roll back if it
    /// still fails to parse.
    pub safety_net: bool,
}

impl Default for RepairConfig {
    fn default() -> Self {
        Self {
            disabled_rules: Vec::new(),
            only_rules: None,
            safety_net: true,
        }
    }
}

// ---------------------------------------------------------------------------
// RepairOutput
// ---------------------------------------------------------------------------

/// Result of a `repair_text` call.
#[derive(Debug, Clone)]
pub struct RepairOutput {
    /// Possibly rewritten text. Equals the original if no rules matched or
    /// the safety net rolled back the changes.
    pub text: String,
    /// Ordered list of rewrites (possibly attempted rewrites if rolled back).
    pub report: RepairReport,
    /// `true` when the safety net detected that the repaired text still fails
    /// to parse and reverted to the original.
    pub rolled_back: bool,
}

// ---------------------------------------------------------------------------
// repair_text
// ---------------------------------------------------------------------------

/// Return the stable IDs of all built-in repair rules in canonical order.
///
/// This includes `R-BARE-CODE-BLOCK` even though it is disabled by default.
#[must_use]
pub fn builtin_rule_ids() -> Vec<&'static str> {
    builtin_rules().iter().map(|r| r.id()).collect()
}

/// Apply active repair rules to `raw`, returning a `RepairOutput`.
///
/// Active rules = builtin rules that are enabled by default, minus
/// `disabled_rules`, unless `only_rules` is set in which case only those IDs
/// run (even if they are disabled by default, e.g. `R-BARE-CODE-BLOCK`).
pub fn repair_text(raw: &str, config: &RepairConfig) -> RepairOutput {
    let all_rules = builtin_rules();

    // Select active rules.
    let active_rules: Vec<&dyn RepairRule> = if let Some(only) = &config.only_rules {
        all_rules
            .iter()
            .filter(|r| only.contains(&r.id()))
            .map(|r| r.as_ref() as &dyn RepairRule)
            .collect()
    } else {
        all_rules
            .iter()
            .filter(|r| r.enabled_by_default() && !config.disabled_rules.contains(&r.id()))
            .map(|r| r.as_ref() as &dyn RepairRule)
            .collect()
    };

    // Apply rules sequentially, accumulating records.
    let mut current_text = raw.to_owned();
    let mut all_records: Vec<RepairRecord> = Vec::new();

    for rule in &active_rules {
        let (next_text, mut records) = rule.apply(&current_text);
        current_text = next_text;
        all_records.append(&mut records);
    }

    // Safety-net: re-parse and roll back if still broken.
    if !all_records.is_empty() && config.safety_net {
        if let Err(_parse_errors) = parser::parse(&current_text) {
            // Roll back: return original text but preserve the attempted records.
            let report = RepairReport {
                rewrites: all_records,
                rolled_back: true,
            };
            return RepairOutput {
                text: raw.to_owned(),
                report,
                rolled_back: true,
            };
        }
    }

    let report = RepairReport {
        rewrites: all_records,
        rolled_back: false,
    };

    RepairOutput {
        text: current_text,
        report,
        rolled_back: false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_agm() -> &'static str {
        "agm: 1.0\npackage: test\nversion: 0.1.0\n\nnode mynode\ntype: facts\nsummary: Test node\n"
    }

    #[test]
    fn test_repair_clean_input_unchanged() {
        let config = RepairConfig::default();
        let out = repair_text(minimal_agm(), &config);
        assert_eq!(out.text, minimal_agm());
        assert!(out.report.is_empty());
        assert!(!out.rolled_back);
    }

    #[test]
    fn test_only_rules_flag_respected() {
        // Apply only R-TRAILING-WS; smart quotes should NOT be replaced.
        // Safety net is disabled because the input is not valid AGM.
        let input = "\u{201C}smart\u{201D}   \n";
        let config = RepairConfig {
            only_rules: Some(vec!["R-TRAILING-WS"]),
            safety_net: false,
            ..Default::default()
        };
        let out = repair_text(input, &config);
        // Smart quotes should be preserved (R-SMART-QUOTES not in only_rules)
        assert!(
            out.text.contains('\u{201C}'),
            "smart quotes must be preserved when not in only_rules"
        );
        // Trailing whitespace should be stripped
        assert!(
            !out.text.contains("   "),
            "trailing whitespace should be removed"
        );
    }

    #[test]
    fn test_disabled_rule_not_applied() {
        let input = "items:\n* one\n* two\n";
        let config = RepairConfig {
            disabled_rules: vec!["R-BULLET-STAR"],
            ..Default::default()
        };
        let out = repair_text(input, &config);
        // Bullets should NOT have been rewritten
        assert!(out.text.contains("* one"), "disabled rule should not apply");
    }

    #[test]
    fn test_safety_net_rolls_back_bad_repair() {
        // Craft input where applying R-MISSING-AGM-FENCE strips the fence but
        // the result is still unparseable (the inner content is garbage).
        // We test this by running a config with safety_net:true on content that
        // cannot parse.
        let garbage = "```agm\nthis is not valid AGM at all!\n???\n```\n";
        let config = RepairConfig {
            safety_net: true,
            only_rules: Some(vec!["R-MISSING-AGM-FENCE"]),
            ..Default::default()
        };
        let out = repair_text(garbage, &config);
        // The safety net should have rolled back
        assert!(
            out.rolled_back,
            "safety net should have triggered on unparseable output"
        );
        assert_eq!(
            out.text, garbage,
            "original text should be preserved on rollback"
        );
        // Records are still present (for debugging)
        assert!(
            !out.report.rewrites.is_empty(),
            "records preserved even on rollback"
        );
    }

    #[test]
    fn test_idempotency() {
        let messy = "Here's the AGM:\n\nagm: 1.0\npackage: test\nversion: 0.1.0  \n\nnode mynode\ntype: facts\nsummary: \u{201C}test\u{201D}\n";
        let config = RepairConfig {
            safety_net: false, // messy content may not parse even after repair
            ..Default::default()
        };
        let first = repair_text(messy, &config);
        let second = repair_text(&first.text, &config);
        assert_eq!(
            first.text, second.text,
            "second pass must be identical to first pass (idempotency)"
        );
        assert!(
            second.report.is_empty(),
            "second pass must produce no records"
        );
    }

    #[test]
    fn test_no_safety_net_keeps_repaired_text() {
        let input = "```agm\nnot valid content\n```\n";
        let config = RepairConfig {
            safety_net: false,
            only_rules: Some(vec!["R-MISSING-AGM-FENCE"]),
            ..Default::default()
        };
        let out = repair_text(input, &config);
        // With no safety net, the rewritten text is kept even if it won't parse
        assert!(!out.rolled_back);
        assert!(!out.text.contains("```"));
    }

    #[test]
    fn test_multiple_rules_applied_in_order() {
        // CRLF + smart quotes
        let input = "summary: \u{201C}hello\u{201D}\r\npackage: test\r\n";
        let config = RepairConfig {
            safety_net: false,
            ..Default::default()
        };
        let out = repair_text(input, &config);
        assert!(!out.text.contains('\r'), "CRLF should be normalized");
        assert!(
            !out.text.contains('\u{201C}'),
            "smart quotes should be replaced"
        );
    }
}
