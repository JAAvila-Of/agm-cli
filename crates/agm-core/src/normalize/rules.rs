//! Rule-set types and loading logic for the normalizer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// RuleParseError
// ---------------------------------------------------------------------------

/// Error returned when loading a rule set from YAML fails.
#[derive(Debug, thiserror::Error)]
pub enum RuleParseError {
    #[error("invalid YAML: {0}")]
    Yaml(String),
    #[error("empty rule file")]
    Empty,
}

// ---------------------------------------------------------------------------
// RuleSet
// ---------------------------------------------------------------------------

/// A collection of field-alias and type-alias rules keyed by canonical form.
///
/// The rule set drives the normalization engine:
/// - `type_aliases`: maps a canonical node type to its accepted synonyms.
/// - `field_aliases`: per-type map of canonical field name → synonyms.
/// - `universal_field_aliases`: field aliases that apply regardless of node type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleSet {
    /// Map: canonical node type → { canonical field → [synonyms] }
    #[serde(default)]
    pub field_aliases: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// Map: canonical node type → [synonyms]
    #[serde(default)]
    pub type_aliases: BTreeMap<String, Vec<String>>,
    /// Field aliases applied to every node type.
    #[serde(default)]
    pub universal_field_aliases: BTreeMap<String, Vec<String>>,
}

impl RuleSet {
    /// Returns the built-in v1.2.0 rule set, loaded from the embedded YAML.
    ///
    /// The embedded YAML is validated at test time; it cannot fail at runtime
    /// unless the binary was built with a corrupted asset.
    #[must_use]
    pub fn builtin() -> Self {
        let yaml = include_str!("builtin_rules.yaml");
        // SAFETY: the embedded YAML is checked by the `test_builtin_parses` unit
        // test at build time. Unwrapping here is intentional — if the embedded
        // asset is broken the binary itself is broken.
        serde_yaml::from_str(yaml).expect("built-in rule set YAML is malformed")
    }

    /// Loads a rule set from a YAML string. Used for `--rules` / `.agm-normalize.yaml`.
    ///
    /// # Errors
    ///
    /// Returns `RuleParseError::Empty` when the string is blank, or
    /// `RuleParseError::Yaml` when the YAML is syntactically invalid.
    pub fn from_yaml_str(input: &str) -> Result<Self, RuleParseError> {
        if input.trim().is_empty() {
            return Err(RuleParseError::Empty);
        }
        serde_yaml::from_str(input).map_err(|e| RuleParseError::Yaml(e.to_string()))
    }

    /// Merges `other` on top of `self`.
    ///
    /// For each canonical type in `other.type_aliases`, the synonym list in
    /// `other` **replaces** the list in `self` (user override wins). Field
    /// aliases follow the same replacement rule per canonical field.
    pub fn merge(&mut self, other: Self) {
        for (canonical_type, synonyms) in other.type_aliases {
            self.type_aliases.insert(canonical_type, synonyms);
        }
        for (node_type, field_map) in other.field_aliases {
            let entry = self.field_aliases.entry(node_type).or_default();
            for (canonical_field, synonyms) in field_map {
                entry.insert(canonical_field, synonyms);
            }
        }
        for (canonical_field, synonyms) in other.universal_field_aliases {
            self.universal_field_aliases
                .insert(canonical_field, synonyms);
        }
    }

    /// Returns the canonical name for a field synonym on a given node type,
    /// or `None` if the name is already canonical or unknown.
    ///
    /// Lookup order: per-type rules first, then universal rules.
    #[must_use]
    pub fn canonical_field<'a>(&'a self, node_type: &str, name: &str) -> Option<&'a str> {
        // Per-type lookup
        if let Some(field_map) = self.field_aliases.get(node_type) {
            for (canonical, synonyms) in field_map {
                if synonyms.iter().any(|s| s == name) {
                    return Some(canonical.as_str());
                }
            }
        }
        // Universal lookup
        for (canonical, synonyms) in &self.universal_field_aliases {
            if synonyms.iter().any(|s| s == name) {
                return Some(canonical.as_str());
            }
        }
        None
    }

    /// Returns the canonical name for a type synonym, or `None` if the name
    /// is already a canonical type or is completely unknown.
    #[must_use]
    pub fn canonical_type<'a>(&'a self, name: &str) -> Option<&'a str> {
        for (canonical, synonyms) in &self.type_aliases {
            if synonyms.iter().any(|s| s == name) {
                return Some(canonical.as_str());
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_parses() {
        let rs = RuleSet::builtin();
        // type aliases
        assert!(!rs.type_aliases.is_empty());
        // field aliases
        assert!(!rs.field_aliases.is_empty());
        // universal aliases
        assert!(!rs.universal_field_aliases.is_empty());
    }

    #[test]
    fn test_from_yaml_str_happy_path() {
        let yaml = r#"
type_aliases:
  orchestration:
    - plan
field_aliases:
  workflow:
    steps:
      - tasks
universal_field_aliases:
  depends:
    - depends_on
"#;
        let rs = RuleSet::from_yaml_str(yaml).expect("should parse");
        assert!(rs.type_aliases.contains_key("orchestration"));
        assert!(rs.field_aliases.contains_key("workflow"));
        assert!(rs.universal_field_aliases.contains_key("depends"));
    }

    #[test]
    fn test_from_yaml_str_malformed_returns_err() {
        let yaml = "{ invalid yaml: [unclosed";
        let result = RuleSet::from_yaml_str(yaml);
        assert!(result.is_err(), "expected Yaml parse error");
    }

    #[test]
    fn test_from_yaml_str_empty_returns_err() {
        let result = RuleSet::from_yaml_str("   ");
        assert!(matches!(result, Err(RuleParseError::Empty)));
    }

    #[test]
    fn test_merge_user_overrides_builtin() {
        let mut base = RuleSet::builtin();
        let override_yaml = r#"
type_aliases:
  orchestration:
    - my_plan
universal_field_aliases:
  depends:
    - needs_node
"#;
        let user = RuleSet::from_yaml_str(override_yaml).unwrap();
        base.merge(user);
        // Override replaced the builtin list for orchestration
        let orch = base.type_aliases.get("orchestration").unwrap();
        assert!(orch.contains(&"my_plan".to_owned()));
        // Override replaced the builtin universal list for depends
        let dep = base.universal_field_aliases.get("depends").unwrap();
        assert!(dep.contains(&"needs_node".to_owned()));
    }

    #[test]
    fn test_canonical_field_per_type_rule() {
        let rs = RuleSet::builtin();
        // "groups" is a synonym for "parallel_groups" on orchestration
        assert_eq!(
            rs.canonical_field("orchestration", "groups"),
            Some("parallel_groups")
        );
    }

    #[test]
    fn test_canonical_field_universal_fallback() {
        let rs = RuleSet::builtin();
        // "depends_on" is a universal synonym for "depends"
        assert_eq!(
            rs.canonical_field("workflow", "depends_on"),
            Some("depends")
        );
        // Also applies to any arbitrary type
        assert_eq!(rs.canonical_field("facts", "prereq"), Some("depends"));
    }

    #[test]
    fn test_canonical_field_already_canonical_returns_none() {
        let rs = RuleSet::builtin();
        assert_eq!(rs.canonical_field("orchestration", "parallel_groups"), None);
        assert_eq!(rs.canonical_field("workflow", "steps"), None);
    }

    #[test]
    fn test_canonical_field_unknown_returns_none() {
        let rs = RuleSet::builtin();
        assert_eq!(rs.canonical_field("workflow", "not_a_synonym"), None);
    }

    #[test]
    fn test_canonical_type_known_synonym() {
        let rs = RuleSet::builtin();
        assert_eq!(rs.canonical_type("plan_execution"), Some("orchestration"));
        assert_eq!(rs.canonical_type("plan"), Some("orchestration"));
        assert_eq!(rs.canonical_type("execution_plan"), Some("orchestration"));
    }

    #[test]
    fn test_canonical_type_already_canonical_returns_none() {
        let rs = RuleSet::builtin();
        assert_eq!(rs.canonical_type("orchestration"), None);
        assert_eq!(rs.canonical_type("workflow"), None);
    }

    #[test]
    fn test_canonical_type_unknown_returns_none() {
        let rs = RuleSet::builtin();
        assert_eq!(rs.canonical_type("totally_unknown"), None);
    }

    #[test]
    fn test_canonical_field_phases_synonym() {
        let rs = RuleSet::builtin();
        assert_eq!(
            rs.canonical_field("orchestration", "phases"),
            Some("parallel_groups")
        );
    }

    #[test]
    fn test_canonical_field_related_universal() {
        let rs = RuleSet::builtin();
        assert_eq!(rs.canonical_field("facts", "related"), Some("related_to"));
        assert_eq!(
            rs.canonical_field("decision", "relates_to"),
            Some("related_to")
        );
    }

    #[test]
    fn test_merge_adds_new_type() {
        let mut base = RuleSet::default();
        let extra_yaml = r#"
type_aliases:
  workflow:
    - wf
"#;
        let extra = RuleSet::from_yaml_str(extra_yaml).unwrap();
        base.merge(extra);
        assert_eq!(base.canonical_type("wf"), Some("workflow"));
    }
}
