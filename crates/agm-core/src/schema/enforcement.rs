//! Schema enforcement logic (spec §14.3).
//!
//! Validates a `Node` against its type's schema at a configurable `EnforcementLevel`
//! and returns a list of `AgmError` diagnostics.

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation, Severity};
use crate::model::node::Node;
use crate::model::schema::EnforcementLevel;

use super::registry::{UNIVERSAL_FIELDS, get_schema};

/// Returns `true` if the named field is present (Some and non-empty) on the node.
fn field_is_present(node: &Node, field_name: &str) -> bool {
    match field_name {
        // Always-present fields (not Option)
        "type" => true,
        "summary" => true,

        // Control fields
        "priority" => node.priority.is_some(),
        "stability" => node.stability.is_some(),
        "confidence" => node.confidence.is_some(),
        "status" => node.status.is_some(),

        // Relationship fields
        "depends" => node.depends.is_some(),
        "related_to" => node.related_to.is_some(),
        "replaces" => node.replaces.is_some(),
        "conflicts" => node.conflicts.is_some(),
        "see_also" => node.see_also.is_some(),

        // Operational fields
        "items" => node.items.is_some(),
        "steps" => node.steps.is_some(),
        "fields" => node.fields.is_some(),
        "input" => node.input.is_some(),
        "output" => node.output.is_some(),

        // Explanatory fields
        "detail" => node.detail.is_some(),
        "rationale" => node.rationale.is_some(),
        "tradeoffs" => node.tradeoffs.is_some(),
        "resolution" => node.resolution.is_some(),
        "examples" => node.examples.is_some(),
        "notes" => node.notes.is_some(),

        // Executable fields
        "code" => node.code.is_some(),
        "code_blocks" => node.code_blocks.is_some(),
        "verify" => node.verify.is_some(),
        "agent_context" => node.agent_context.is_some(),
        "target" => node.target.is_some(),

        // Execution state fields
        "execution_status" => node.execution_status.is_some(),
        "executed_by" => node.executed_by.is_some(),
        "executed_at" => node.executed_at.is_some(),
        "execution_log" => node.execution_log.is_some(),
        "retry_count" => node.retry_count.is_some(),

        // Orchestration fields
        "parallel_groups" => node.parallel_groups.is_some(),

        // Memory fields
        "memory" => node.memory.is_some(),

        // Context fields
        "scope" => node.scope.is_some(),
        "applies_when" => node.applies_when.is_some(),
        "valid_from" => node.valid_from.is_some(),
        "valid_until" => node.valid_until.is_some(),
        "tags" => node.tags.is_some(),
        "aliases" => node.aliases.is_some(),
        "keywords" => node.keywords.is_some(),

        // Extension fields
        other => node.extra_fields.contains_key(other),
    }
}

/// Returns the set of all field names currently present on the node.
fn present_fields(node: &Node) -> Vec<&str> {
    let mut present = Vec::new();

    // Always-present
    present.push("type");
    present.push("summary");

    // Control fields
    if node.priority.is_some() {
        present.push("priority");
    }
    if node.stability.is_some() {
        present.push("stability");
    }
    if node.confidence.is_some() {
        present.push("confidence");
    }
    if node.status.is_some() {
        present.push("status");
    }

    // Relationship fields
    if node.depends.is_some() {
        present.push("depends");
    }
    if node.related_to.is_some() {
        present.push("related_to");
    }
    if node.replaces.is_some() {
        present.push("replaces");
    }
    if node.conflicts.is_some() {
        present.push("conflicts");
    }
    if node.see_also.is_some() {
        present.push("see_also");
    }

    // Operational fields
    if node.items.is_some() {
        present.push("items");
    }
    if node.steps.is_some() {
        present.push("steps");
    }
    if node.fields.is_some() {
        present.push("fields");
    }
    if node.input.is_some() {
        present.push("input");
    }
    if node.output.is_some() {
        present.push("output");
    }

    // Explanatory fields
    if node.detail.is_some() {
        present.push("detail");
    }
    if node.rationale.is_some() {
        present.push("rationale");
    }
    if node.tradeoffs.is_some() {
        present.push("tradeoffs");
    }
    if node.resolution.is_some() {
        present.push("resolution");
    }
    if node.examples.is_some() {
        present.push("examples");
    }
    if node.notes.is_some() {
        present.push("notes");
    }

    // Executable fields
    if node.code.is_some() {
        present.push("code");
    }
    if node.code_blocks.is_some() {
        present.push("code_blocks");
    }
    if node.verify.is_some() {
        present.push("verify");
    }
    if node.agent_context.is_some() {
        present.push("agent_context");
    }
    if node.target.is_some() {
        present.push("target");
    }

    // Execution state fields
    if node.execution_status.is_some() {
        present.push("execution_status");
    }
    if node.executed_by.is_some() {
        present.push("executed_by");
    }
    if node.executed_at.is_some() {
        present.push("executed_at");
    }
    if node.execution_log.is_some() {
        present.push("execution_log");
    }
    if node.retry_count.is_some() {
        present.push("retry_count");
    }

    // Orchestration fields
    if node.parallel_groups.is_some() {
        present.push("parallel_groups");
    }

    // Memory fields
    if node.memory.is_some() {
        present.push("memory");
    }

    // Context fields
    if node.scope.is_some() {
        present.push("scope");
    }
    if node.applies_when.is_some() {
        present.push("applies_when");
    }
    if node.valid_from.is_some() {
        present.push("valid_from");
    }
    if node.valid_until.is_some() {
        present.push("valid_until");
    }
    if node.tags.is_some() {
        present.push("tags");
    }
    if node.aliases.is_some() {
        present.push("aliases");
    }
    if node.keywords.is_some() {
        present.push("keywords");
    }

    // Extension fields
    for key in node.extra_fields.keys() {
        present.push(key.as_str());
    }

    present
}

/// Validates a node against its type schema at the given enforcement level.
///
/// Returns an empty `Vec` if no violations are found.
#[must_use]
pub fn validate_schema(node: &Node, level: &EnforcementLevel, file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();

    let Some(schema) = get_schema(&node.node_type) else {
        // Custom type — no schema applies.
        return errors;
    };

    let location = ErrorLocation::full(file_name, node.span.start_line, &node.id);

    // 1. Check REQUIRED fields
    for field in &schema.required {
        // summary is non-optional on Node; V002 handles it elsewhere
        if field == "summary" {
            continue;
        }
        if !field_is_present(node, field) {
            let severity = match level {
                EnforcementLevel::Strict | EnforcementLevel::Standard => Severity::Error,
                EnforcementLevel::Permissive => Severity::Warning,
            };
            errors.push(AgmError::with_severity(
                ErrorCode::V024,
                severity,
                format!(
                    "Node `{}` of type `{}` missing required schema field: `{}`",
                    node.id, node.node_type, field
                ),
                location.clone(),
            ));
        }
    }

    // 2. Check RECOMMENDED fields
    for field in &schema.recommended {
        if !field_is_present(node, field) {
            match level {
                EnforcementLevel::Strict | EnforcementLevel::Standard => {
                    errors.push(AgmError::with_severity(
                        ErrorCode::V010,
                        Severity::Warning,
                        format!(
                            "Node type `{}` typically includes field `{}` (missing)",
                            node.node_type, field
                        ),
                        location.clone(),
                    ));
                }
                EnforcementLevel::Permissive => {
                    // Silently accept in permissive mode
                }
            }
        }
    }

    // 3. Check DISALLOWED fields
    let present = present_fields(node);
    for field_name in present {
        if UNIVERSAL_FIELDS.contains(&field_name) {
            continue; // universal fields are NEVER flagged
        }
        if schema.disallowed.contains(&field_name.to_owned()) {
            match level {
                EnforcementLevel::Strict => {
                    errors.push(AgmError::with_severity(
                        ErrorCode::V016,
                        Severity::Error,
                        format!(
                            "Disallowed field `{}` on node type `{}` (strict mode)",
                            field_name, node.node_type
                        ),
                        location.clone(),
                    ));
                }
                EnforcementLevel::Standard => {
                    errors.push(AgmError::with_severity(
                        ErrorCode::V017,
                        Severity::Warning,
                        format!(
                            "Disallowed field `{}` on node type `{}` (standard mode)",
                            field_name, node.node_type
                        ),
                        location.clone(),
                    ));
                }
                EnforcementLevel::Permissive => {
                    // Silently accept
                }
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::model::fields::{FieldValue, NodeType, Span};
    use crate::model::node::Node;
    use crate::model::schema::EnforcementLevel;

    fn minimal_node() -> Node {
        Node {
            id: "test.node".to_owned(),
            node_type: NodeType::Facts,
            summary: "a test node".to_owned(),
            priority: None,
            stability: None,
            confidence: None,
            status: None,
            depends: None,
            related_to: None,
            replaces: None,
            conflicts: None,
            see_also: None,
            items: None,
            steps: None,
            fields: None,
            input: None,
            output: None,
            detail: None,
            rationale: None,
            tradeoffs: None,
            resolution: None,
            examples: None,
            notes: None,
            code: None,
            code_blocks: None,
            verify: None,
            agent_context: None,
            target: None,
            execution_status: None,
            executed_by: None,
            executed_at: None,
            execution_log: None,
            retry_count: None,
            parallel_groups: None,
            memory: None,
            scope: None,
            applies_when: None,
            valid_from: None,
            valid_until: None,
            tags: None,
            aliases: None,
            keywords: None,
            extra_fields: BTreeMap::new(),
            span: Span::new(1, 1),
        }
    }

    // -------------------------------------------------------------------------
    // Required field checks
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_schema_entity_missing_fields_strict_returns_error() {
        let mut node = minimal_node();
        node.node_type = NodeType::Entity;
        // Entity requires "fields" — not set
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        let v024_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V024 && e.severity == Severity::Error)
            .collect();
        assert!(
            !v024_errors.is_empty(),
            "expected V024 error for missing 'fields'"
        );
        assert!(v024_errors.iter().any(|e| e.message.contains("fields")));
    }

    #[test]
    fn test_validate_schema_decision_missing_rationale_standard_returns_error() {
        let mut node = minimal_node();
        node.node_type = NodeType::Decision;
        // Decision requires "rationale" — not set
        let errors = validate_schema(&node, &EnforcementLevel::Standard, "test.agm");
        let v024_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V024 && e.severity == Severity::Error)
            .collect();
        assert!(!v024_errors.is_empty());
        assert!(v024_errors.iter().any(|e| e.message.contains("rationale")));
    }

    #[test]
    fn test_validate_schema_orchestration_missing_parallel_groups_permissive_returns_warning() {
        let mut node = minimal_node();
        node.node_type = NodeType::Orchestration;
        // Orchestration requires "parallel_groups" — not set
        let errors = validate_schema(&node, &EnforcementLevel::Permissive, "test.agm");
        let v024_warnings: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V024 && e.severity == Severity::Warning)
            .collect();
        assert!(!v024_warnings.is_empty());
        assert!(
            v024_warnings
                .iter()
                .any(|e| e.message.contains("parallel_groups"))
        );
    }

    #[test]
    fn test_validate_schema_rules_missing_items_standard_returns_error() {
        let mut node = minimal_node();
        node.node_type = NodeType::Rules;
        // Rules requires "items" — not set
        let errors = validate_schema(&node, &EnforcementLevel::Standard, "test.agm");
        let v024_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V024 && e.severity == Severity::Error)
            .collect();
        assert!(!v024_errors.is_empty());
        assert!(v024_errors.iter().any(|e| e.message.contains("items")));
    }

    // -------------------------------------------------------------------------
    // Recommended field checks
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_schema_workflow_missing_steps_strict_returns_warning() {
        let node = minimal_node(); // Facts type, but we test workflow
        let mut workflow_node = minimal_node();
        workflow_node.node_type = NodeType::Workflow;
        // Workflow recommends "steps" — not set
        let errors = validate_schema(&workflow_node, &EnforcementLevel::Strict, "test.agm");
        let v010_warnings: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V010 && e.severity == Severity::Warning)
            .collect();
        assert!(
            !v010_warnings.is_empty(),
            "expected V010 warning for missing 'steps'"
        );
        // suppress unused warning
        let _ = node;
    }

    #[test]
    fn test_validate_schema_facts_missing_items_standard_returns_warning() {
        let node = minimal_node(); // Facts type, items is recommended
        let errors = validate_schema(&node, &EnforcementLevel::Standard, "test.agm");
        let v010_warnings: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V010 && e.severity == Severity::Warning)
            .collect();
        assert!(!v010_warnings.is_empty());
        assert!(v010_warnings.iter().any(|e| e.message.contains("items")));
    }

    #[test]
    fn test_validate_schema_exception_missing_resolution_permissive_returns_empty() {
        let mut node = minimal_node();
        node.node_type = NodeType::Exception;
        // Exception recommends "resolution" — permissive should produce no V010
        let errors = validate_schema(&node, &EnforcementLevel::Permissive, "test.agm");
        let v010: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V010)
            .collect();
        assert!(
            v010.is_empty(),
            "permissive mode should not warn on missing recommended fields"
        );
    }

    // -------------------------------------------------------------------------
    // Disallowed field checks
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_schema_facts_with_steps_strict_returns_v016_error() {
        let mut node = minimal_node(); // Facts type
        node.steps = Some(vec!["step one".to_owned()]);
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        let v016_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V016 && e.severity == Severity::Error)
            .collect();
        assert!(!v016_errors.is_empty());
        assert!(v016_errors.iter().any(|e| e.message.contains("steps")));
    }

    #[test]
    fn test_validate_schema_entity_with_code_standard_returns_v017_warning() {
        let mut node = minimal_node();
        node.node_type = NodeType::Entity;
        // Entity disallows "code"
        use crate::model::code::{CodeAction, CodeBlock};
        node.code = Some(CodeBlock {
            lang: Some("rust".to_owned()),
            body: "fn main() {}".to_owned(),
            anchor: None,
            target: None,
            action: CodeAction::Full,
            old: None,
        });
        let errors = validate_schema(&node, &EnforcementLevel::Standard, "test.agm");
        let v017_warnings: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V017 && e.severity == Severity::Warning)
            .collect();
        assert!(!v017_warnings.is_empty());
        assert!(v017_warnings.iter().any(|e| e.message.contains("code")));
    }

    #[test]
    fn test_validate_schema_decision_with_parallel_groups_permissive_returns_empty() {
        let mut node = minimal_node();
        node.node_type = NodeType::Decision;
        // Decision disallows "parallel_groups" — but permissive should ignore
        use crate::model::orchestration::{ParallelGroup, Strategy};
        node.parallel_groups = Some(vec![ParallelGroup {
            group: "g1".to_owned(),
            nodes: vec!["n1".to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        }]);
        // Decision also requires "rationale" — add it to isolate the disallowed check
        node.rationale = Some(vec!["some rationale".to_owned()]);
        let errors = validate_schema(&node, &EnforcementLevel::Permissive, "test.agm");
        let disallowed_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V016 || e.code == ErrorCode::V017)
            .collect();
        assert!(disallowed_errors.is_empty());
    }

    #[test]
    fn test_validate_schema_glossary_with_rationale_strict_returns_v016_error() {
        let mut node = minimal_node();
        node.node_type = NodeType::Glossary;
        node.rationale = Some(vec!["a rationale".to_owned()]);
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        let v016_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V016 && e.severity == Severity::Error)
            .collect();
        assert!(!v016_errors.is_empty());
        assert!(v016_errors.iter().any(|e| e.message.contains("rationale")));
    }

    // -------------------------------------------------------------------------
    // Universal field checks
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_schema_facts_with_depends_strict_returns_empty_disallowed() {
        let mut node = minimal_node(); // Facts type
        // "depends" is universal — never flagged even though Facts doesn't list it in allowed
        node.depends = Some(vec!["other.node".to_owned()]);
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        let disallowed: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V016 || e.code == ErrorCode::V017)
            .collect();
        assert!(
            disallowed.is_empty(),
            "universal field 'depends' should never be flagged"
        );
    }

    #[test]
    fn test_validate_schema_entity_with_all_universal_fields_returns_empty_disallowed() {
        let mut node = minimal_node();
        node.node_type = NodeType::Entity;
        node.fields = Some(vec!["id: String".to_owned()]); // satisfy required
        node.priority = Some(crate::model::fields::Priority::High);
        node.stability = Some(crate::model::fields::Stability::High);
        node.tags = Some(vec!["core".to_owned()]);
        node.keywords = Some(vec!["data".to_owned()]);
        node.notes = Some("some notes".to_owned());
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        let disallowed: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V016 || e.code == ErrorCode::V017)
            .collect();
        assert!(disallowed.is_empty());
    }

    #[test]
    fn test_validate_schema_orchestration_with_notes_standard_returns_empty_disallowed() {
        let mut node = minimal_node();
        node.node_type = NodeType::Orchestration;
        node.notes = Some("some notes".to_owned());
        // "notes" is universal — not flagged even though orchestration has disallowed fields
        let errors = validate_schema(&node, &EnforcementLevel::Standard, "test.agm");
        let disallowed: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V016 || e.code == ErrorCode::V017)
            .collect();
        assert!(disallowed.is_empty());
    }

    // -------------------------------------------------------------------------
    // Custom type passthrough
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_schema_custom_type_with_all_fields_returns_empty() {
        let mut node = minimal_node();
        node.node_type = NodeType::Custom("my_type".to_owned());
        node.steps = Some(vec!["step".to_owned()]);
        node.rationale = Some(vec!["reason".to_owned()]);
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_schema_custom_type_no_required_fields_returns_empty() {
        let mut node = minimal_node();
        node.node_type = NodeType::Custom("another_type".to_owned());
        let errors = validate_schema(&node, &EnforcementLevel::Standard, "test.agm");
        assert!(errors.is_empty());
    }

    // -------------------------------------------------------------------------
    // Edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_schema_node_with_only_required_fields_returns_empty_errors() {
        // Workflow only requires summary — no V024 errors expected
        let mut node = minimal_node();
        node.node_type = NodeType::Workflow;
        let errors = validate_schema(&node, &EnforcementLevel::Standard, "test.agm");
        let hard_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.severity == Severity::Error)
            .collect();
        assert!(
            hard_errors.is_empty(),
            "workflow with only summary should have no errors (only warnings for recommended)"
        );
    }

    #[test]
    fn test_validate_schema_node_with_all_valid_fields_returns_empty() {
        let mut node = minimal_node();
        node.node_type = NodeType::Entity;
        node.fields = Some(vec!["id: String".to_owned()]);
        node.stability = Some(crate::model::fields::Stability::High);
        node.priority = Some(crate::model::fields::Priority::Normal);
        node.detail = Some("detailed description".to_owned());
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        assert!(
            errors.is_empty(),
            "entity with all required + recommended + allowed should be clean"
        );
    }

    #[test]
    fn test_validate_schema_multiple_violations_returns_all() {
        let mut node = minimal_node();
        node.node_type = NodeType::Entity;
        // Entity requires: fields (missing)
        // Entity disallows: steps, rationale, resolution, parallel_groups, code
        node.steps = Some(vec!["a step".to_owned()]);
        node.rationale = Some(vec!["a reason".to_owned()]);
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        // Should have V024 for missing "fields", V016 for "steps", V016 for "rationale"
        assert!(
            errors.len() >= 3,
            "expected at least 3 violations, got {}",
            errors.len()
        );
    }

    #[test]
    fn test_validate_schema_summary_not_double_reported() {
        // summary is in Facts required list but should be skipped
        let node = minimal_node(); // Facts, summary is present (non-optional)
        let errors = validate_schema(&node, &EnforcementLevel::Strict, "test.agm");
        let v024_for_summary: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::V024 && e.message.contains("summary"))
            .collect();
        assert!(
            v024_for_summary.is_empty(),
            "summary should never produce V024"
        );
    }

    // -------------------------------------------------------------------------
    // Field presence helper
    // -------------------------------------------------------------------------

    #[test]
    fn test_field_is_present_option_some_returns_true() {
        let mut node = minimal_node();
        node.items = Some(vec!["item one".to_owned()]);
        assert!(field_is_present(&node, "items"));
    }

    #[test]
    fn test_field_is_present_option_none_returns_false() {
        let node = minimal_node();
        assert!(!field_is_present(&node, "items"));
    }

    #[test]
    fn test_field_is_present_extra_field_returns_true() {
        let mut node = minimal_node();
        node.extra_fields.insert(
            "custom_key".to_owned(),
            FieldValue::Scalar("val".to_owned()),
        );
        assert!(field_is_present(&node, "custom_key"));
    }
}
