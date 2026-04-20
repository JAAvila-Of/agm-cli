//! Repair rule trait and registry of built-in rules.
//!
//! Canonical rule order (D6):
//!   R-ZERO-WIDTH → R-CRLF → R-TABS-TO-SPACES → R-TRAILING-WS →
//!   R-SMART-QUOTES → R-MISSING-AGM-FENCE → R-PROSE-BEFORE-HEADER →
//!   R-BULLET-STAR → R-BARE-CODE-BLOCK
//!
//! `R-BARE-CODE-BLOCK` is disabled by default.

pub(crate) mod bullets;
pub(crate) mod code_fence;
pub(crate) mod prose_prefix;
pub(crate) mod smart_quotes;
pub(crate) mod whitespace;

use crate::repair::report::RepairRecord;

// ---------------------------------------------------------------------------
// RepairRule trait
// ---------------------------------------------------------------------------

/// A single text-level repair rule.
pub(crate) trait RepairRule: Send + Sync {
    /// Stable rule identifier (e.g. `"R-SMART-QUOTES"`). Never changes across releases.
    fn id(&self) -> &'static str;

    /// Short human-readable description of what this rule does.
    #[allow(dead_code)]
    fn description(&self) -> &'static str;

    /// Apply the rule to `input`, returning `(output_text, records)`.
    /// If the rule made no changes, returns `(input.to_owned(), vec![])`.
    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>);

    /// Whether this rule is enabled by default.
    fn enabled_by_default(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Built-in rule set
// ---------------------------------------------------------------------------

/// Return the canonical ordered list of all built-in rules.
///
/// `R-BARE-CODE-BLOCK` is last and has `enabled_by_default() == false`.
pub(crate) fn builtin_rules() -> Vec<Box<dyn RepairRule>> {
    vec![
        Box::new(whitespace::ZeroWidthRule),
        Box::new(whitespace::CrlfRule),
        Box::new(whitespace::TabsToSpacesRule),
        Box::new(whitespace::TrailingWsRule),
        Box::new(smart_quotes::SmartQuotesRule),
        Box::new(prose_prefix::MissingAgmFenceRule),
        Box::new(prose_prefix::ProsePrefixRule),
        Box::new(bullets::BulletStarRule),
        Box::new(DisabledByDefault(code_fence::BareCodeBlockRule)),
    ]
}

/// Wraps a rule and overrides `enabled_by_default()` to return `false`.
struct DisabledByDefault<R: RepairRule>(R);

impl<R: RepairRule> RepairRule for DisabledByDefault<R> {
    fn id(&self) -> &'static str {
        self.0.id()
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }

    fn apply(&self, input: &str) -> (String, Vec<RepairRecord>) {
        self.0.apply(input)
    }

    fn enabled_by_default(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_rules_has_expected_ids() {
        let rules = builtin_rules();
        let ids: Vec<&str> = rules.iter().map(|r| r.id()).collect();
        assert!(ids.contains(&"R-ZERO-WIDTH"));
        assert!(ids.contains(&"R-CRLF"));
        assert!(ids.contains(&"R-TABS-TO-SPACES"));
        assert!(ids.contains(&"R-TRAILING-WS"));
        assert!(ids.contains(&"R-SMART-QUOTES"));
        assert!(ids.contains(&"R-MISSING-AGM-FENCE"));
        assert!(ids.contains(&"R-PROSE-BEFORE-HEADER"));
        assert!(ids.contains(&"R-BULLET-STAR"));
        assert!(ids.contains(&"R-BARE-CODE-BLOCK"));
        assert_eq!(ids.len(), 9);
    }

    #[test]
    fn test_canonical_rule_order() {
        let rules = builtin_rules();
        let ids: Vec<&str> = rules.iter().map(|r| r.id()).collect();
        let expected = [
            "R-ZERO-WIDTH",
            "R-CRLF",
            "R-TABS-TO-SPACES",
            "R-TRAILING-WS",
            "R-SMART-QUOTES",
            "R-MISSING-AGM-FENCE",
            "R-PROSE-BEFORE-HEADER",
            "R-BULLET-STAR",
            "R-BARE-CODE-BLOCK",
        ];
        assert_eq!(ids, expected);
    }

    #[test]
    fn test_bare_code_block_disabled_by_default() {
        let rules = builtin_rules();
        let bare = rules
            .iter()
            .find(|r| r.id() == "R-BARE-CODE-BLOCK")
            .unwrap();
        assert!(
            !bare.enabled_by_default(),
            "R-BARE-CODE-BLOCK must be disabled by default"
        );
    }

    #[test]
    fn test_all_other_rules_enabled_by_default() {
        let rules = builtin_rules();
        for rule in &rules {
            if rule.id() != "R-BARE-CODE-BLOCK" {
                assert!(
                    rule.enabled_by_default(),
                    "rule {} should be enabled by default",
                    rule.id()
                );
            }
        }
    }
}
