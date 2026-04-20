//! AST-level normalization engine: walks nodes and applies field/type rewrites.

use std::collections::HashMap;

use crate::model::fields::{FieldValue, NodeType};
use crate::model::file::AgmFile;

use super::report::{NormalizeReport, NormalizeWarning, NormalizeWarningCode, Rewrite};
use super::{CANONICAL_NODE_TYPES, NormalizeConfig};

// ---------------------------------------------------------------------------
// ReverseIndex
// ---------------------------------------------------------------------------

/// Pre-built reverse lookup table: `(node_type_str, synonym) → canonical`.
/// Built once per `NormalizeConfig` so the rewrite pass is O(fields) per node.
struct ReverseIndex {
    /// `(node_type, synonym) → canonical`
    per_type: HashMap<(String, String), String>,
    /// `synonym → canonical` (universal)
    universal: HashMap<String, String>,
}

impl ReverseIndex {
    fn build(config: &NormalizeConfig) -> Self {
        let mut per_type: HashMap<(String, String), String> = HashMap::new();
        let mut universal: HashMap<String, String> = HashMap::new();

        for (node_type, field_map) in &config.rules.field_aliases {
            for (canonical, synonyms) in field_map {
                for synonym in synonyms {
                    per_type.insert((node_type.clone(), synonym.clone()), canonical.clone());
                }
            }
        }

        for (canonical, synonyms) in &config.rules.universal_field_aliases {
            for synonym in synonyms {
                universal.insert(synonym.clone(), canonical.clone());
            }
        }

        Self {
            per_type,
            universal,
        }
    }

    /// Returns the canonical field name for `(node_type, synonym)`, checking
    /// per-type rules first, then universal.
    fn resolve<'a>(&'a self, node_type: &str, synonym: &str) -> Option<&'a str> {
        if let Some(c) = self
            .per_type
            .get(&(node_type.to_owned(), synonym.to_owned()))
        {
            return Some(c.as_str());
        }
        self.universal.get(synonym).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Normalizes an `AgmFile` in place. Applies type-level rewrites first
/// (if `config.normalize_types`), then field-level rewrites (if
/// `config.normalize_fields`). Returns the full normalization report.
#[must_use]
pub fn normalize_ast(file: &mut AgmFile, config: &NormalizeConfig) -> NormalizeReport {
    let mut report = NormalizeReport::default();

    // Pass 1 — type-level rewrites (must run before field-level so per-type
    // field rules apply to the correct canonical type).
    if config.normalize_types {
        for node in &mut file.nodes {
            if let NodeType::Custom(ref s) = node.node_type.clone() {
                if let Some(canonical) = config.rules.canonical_type(s) {
                    let new_type: NodeType = canonical
                        .parse()
                        .expect("canonical_type returned a non-parseable type string");
                    let rule_id = format!("type.{canonical}");
                    report.rewrites.push(Rewrite {
                        rule_id: rule_id.clone(),
                        node_id: Some(node.id.clone()),
                        before: s.clone(),
                        after: canonical.to_owned(),
                        span: node.span.clone(),
                    });
                    report.rules_applied.push(rule_id);
                    report.warnings.push(NormalizeWarning {
                        code: NormalizeWarningCode::TypeRewritten,
                        message: format!(
                            "type '{}' rewritten to '{}' on node '{}'",
                            s, canonical, node.id
                        ),
                        node_id: Some(node.id.clone()),
                        span: node.span.clone(),
                    });
                    node.node_type = new_type;
                }
            }
        }
    }

    // Pass 2 — field-level rewrites.
    if config.normalize_fields {
        let index = ReverseIndex::build(config);

        for node in &mut file.nodes {
            let node_type_str = node.node_type.to_string();

            // Collect the synonym keys that need action (borrow checker: collect before mutating).
            let keys: Vec<String> = node.extra_fields.keys().cloned().collect();

            for synonym in keys {
                let Some(canonical) = index.resolve(&node_type_str, &synonym) else {
                    // Not a known synonym — leave it alone.
                    continue;
                };

                // Check for unknown node type in user rules (informational only).
                // We do this by checking if the canonical type this rule belongs to
                // is a recognised canonical type. UnknownNodeType warning is raised
                // when a per-type rule targets a type not in the canonical list.
                // (Universal rules are always valid.)
                if index
                    .per_type
                    .contains_key(&(node_type_str.clone(), synonym.clone()))
                {
                    let rule_node_type = &node_type_str;
                    if !CANONICAL_NODE_TYPES.contains(&rule_node_type.as_str()) {
                        report.warnings.push(NormalizeWarning {
                            code: NormalizeWarningCode::UnknownNodeType,
                            message: format!(
                                "rule for unknown node type '{}' applied to node '{}'",
                                rule_node_type, node.id
                            ),
                            node_id: Some(node.id.clone()),
                            span: node.span.clone(),
                        });
                    }
                }

                let synonym_value = node.extra_fields.remove(&synonym).unwrap();
                let rule_id = format!("field.{node_type_str}.{canonical}");
                let synonym_span = node.span.clone();

                // Determine the expected kind for the canonical target field.
                let expected_list = canonical_field_expects_list(canonical);

                // Kind check.
                let kind_ok = matches!(
                    (&synonym_value, expected_list),
                    (FieldValue::List(_), true)
                        | (FieldValue::Scalar(_), false)
                        | (FieldValue::Block(_), false)
                );

                if !kind_ok {
                    // TypeMismatch: synonym has wrong value kind — skip rewrite.
                    report.warnings.push(NormalizeWarning {
                        code: NormalizeWarningCode::TypeMismatch,
                        message: format!(
                            "field '{}' on node '{}': synonym '{}' has wrong value kind for canonical '{}' — rewrite skipped",
                            canonical, node.id, synonym, canonical
                        ),
                        node_id: Some(node.id.clone()),
                        span: synonym_span,
                    });
                    // Put it back so the field is not lost.
                    node.extra_fields.insert(synonym, synonym_value);
                    continue;
                }

                // Check whether the canonical field is already set on the node
                // (as a typed struct field, not in extra_fields).
                let canonical_present = is_canonical_field_set(node, canonical);

                if canonical_present {
                    // Collision: canonical form already present.
                    let canonical_value = get_canonical_field_value(node, canonical);
                    let values_equal = canonical_value.as_ref() == Some(&synonym_value);
                    if values_equal {
                        // Silent drop — identical values.
                    } else {
                        // Warn and discard synonym (keep canonical).
                        if config.warn_on_collision {
                            report.warnings.push(NormalizeWarning {
                                code: NormalizeWarningCode::CollisionKeepingCanonical,
                                message: format!(
                                    "field '{}' and synonym '{}' both present on node '{}' with different values — keeping canonical",
                                    canonical, synonym, node.id
                                ),
                                node_id: Some(node.id.clone()),
                                span: synonym_span,
                            });
                        }
                    }
                    // synonym value was already removed; canonical is kept as-is.
                } else {
                    // Apply rewrite: move synonym value to canonical field.
                    // For known canonical fields, set the structured field.
                    // For unknown/extra canonical fields, set in extra_fields.
                    if !set_canonical_field(node, canonical, synonym_value.clone()) {
                        // Canonical is not a typed struct field — put into extra_fields.
                        node.extra_fields
                            .insert(canonical.to_owned(), synonym_value);
                    }
                    report.rewrites.push(Rewrite {
                        rule_id: rule_id.clone(),
                        node_id: Some(node.id.clone()),
                        before: synonym.clone(),
                        after: canonical.to_owned(),
                        span: synonym_span,
                    });
                    report.rules_applied.push(rule_id);
                }
            }
        }
    }

    report
}

// ---------------------------------------------------------------------------
// Helpers: typed field introspection
// ---------------------------------------------------------------------------

/// Returns `true` when the canonical field is expected to hold a `FieldValue::List`.
fn canonical_field_expects_list(canonical: &str) -> bool {
    matches!(
        canonical,
        "depends"
            | "related_to"
            | "replaces"
            | "conflicts"
            | "see_also"
            | "tags"
            | "aliases"
            | "keywords"
            | "items"
            | "steps"
            | "fields"
            | "input"
            | "output"
            | "rationale"
            | "tradeoffs"
            | "resolution"
            | "labels"
            | "scope"
            // Structured list fields: when stored via extra_fields they are Lists
            | "parallel_groups"
            | "code_blocks"
            | "verify"
            | "memory"
    )
}

/// Returns `true` when the node already has the canonical field set as a typed
/// struct field (not via `extra_fields`).
fn is_canonical_field_set(node: &crate::model::node::Node, canonical: &str) -> bool {
    match canonical {
        "depends" => node.depends.is_some(),
        "related_to" => node.related_to.is_some(),
        "replaces" => node.replaces.is_some(),
        "conflicts" => node.conflicts.is_some(),
        "see_also" => node.see_also.is_some(),
        "items" => node.items.is_some(),
        "steps" => node.steps.is_some(),
        "fields" => node.fields.is_some(),
        "input" => node.input.is_some(),
        "output" => node.output.is_some(),
        "detail" => node.detail.is_some(),
        "rationale" => node.rationale.is_some(),
        "tradeoffs" => node.tradeoffs.is_some(),
        "resolution" => node.resolution.is_some(),
        "tags" => node.tags.is_some(),
        "aliases" => node.aliases.is_some(),
        "keywords" => node.keywords.is_some(),
        "labels" => node.labels.is_some(),
        "scope" => node.scope.is_some(),
        "parallel_groups" => node.parallel_groups.is_some(),
        "code_blocks" => node.code_blocks.is_some(),
        _ => false,
    }
}

/// Returns the current value of a canonical typed field as a `FieldValue`, for
/// equality comparison with a synonym value.
fn get_canonical_field_value(
    node: &crate::model::node::Node,
    canonical: &str,
) -> Option<FieldValue> {
    match canonical {
        "depends" => node.depends.as_ref().map(|v| FieldValue::List(v.clone())),
        "related_to" => node
            .related_to
            .as_ref()
            .map(|v| FieldValue::List(v.clone())),
        "replaces" => node.replaces.as_ref().map(|v| FieldValue::List(v.clone())),
        "conflicts" => node.conflicts.as_ref().map(|v| FieldValue::List(v.clone())),
        "see_also" => node.see_also.as_ref().map(|v| FieldValue::List(v.clone())),
        "items" => node.items.as_ref().map(|v| FieldValue::List(v.clone())),
        "steps" => node.steps.as_ref().map(|v| FieldValue::List(v.clone())),
        "fields" => node.fields.as_ref().map(|v| FieldValue::List(v.clone())),
        "input" => node.input.as_ref().map(|v| FieldValue::List(v.clone())),
        "output" => node.output.as_ref().map(|v| FieldValue::List(v.clone())),
        "detail" => node.detail.as_ref().map(|v| FieldValue::Block(v.clone())),
        "rationale" => node.rationale.as_ref().map(|v| FieldValue::List(v.clone())),
        "tradeoffs" => node.tradeoffs.as_ref().map(|v| FieldValue::List(v.clone())),
        "resolution" => node
            .resolution
            .as_ref()
            .map(|v| FieldValue::List(v.clone())),
        "tags" => node.tags.as_ref().map(|v| FieldValue::List(v.clone())),
        "aliases" => node.aliases.as_ref().map(|v| FieldValue::List(v.clone())),
        "keywords" => node.keywords.as_ref().map(|v| FieldValue::List(v.clone())),
        "labels" => node.labels.as_ref().map(|v| FieldValue::List(v.clone())),
        "scope" => node.scope.as_ref().map(|v| FieldValue::List(v.clone())),
        _ => None,
    }
}

/// Sets a typed struct field from a `FieldValue`, returning `true` if the field
/// was recognized and set, `false` if it is not a typed struct field.
fn set_canonical_field(
    node: &mut crate::model::node::Node,
    canonical: &str,
    value: FieldValue,
) -> bool {
    match (canonical, value) {
        ("depends", FieldValue::List(v)) => {
            node.depends = Some(v);
            true
        }
        ("related_to", FieldValue::List(v)) => {
            node.related_to = Some(v);
            true
        }
        ("replaces", FieldValue::List(v)) => {
            node.replaces = Some(v);
            true
        }
        ("conflicts", FieldValue::List(v)) => {
            node.conflicts = Some(v);
            true
        }
        ("see_also", FieldValue::List(v)) => {
            node.see_also = Some(v);
            true
        }
        ("items", FieldValue::List(v)) => {
            node.items = Some(v);
            true
        }
        ("steps", FieldValue::List(v)) => {
            node.steps = Some(v);
            true
        }
        ("fields", FieldValue::List(v)) => {
            node.fields = Some(v);
            true
        }
        ("input", FieldValue::List(v)) => {
            node.input = Some(v);
            true
        }
        ("output", FieldValue::List(v)) => {
            node.output = Some(v);
            true
        }
        ("detail", FieldValue::Block(v)) | ("detail", FieldValue::Scalar(v)) => {
            node.detail = Some(v);
            true
        }
        ("rationale", FieldValue::List(v)) => {
            node.rationale = Some(v);
            true
        }
        ("tradeoffs", FieldValue::List(v)) => {
            node.tradeoffs = Some(v);
            true
        }
        ("resolution", FieldValue::List(v)) => {
            node.resolution = Some(v);
            true
        }
        ("tags", FieldValue::List(v)) => {
            node.tags = Some(v);
            true
        }
        ("aliases", FieldValue::List(v)) => {
            node.aliases = Some(v);
            true
        }
        ("keywords", FieldValue::List(v)) => {
            node.keywords = Some(v);
            true
        }
        ("labels", FieldValue::List(v)) => {
            node.labels = Some(v);
            true
        }
        ("scope", FieldValue::List(v)) => {
            node.scope = Some(v);
            true
        }
        // parallel_groups and code_blocks are structured fields — cannot be set
        // from a raw FieldValue. They land in extra_fields instead.
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::model::fields::{FieldValue, NodeType};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;
    use crate::normalize::{NormalizeConfig, normalize_ast};

    fn minimal_file(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: Header {
                agm: "1.0".to_owned(),
                package: "test".to_owned(),
                version: "0.1.0".to_owned(),
                title: None,
                owner: None,
                imports: None,
                default_load: None,
                description: None,
                tags: None,
                status: None,
                load_profiles: None,
                target_runtime: None,
            },
            nodes,
        }
    }

    fn workflow_node_with_extra(key: &str, value: FieldValue) -> Node {
        let mut n = Node {
            id: "n1".to_owned(),
            node_type: NodeType::Workflow,
            summary: "test".to_owned(),
            ..Default::default()
        };
        n.extra_fields.insert(key.to_owned(), value);
        n
    }

    fn orchestration_node_with_extra(key: &str, value: FieldValue) -> Node {
        let mut n = Node {
            id: "orch1".to_owned(),
            node_type: NodeType::Orchestration,
            summary: "plan node".to_owned(),
            ..Default::default()
        };
        n.extra_fields.insert(key.to_owned(), value);
        n
    }

    #[test]
    fn test_rewrite_extra_field_to_canonical() {
        let node = workflow_node_with_extra(
            "depends_on",
            FieldValue::List(vec!["other.node".to_owned()]),
        );
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report = normalize_ast(&mut file, &config);

        assert!(!report.rewrites.is_empty(), "expected a rewrite");
        let n = &file.nodes[0];
        assert!(!n.extra_fields.contains_key("depends_on"));
        assert_eq!(n.depends, Some(vec!["other.node".to_owned()]));
    }

    #[test]
    fn test_rewrite_canonical_only_is_noop() {
        let mut node = Node {
            id: "n1".to_owned(),
            node_type: NodeType::Workflow,
            summary: "test".to_owned(),
            ..Default::default()
        };
        node.depends = Some(vec!["other".to_owned()]);
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report = normalize_ast(&mut file, &config);
        assert!(report.rewrites.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn test_rewrite_collision_same_value_silent() {
        let mut node = Node {
            id: "n1".to_owned(),
            node_type: NodeType::Workflow,
            summary: "test".to_owned(),
            ..Default::default()
        };
        node.depends = Some(vec!["other".to_owned()]);
        // Same value as synonym
        node.extra_fields.insert(
            "depends_on".to_owned(),
            FieldValue::List(vec!["other".to_owned()]),
        );
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report = normalize_ast(&mut file, &config);
        // Silent drop — no rewrites recorded, no warnings
        assert!(report.rewrites.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn test_rewrite_collision_different_value_warns() {
        use super::NormalizeWarningCode;
        let mut node = Node {
            id: "n1".to_owned(),
            node_type: NodeType::Workflow,
            summary: "test".to_owned(),
            ..Default::default()
        };
        node.depends = Some(vec!["a".to_owned()]);
        node.extra_fields.insert(
            "depends_on".to_owned(),
            FieldValue::List(vec!["b".to_owned()]),
        );
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report = normalize_ast(&mut file, &config);
        assert!(report.rewrites.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(
            report.warnings[0].code,
            NormalizeWarningCode::CollisionKeepingCanonical
        );
        // canonical was kept
        assert_eq!(file.nodes[0].depends, Some(vec!["a".to_owned()]));
    }

    #[test]
    fn test_type_mismatch_warns_no_rewrite() {
        use super::NormalizeWarningCode;
        // depends_on expects a List, give it a Scalar
        let node = workflow_node_with_extra("depends_on", FieldValue::Scalar("x".to_owned()));
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report = normalize_ast(&mut file, &config);
        assert!(report.rewrites.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].code, NormalizeWarningCode::TypeMismatch);
        // field must still be present
        assert!(file.nodes[0].extra_fields.contains_key("depends_on"));
    }

    #[test]
    fn test_type_level_rewrite_custom_to_orchestration() {
        let mut node = Node {
            id: "plan1".to_owned(),
            node_type: NodeType::Custom("plan_execution".to_owned()),
            summary: "the plan".to_owned(),
            ..Default::default()
        };
        node.extra_fields
            .insert("groups".to_owned(), FieldValue::List(vec!["g1".to_owned()]));
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report = normalize_ast(&mut file, &config);

        // Type should be rewritten
        assert_eq!(file.nodes[0].node_type, NodeType::Orchestration);
        // Field rewrite: "groups" → "parallel_groups" (but parallel_groups is structured,
        // so it lands in extra_fields under the canonical name)
        assert!(!file.nodes[0].extra_fields.contains_key("groups"));
        assert!(
            file.nodes[0].extra_fields.contains_key("parallel_groups")
                || file.nodes[0].parallel_groups.is_some()
        );
        // Type rewrite should be in report
        let type_rewrites: Vec<_> = report
            .rewrites
            .iter()
            .filter(|r| r.rule_id.starts_with("type."))
            .collect();
        assert!(!type_rewrites.is_empty());
    }

    #[test]
    fn test_normalize_idempotent_on_canonical_input() {
        let mut node = Node {
            id: "n1".to_owned(),
            node_type: NodeType::Workflow,
            summary: "test".to_owned(),
            ..Default::default()
        };
        node.depends = Some(vec!["a".to_owned()]);
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report1 = normalize_ast(&mut file, &config);
        assert!(
            report1.is_empty(),
            "canonical input should produce empty report"
        );
        let report2 = normalize_ast(&mut file, &config);
        assert!(
            report2.is_empty(),
            "second pass on canonical should also be empty"
        );
    }

    #[test]
    fn test_normalize_idempotent_on_normalized_output() {
        let node =
            workflow_node_with_extra("depends_on", FieldValue::List(vec!["other".to_owned()]));
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report1 = normalize_ast(&mut file, &config);
        assert!(!report1.rewrites.is_empty());
        // Second pass must be a no-op
        let report2 = normalize_ast(&mut file, &config);
        assert!(
            report2.rewrites.is_empty(),
            "second pass must produce no rewrites"
        );
    }

    #[test]
    fn test_no_types_flag_disables_type_rewrite() {
        let node = Node {
            id: "p1".to_owned(),
            node_type: NodeType::Custom("plan_execution".to_owned()),
            summary: "test".to_owned(),
            ..Default::default()
        };
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig {
            normalize_types: false,
            ..NormalizeConfig::default()
        };
        let report = normalize_ast(&mut file, &config);
        // Type rewrite disabled — should remain Custom
        assert_eq!(
            file.nodes[0].node_type,
            NodeType::Custom("plan_execution".to_owned())
        );
        let type_rewrites: Vec<_> = report
            .rewrites
            .iter()
            .filter(|r| r.rule_id.starts_with("type."))
            .collect();
        assert!(type_rewrites.is_empty());
    }

    #[test]
    fn test_orchestration_groups_synonym() {
        let node = orchestration_node_with_extra(
            "groups",
            FieldValue::List(vec!["g1".to_owned(), "g2".to_owned()]),
        );
        let mut file = minimal_file(vec![node]);
        let config = NormalizeConfig::default();
        let report = normalize_ast(&mut file, &config);
        assert!(!report.rewrites.is_empty());
        let n = &file.nodes[0];
        assert!(!n.extra_fields.contains_key("groups"));
        // "parallel_groups" as a FieldValue::List lands in extra_fields
        // since ParallelGroup is a structured type not settable from FieldValue
        assert!(n.extra_fields.contains_key("parallel_groups") || n.parallel_groups.is_some());
    }
}
