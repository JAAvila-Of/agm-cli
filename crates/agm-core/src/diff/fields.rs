//! Field-level diff within a single node, and severity classification.

use serde::{Deserialize, Serialize};

use crate::model::fields::FieldValue;
use crate::model::node::Node;

use super::{ChangeKind, ChangeSeverity};

/// A serializable snapshot of a field value for display in reports.
///
/// Unlike `FieldValue` from the model (which has `Scalar`, `List`, `Block`),
/// this type adds representation for complex fields (code blocks, verify
/// checks, agent context, etc.) that are serialized as JSON strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValueSnapshot {
    /// A single scalar string.
    Scalar(String),
    /// A list of strings.
    List(Vec<String>),
    /// A block of text.
    Block(String),
    /// Complex structured value serialized to JSON string for display.
    Complex(String),
}

impl FieldValueSnapshot {
    /// Converts a `FieldValue` to a snapshot.
    pub(crate) fn from_field_value(v: &FieldValue) -> Self {
        match v {
            FieldValue::Scalar(s) => Self::Scalar(s.clone()),
            FieldValue::List(l) => Self::List(l.clone()),
            FieldValue::Block(b) => Self::Block(b.clone()),
        }
    }

    /// Creates a scalar snapshot from a string.
    pub(crate) fn from_str_val(s: &str) -> Self {
        Self::Scalar(s.to_owned())
    }

    /// Creates a complex snapshot by serializing a value to a JSON string.
    pub(crate) fn from_complex<T: Serialize>(v: &T) -> Self {
        Self::Complex(serde_json::to_string(v).unwrap_or_default())
    }
}

/// A change to a single field within a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub kind: ChangeKind,
    pub severity: ChangeSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<FieldValueSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<FieldValueSnapshot>,
}

/// Classifies the severity of a change to a given field.
///
/// See the severity classification table in section 4.1 of the plan.
#[must_use]
pub(crate) fn classify_severity(field_name: &str, kind: ChangeKind) -> ChangeSeverity {
    match (field_name, kind) {
        // Identity & structure
        ("node_type" | "type", _) => ChangeSeverity::Breaking,

        // Required fields
        ("summary", _) => ChangeSeverity::Minor,

        // Control fields
        ("priority" | "stability" | "confidence" | "status", ChangeKind::Added) => {
            ChangeSeverity::Info
        }
        ("priority" | "stability" | "confidence" | "status", _) => ChangeSeverity::Minor,

        // Relationship: depends
        ("depends", ChangeKind::Removed) => ChangeSeverity::Breaking,
        ("depends", ChangeKind::Added) => ChangeSeverity::Minor,
        ("depends", ChangeKind::Modified) => ChangeSeverity::Minor,

        // Relationship: related_to, see_also
        ("related_to" | "see_also", _) => ChangeSeverity::Info,

        // Relationship: replaces, conflicts
        ("replaces" | "conflicts", _) => ChangeSeverity::Minor,

        // Operational fields: items
        ("items", _) => ChangeSeverity::Minor,

        // Operational fields: steps (removed = breaking, added = minor)
        ("steps", ChangeKind::Removed) => ChangeSeverity::Breaking,
        ("steps", _) => ChangeSeverity::Minor,

        // Operational fields: fields, input, output (removed = breaking)
        ("fields" | "input" | "output", ChangeKind::Removed) => ChangeSeverity::Breaking,
        ("fields" | "input" | "output", _) => ChangeSeverity::Minor,

        // Explanatory fields
        ("detail" | "examples" | "notes", _) => ChangeSeverity::Info,
        ("rationale" | "tradeoffs" | "resolution", _) => ChangeSeverity::Info,

        // Executable fields
        ("code" | "code_blocks", ChangeKind::Removed) => ChangeSeverity::Breaking,
        ("code" | "code_blocks", _) => ChangeSeverity::Minor,
        ("verify", _) => ChangeSeverity::Minor,
        ("agent_context", ChangeKind::Added) => ChangeSeverity::Info,
        ("agent_context", _) => ChangeSeverity::Minor,
        ("target", ChangeKind::Removed) => ChangeSeverity::Breaking,
        ("target", _) => ChangeSeverity::Minor,

        // Execution state fields
        (
            "execution_status" | "executed_by" | "executed_at" | "execution_log" | "retry_count",
            _,
        ) => ChangeSeverity::Info,

        // Orchestration fields
        ("parallel_groups", ChangeKind::Removed) => ChangeSeverity::Breaking,
        ("parallel_groups", _) => ChangeSeverity::Minor,

        // Memory fields
        ("memory", ChangeKind::Added) => ChangeSeverity::Info,
        ("memory", _) => ChangeSeverity::Minor,

        // Context fields: scope
        ("scope", ChangeKind::Added) => ChangeSeverity::Info,
        ("scope", _) => ChangeSeverity::Minor,

        // Context fields: applies_when
        ("applies_when", ChangeKind::Added) => ChangeSeverity::Info,
        ("applies_when", _) => ChangeSeverity::Minor,

        // Context fields: valid_from, valid_until
        ("valid_from" | "valid_until", _) => ChangeSeverity::Info,

        // Context fields: tags, keywords
        ("tags" | "keywords", _) => ChangeSeverity::Info,

        // Context fields: aliases
        ("aliases", ChangeKind::Added) => ChangeSeverity::Info,
        ("aliases", _) => ChangeSeverity::Minor,

        // Extension / unknown fields
        _ => ChangeSeverity::Info,
    }
}

/// Compares all fields of two nodes and returns a list of changes.
///
/// Handles typed fields (node_type, summary, priority, etc.) and
/// generic extra_fields. Delegates relationship fields to relation::diff_relation_field.
#[must_use]
pub(crate) fn diff_all_fields(left: &Node, right: &Node) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    // node_type
    if left.node_type != right.node_type {
        changes.push(FieldChange {
            field: "type".to_owned(),
            kind: ChangeKind::Modified,
            severity: classify_severity("node_type", ChangeKind::Modified),
            old_value: Some(FieldValueSnapshot::from_str_val(
                &left.node_type.to_string(),
            )),
            new_value: Some(FieldValueSnapshot::from_str_val(
                &right.node_type.to_string(),
            )),
        });
    }

    // summary
    if left.summary != right.summary {
        changes.push(FieldChange {
            field: "summary".to_owned(),
            kind: ChangeKind::Modified,
            severity: classify_severity("summary", ChangeKind::Modified),
            old_value: Some(FieldValueSnapshot::from_str_val(&left.summary)),
            new_value: Some(FieldValueSnapshot::from_str_val(&right.summary)),
        });
    }

    // Optional scalar: priority
    diff_opt_display(
        &mut changes,
        "priority",
        left.priority.as_ref().map(|v| v.to_string()),
        right.priority.as_ref().map(|v| v.to_string()),
    );

    // stability
    diff_opt_display(
        &mut changes,
        "stability",
        left.stability.as_ref().map(|v| v.to_string()),
        right.stability.as_ref().map(|v| v.to_string()),
    );

    // confidence
    diff_opt_display(
        &mut changes,
        "confidence",
        left.confidence.as_ref().map(|v| v.to_string()),
        right.confidence.as_ref().map(|v| v.to_string()),
    );

    // status
    diff_opt_display(
        &mut changes,
        "status",
        left.status.as_ref().map(|v| v.to_string()),
        right.status.as_ref().map(|v| v.to_string()),
    );

    // Relationship fields (ordered)
    diff_opt_list_field(
        &mut changes,
        "depends",
        left.depends.as_deref(),
        right.depends.as_deref(),
    );

    // Relationship fields (unordered)
    diff_opt_list_field(
        &mut changes,
        "related_to",
        left.related_to.as_deref(),
        right.related_to.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "replaces",
        left.replaces.as_deref(),
        right.replaces.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "conflicts",
        left.conflicts.as_deref(),
        right.conflicts.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "see_also",
        left.see_also.as_deref(),
        right.see_also.as_deref(),
    );

    // Operational list fields
    diff_opt_list_field(
        &mut changes,
        "items",
        left.items.as_deref(),
        right.items.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "steps",
        left.steps.as_deref(),
        right.steps.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "fields",
        left.fields.as_deref(),
        right.fields.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "input",
        left.input.as_deref(),
        right.input.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "output",
        left.output.as_deref(),
        right.output.as_deref(),
    );

    // Explanatory scalar fields
    diff_opt_scalar(
        &mut changes,
        "detail",
        left.detail.as_deref(),
        right.detail.as_deref(),
    );
    diff_opt_scalar(
        &mut changes,
        "examples",
        left.examples.as_deref(),
        right.examples.as_deref(),
    );
    diff_opt_scalar(
        &mut changes,
        "notes",
        left.notes.as_deref(),
        right.notes.as_deref(),
    );

    // Explanatory list fields
    diff_opt_list_field(
        &mut changes,
        "rationale",
        left.rationale.as_deref(),
        right.rationale.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "tradeoffs",
        left.tradeoffs.as_deref(),
        right.tradeoffs.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "resolution",
        left.resolution.as_deref(),
        right.resolution.as_deref(),
    );

    // Executable scalar
    diff_opt_scalar(
        &mut changes,
        "target",
        left.target.as_deref(),
        right.target.as_deref(),
    );

    // Executable complex: code
    diff_opt_complex(
        &mut changes,
        "code",
        left.code.as_ref(),
        right.code.as_ref(),
    );

    // Executable complex: code_blocks
    diff_opt_complex(
        &mut changes,
        "code_blocks",
        left.code_blocks.as_ref(),
        right.code_blocks.as_ref(),
    );

    // Executable complex: verify
    diff_opt_complex(
        &mut changes,
        "verify",
        left.verify.as_ref(),
        right.verify.as_ref(),
    );

    // Executable complex: agent_context
    diff_opt_complex(
        &mut changes,
        "agent_context",
        left.agent_context.as_ref(),
        right.agent_context.as_ref(),
    );

    // Execution state
    diff_opt_display(
        &mut changes,
        "execution_status",
        left.execution_status.as_ref().map(|v| v.to_string()),
        right.execution_status.as_ref().map(|v| v.to_string()),
    );
    diff_opt_scalar(
        &mut changes,
        "executed_by",
        left.executed_by.as_deref(),
        right.executed_by.as_deref(),
    );
    diff_opt_scalar(
        &mut changes,
        "executed_at",
        left.executed_at.as_deref(),
        right.executed_at.as_deref(),
    );
    diff_opt_scalar(
        &mut changes,
        "execution_log",
        left.execution_log.as_deref(),
        right.execution_log.as_deref(),
    );
    diff_opt_display(
        &mut changes,
        "retry_count",
        left.retry_count.map(|v| v.to_string()),
        right.retry_count.map(|v| v.to_string()),
    );

    // Orchestration complex
    diff_opt_complex(
        &mut changes,
        "parallel_groups",
        left.parallel_groups.as_ref(),
        right.parallel_groups.as_ref(),
    );

    // Memory complex
    diff_opt_complex(
        &mut changes,
        "memory",
        left.memory.as_ref(),
        right.memory.as_ref(),
    );

    // Context list fields
    diff_opt_list_field(
        &mut changes,
        "scope",
        left.scope.as_deref(),
        right.scope.as_deref(),
    );
    diff_opt_scalar(
        &mut changes,
        "applies_when",
        left.applies_when.as_deref(),
        right.applies_when.as_deref(),
    );
    diff_opt_scalar(
        &mut changes,
        "valid_from",
        left.valid_from.as_deref(),
        right.valid_from.as_deref(),
    );
    diff_opt_scalar(
        &mut changes,
        "valid_until",
        left.valid_until.as_deref(),
        right.valid_until.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "tags",
        left.tags.as_deref(),
        right.tags.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "aliases",
        left.aliases.as_deref(),
        right.aliases.as_deref(),
    );
    diff_opt_list_field(
        &mut changes,
        "keywords",
        left.keywords.as_deref(),
        right.keywords.as_deref(),
    );

    // Extension (extra_fields)
    diff_extra_fields(&mut changes, &left.extra_fields, &right.extra_fields);

    changes
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn diff_opt_scalar(
    changes: &mut Vec<FieldChange>,
    field: &str,
    left: Option<&str>,
    right: Option<&str>,
) {
    match (left, right) {
        (None, None) => {}
        (None, Some(r)) => changes.push(FieldChange {
            field: field.to_owned(),
            kind: ChangeKind::Added,
            severity: classify_severity(field, ChangeKind::Added),
            old_value: None,
            new_value: Some(FieldValueSnapshot::Scalar(r.to_owned())),
        }),
        (Some(l), None) => changes.push(FieldChange {
            field: field.to_owned(),
            kind: ChangeKind::Removed,
            severity: classify_severity(field, ChangeKind::Removed),
            old_value: Some(FieldValueSnapshot::Scalar(l.to_owned())),
            new_value: None,
        }),
        (Some(l), Some(r)) if l != r => changes.push(FieldChange {
            field: field.to_owned(),
            kind: ChangeKind::Modified,
            severity: classify_severity(field, ChangeKind::Modified),
            old_value: Some(FieldValueSnapshot::Scalar(l.to_owned())),
            new_value: Some(FieldValueSnapshot::Scalar(r.to_owned())),
        }),
        _ => {}
    }
}

fn diff_opt_display(
    changes: &mut Vec<FieldChange>,
    field: &str,
    left: Option<String>,
    right: Option<String>,
) {
    diff_opt_scalar(changes, field, left.as_deref(), right.as_deref());
}

fn diff_opt_list_field(
    changes: &mut Vec<FieldChange>,
    field: &str,
    left: Option<&[String]>,
    right: Option<&[String]>,
) {
    let left_slice = left.unwrap_or_default();
    let right_slice = right.unwrap_or_default();

    // If both are absent, no change
    if left.is_none() && right.is_none() {
        return;
    }

    let sub_changes = super::relation::diff_relation_field(field, left_slice, right_slice);
    changes.extend(sub_changes);
}

fn diff_opt_complex<T: Serialize + PartialEq>(
    changes: &mut Vec<FieldChange>,
    field: &str,
    left: Option<&T>,
    right: Option<&T>,
) {
    match (left, right) {
        (None, None) => {}
        (None, Some(r)) => changes.push(FieldChange {
            field: field.to_owned(),
            kind: ChangeKind::Added,
            severity: classify_severity(field, ChangeKind::Added),
            old_value: None,
            new_value: Some(FieldValueSnapshot::from_complex(r)),
        }),
        (Some(l), None) => changes.push(FieldChange {
            field: field.to_owned(),
            kind: ChangeKind::Removed,
            severity: classify_severity(field, ChangeKind::Removed),
            old_value: Some(FieldValueSnapshot::from_complex(l)),
            new_value: None,
        }),
        (Some(l), Some(r)) if l != r => {
            let old_json = serde_json::to_string(l).unwrap_or_default();
            let new_json = serde_json::to_string(r).unwrap_or_default();
            if old_json != new_json {
                changes.push(FieldChange {
                    field: field.to_owned(),
                    kind: ChangeKind::Modified,
                    severity: classify_severity(field, ChangeKind::Modified),
                    old_value: Some(FieldValueSnapshot::Complex(old_json)),
                    new_value: Some(FieldValueSnapshot::Complex(new_json)),
                });
            }
        }
        _ => {}
    }
}

fn diff_extra_fields(
    changes: &mut Vec<FieldChange>,
    left: &std::collections::BTreeMap<String, FieldValue>,
    right: &std::collections::BTreeMap<String, FieldValue>,
) {
    // Keys only in left -> Removed
    for (key, val) in left {
        if !right.contains_key(key) {
            changes.push(FieldChange {
                field: key.clone(),
                kind: ChangeKind::Removed,
                severity: ChangeSeverity::Info,
                old_value: Some(FieldValueSnapshot::from_field_value(val)),
                new_value: None,
            });
        }
    }

    // Keys only in right -> Added
    for (key, val) in right {
        if !left.contains_key(key) {
            changes.push(FieldChange {
                field: key.clone(),
                kind: ChangeKind::Added,
                severity: ChangeSeverity::Info,
                old_value: None,
                new_value: Some(FieldValueSnapshot::from_field_value(val)),
            });
        }
    }

    // Keys in both but different values -> Modified
    for (key, left_val) in left {
        if let Some(right_val) = right.get(key) {
            if left_val != right_val {
                changes.push(FieldChange {
                    field: key.clone(),
                    kind: ChangeKind::Modified,
                    severity: ChangeSeverity::Info,
                    old_value: Some(FieldValueSnapshot::from_field_value(left_val)),
                    new_value: Some(FieldValueSnapshot::from_field_value(right_val)),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::model::fields::{FieldValue, NodeType, Priority, Span};
    use crate::model::node::Node;

    use super::*;

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

    #[test]
    fn test_classify_severity_type_modified_returns_breaking() {
        assert_eq!(
            classify_severity("node_type", ChangeKind::Modified),
            ChangeSeverity::Breaking
        );
    }

    #[test]
    fn test_classify_severity_summary_modified_returns_minor() {
        assert_eq!(
            classify_severity("summary", ChangeKind::Modified),
            ChangeSeverity::Minor
        );
    }

    #[test]
    fn test_classify_severity_detail_modified_returns_info() {
        assert_eq!(
            classify_severity("detail", ChangeKind::Modified),
            ChangeSeverity::Info
        );
    }

    #[test]
    fn test_classify_severity_depends_removed_returns_breaking() {
        assert_eq!(
            classify_severity("depends", ChangeKind::Removed),
            ChangeSeverity::Breaking
        );
    }

    #[test]
    fn test_classify_severity_steps_removed_returns_breaking() {
        assert_eq!(
            classify_severity("steps", ChangeKind::Removed),
            ChangeSeverity::Breaking
        );
    }

    #[test]
    fn test_classify_severity_code_removed_returns_breaking() {
        assert_eq!(
            classify_severity("code", ChangeKind::Removed),
            ChangeSeverity::Breaking
        );
    }

    #[test]
    fn test_classify_severity_tags_added_returns_info() {
        assert_eq!(
            classify_severity("tags", ChangeKind::Added),
            ChangeSeverity::Info
        );
    }

    #[test]
    fn test_classify_severity_unknown_field_returns_info() {
        assert_eq!(
            classify_severity("some_unknown_field", ChangeKind::Modified),
            ChangeSeverity::Info
        );
    }

    #[test]
    fn test_diff_all_fields_identical_returns_empty() {
        let node = minimal_node();
        let changes = diff_all_fields(&node, &node);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_all_fields_type_changed_returns_breaking() {
        let left = minimal_node();
        let mut right = left.clone();
        right.node_type = NodeType::Rules;
        let changes = diff_all_fields(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "type");
        assert_eq!(changes[0].severity, ChangeSeverity::Breaking);
    }

    #[test]
    fn test_diff_all_fields_summary_changed_returns_minor() {
        let left = minimal_node();
        let mut right = left.clone();
        right.summary = "different summary".to_owned();
        let changes = diff_all_fields(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "summary");
        assert_eq!(changes[0].severity, ChangeSeverity::Minor);
    }

    #[test]
    fn test_diff_all_fields_priority_added_returns_info() {
        let left = minimal_node();
        let mut right = left.clone();
        right.priority = Some(Priority::High);
        let changes = diff_all_fields(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "priority");
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_all_fields_priority_removed_returns_minor() {
        let mut left = minimal_node();
        left.priority = Some(Priority::High);
        let right = minimal_node();
        let changes = diff_all_fields(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "priority");
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].severity, ChangeSeverity::Minor);
    }

    #[test]
    fn test_diff_all_fields_extra_field_added_returns_info() {
        let left = minimal_node();
        let mut right = left.clone();
        right.extra_fields.insert(
            "custom_key".to_owned(),
            FieldValue::Scalar("val".to_owned()),
        );
        let changes = diff_all_fields(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "custom_key");
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_all_fields_extra_field_removed_returns_info() {
        let mut left = minimal_node();
        left.extra_fields.insert(
            "custom_key".to_owned(),
            FieldValue::Scalar("val".to_owned()),
        );
        let right = minimal_node();
        let changes = diff_all_fields(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "custom_key");
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_all_fields_code_block_changed_returns_minor() {
        use crate::model::code::{CodeAction, CodeBlock};
        let left = minimal_node();
        let mut right = left.clone();
        right.code = Some(CodeBlock {
            lang: Some("rust".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "fn main() {}".to_owned(),
            anchor: None,
            old: None,
        });
        let changes = diff_all_fields(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "code");
        assert_eq!(changes[0].kind, ChangeKind::Added);
        // added code -> Minor
        assert_eq!(changes[0].severity, ChangeSeverity::Minor);
    }

    #[test]
    fn test_field_value_snapshot_from_field_value_scalar() {
        let fv = FieldValue::Scalar("hello".to_owned());
        let snap = FieldValueSnapshot::from_field_value(&fv);
        assert_eq!(snap, FieldValueSnapshot::Scalar("hello".to_owned()));
    }

    #[test]
    fn test_field_value_snapshot_from_field_value_list() {
        let fv = FieldValue::List(vec!["a".to_owned(), "b".to_owned()]);
        let snap = FieldValueSnapshot::from_field_value(&fv);
        assert_eq!(
            snap,
            FieldValueSnapshot::List(vec!["a".to_owned(), "b".to_owned()])
        );
    }

    #[test]
    fn test_field_value_snapshot_from_complex_serializes_json() {
        #[derive(Serialize)]
        struct Simple {
            key: &'static str,
        }
        let val = Simple { key: "value" };
        let snap = FieldValueSnapshot::from_complex(&val);
        assert!(matches!(snap, FieldValueSnapshot::Complex(_)));
        if let FieldValueSnapshot::Complex(s) = snap {
            assert!(s.contains("key"));
            assert!(s.contains("value"));
        }
    }
}
