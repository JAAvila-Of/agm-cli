//! Node field filtering by [`LoadMode`].
//!
//! [`filter_node`] produces a new [`Node`] that contains only the fields
//! appropriate for the requested loading mode. Exhaustive field construction
//! is used (no `..` rest syntax) so that adding a new field to [`Node`]
//! forces a compile error here, ensuring no field is accidentally omitted.

use std::collections::BTreeMap;

use crate::model::node::Node;

use super::mode::{FieldCategory, LoadMode};

// ---------------------------------------------------------------------------
// filter_node
// ---------------------------------------------------------------------------

/// Returns a new [`Node`] with only the fields appropriate for `mode`.
///
/// Fields not included in `mode` are set to `None` (or empty for
/// `extra_fields`). The `span` metadata field is always preserved.
#[must_use]
pub fn filter_node(node: &Node, mode: LoadMode) -> Node {
    let op = FieldCategory::Operational.included_in(mode);
    let ex = FieldCategory::Executable.included_in(mode);
    let full = FieldCategory::Full.included_in(mode);

    // Summary fields are always included (every mode is a superset of Summary).
    Node {
        // --- Always (Summary) ---
        id: node.id.clone(),
        node_type: node.node_type.clone(),
        summary: node.summary.clone(),
        priority: node.priority.clone(),
        stability: node.stability.clone(),
        depends: node.depends.clone(),
        tags: node.tags.clone(),

        // --- Operational ---
        items: if op { node.items.clone() } else { None },
        steps: if op { node.steps.clone() } else { None },
        fields: if op { node.fields.clone() } else { None },
        input: if op { node.input.clone() } else { None },
        output: if op { node.output.clone() } else { None },

        // --- Executable ---
        code: if ex { node.code.clone() } else { None },
        code_blocks: if ex { node.code_blocks.clone() } else { None },
        verify: if ex { node.verify.clone() } else { None },
        agent_context: if ex { node.agent_context.clone() } else { None },
        target: if ex { node.target.clone() } else { None },
        execution_status: if ex {
            node.execution_status.clone()
        } else {
            None
        },
        executed_by: if ex { node.executed_by.clone() } else { None },
        executed_at: if ex { node.executed_at.clone() } else { None },
        execution_log: if ex { node.execution_log.clone() } else { None },
        retry_count: if ex { node.retry_count } else { None },
        memory: if ex { node.memory.clone() } else { None },

        // --- Full only ---
        confidence: if full { node.confidence.clone() } else { None },
        status: if full { node.status.clone() } else { None },
        related_to: if full { node.related_to.clone() } else { None },
        replaces: if full { node.replaces.clone() } else { None },
        conflicts: if full { node.conflicts.clone() } else { None },
        see_also: if full { node.see_also.clone() } else { None },
        detail: if full { node.detail.clone() } else { None },
        rationale: if full { node.rationale.clone() } else { None },
        tradeoffs: if full { node.tradeoffs.clone() } else { None },
        resolution: if full { node.resolution.clone() } else { None },
        examples: if full { node.examples.clone() } else { None },
        notes: if full { node.notes.clone() } else { None },
        parallel_groups: if full {
            node.parallel_groups.clone()
        } else {
            None
        },
        scope: if full { node.scope.clone() } else { None },
        applies_when: if full {
            node.applies_when.clone()
        } else {
            None
        },
        valid_from: if full { node.valid_from.clone() } else { None },
        valid_until: if full { node.valid_until.clone() } else { None },
        aliases: if full { node.aliases.clone() } else { None },
        keywords: if full { node.keywords.clone() } else { None },
        extra_fields: if full {
            node.extra_fields.clone()
        } else {
            BTreeMap::new()
        },

        // --- Ticket fields (v1.2.0) — included in Full mode ---
        title: if full { node.title.clone() } else { None },
        description: if full { node.description.clone() } else { None },
        action: if full { node.action.clone() } else { None },
        sdd_phase: if full { node.sdd_phase.clone() } else { None },
        prompt: if full { node.prompt.clone() } else { None },
        assignee: if full { node.assignee.clone() } else { None },
        labels: if full { node.labels.clone() } else { None },
        ticket_id: if full { node.ticket_id.clone() } else { None },

        // --- Always preserved (metadata) ---
        span: node.span.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::model::code::CodeBlock;
    use crate::model::execution::ExecutionStatus;
    use crate::model::fields::{
        Confidence, FieldValue, NodeStatus, NodeType, Priority, Span, Stability,
    };
    use crate::model::node::Node;

    use super::*;

    fn full_node() -> Node {
        use std::collections::BTreeMap;
        Node {
            id: "test.node".to_owned(),
            node_type: NodeType::Workflow,
            summary: "a test workflow".to_owned(),
            priority: Some(Priority::Critical),
            stability: Some(Stability::High),
            confidence: Some(Confidence::High),
            status: Some(NodeStatus::Active),
            depends: Some(vec!["dep.a".to_owned()]),
            related_to: Some(vec!["rel.b".to_owned()]),
            replaces: Some(vec!["old.node".to_owned()]),
            conflicts: Some(vec!["conf.x".to_owned()]),
            see_also: Some(vec!["ref.y".to_owned()]),
            items: Some(vec!["item1".to_owned()]),
            steps: Some(vec!["step1".to_owned()]),
            fields: Some(vec!["field1".to_owned()]),
            input: Some(vec!["in1".to_owned()]),
            output: Some(vec!["out1".to_owned()]),
            detail: Some("detail text".to_owned()),
            rationale: Some(vec!["reason".to_owned()]),
            tradeoffs: Some(vec!["tradeoff".to_owned()]),
            resolution: Some(vec!["resolution".to_owned()]),
            examples: Some("example text".to_owned()),
            notes: Some("note text".to_owned()),
            code: Some(CodeBlock {
                lang: Some("bash".to_owned()),
                target: None,
                action: crate::model::code::CodeAction::Full,
                body: "echo hello".to_owned(),
                anchor: None,
                old: None,
            }),
            target: Some("agent-01".to_owned()),
            execution_status: Some(ExecutionStatus::Completed),
            executed_by: Some("agent-01".to_owned()),
            executed_at: Some("2026-04-06T00:00:00Z".to_owned()),
            execution_log: Some("log entry".to_owned()),
            retry_count: Some(1),
            scope: Some(vec!["scope1".to_owned()]),
            applies_when: Some("condition".to_owned()),
            valid_from: Some("2026-01-01".to_owned()),
            valid_until: Some("2026-12-31".to_owned()),
            tags: Some(vec!["tag1".to_owned()]),
            aliases: Some(vec!["alias1".to_owned()]),
            keywords: Some(vec!["kw1".to_owned()]),
            extra_fields: {
                let mut m = BTreeMap::new();
                m.insert("custom".to_owned(), FieldValue::Scalar("val".to_owned()));
                m
            },
            span: Span::new(5, 20),
            ..Default::default()
        }
    }

    #[test]
    fn test_filter_node_summary_mode_keeps_only_summary_fields() {
        let node = full_node();
        let filtered = filter_node(&node, LoadMode::Summary);

        // Summary fields preserved
        assert_eq!(filtered.id, "test.node");
        assert_eq!(filtered.node_type, NodeType::Workflow);
        assert_eq!(filtered.summary, "a test workflow");
        assert!(filtered.priority.is_some());
        assert!(filtered.stability.is_some());
        assert!(filtered.depends.is_some());
        assert!(filtered.tags.is_some());

        // Operational fields cleared
        assert!(filtered.items.is_none());
        assert!(filtered.steps.is_none());
        assert!(filtered.fields.is_none());
        assert!(filtered.input.is_none());
        assert!(filtered.output.is_none());

        // Executable fields cleared
        assert!(filtered.code.is_none());
        assert!(filtered.execution_status.is_none());
        assert!(filtered.retry_count.is_none());

        // Full fields cleared
        assert!(filtered.confidence.is_none());
        assert!(filtered.related_to.is_none());
        assert!(filtered.detail.is_none());
        assert!(filtered.extra_fields.is_empty());
    }

    #[test]
    fn test_filter_node_operational_mode_includes_operational_fields() {
        let node = full_node();
        let filtered = filter_node(&node, LoadMode::Operational);

        // Summary fields
        assert!(filtered.priority.is_some());
        assert!(filtered.depends.is_some());

        // Operational fields present
        assert!(filtered.items.is_some());
        assert!(filtered.steps.is_some());
        assert!(filtered.fields.is_some());
        assert!(filtered.input.is_some());
        assert!(filtered.output.is_some());

        // Executable still excluded
        assert!(filtered.code.is_none());
        assert!(filtered.execution_status.is_none());

        // Full still excluded
        assert!(filtered.confidence.is_none());
        assert!(filtered.extra_fields.is_empty());
    }

    #[test]
    fn test_filter_node_executable_mode_includes_executable_fields() {
        let node = full_node();
        let filtered = filter_node(&node, LoadMode::Executable);

        // Summary
        assert!(filtered.priority.is_some());
        // Operational
        assert!(filtered.steps.is_some());
        // Executable present
        assert!(filtered.code.is_some());
        assert!(filtered.execution_status.is_some());
        assert!(filtered.executed_by.is_some());
        assert!(filtered.retry_count.is_some());

        // Full still excluded
        assert!(filtered.confidence.is_none());
        assert!(filtered.detail.is_none());
        assert!(filtered.extra_fields.is_empty());
    }

    #[test]
    fn test_filter_node_full_mode_includes_all_fields() {
        let node = full_node();
        let filtered = filter_node(&node, LoadMode::Full);

        assert!(filtered.confidence.is_some());
        assert!(filtered.status.is_some());
        assert!(filtered.related_to.is_some());
        assert!(filtered.detail.is_some());
        assert!(filtered.rationale.is_some());
        assert!(filtered.aliases.is_some());
        assert!(!filtered.extra_fields.is_empty());
    }

    #[test]
    fn test_filter_node_span_always_preserved() {
        let node = full_node();
        for mode in [
            LoadMode::Summary,
            LoadMode::Operational,
            LoadMode::Executable,
            LoadMode::Full,
        ] {
            let filtered = filter_node(&node, mode);
            assert_eq!(filtered.span.start_line, 5);
            assert_eq!(filtered.span.end_line, 20);
        }
    }

    #[test]
    fn test_filter_node_extra_fields_cleared_below_full() {
        let node = full_node();
        for mode in [
            LoadMode::Summary,
            LoadMode::Operational,
            LoadMode::Executable,
        ] {
            let filtered = filter_node(&node, mode);
            assert!(
                filtered.extra_fields.is_empty(),
                "extra_fields should be empty in {mode} mode"
            );
        }
        let full = filter_node(&node, LoadMode::Full);
        assert!(!full.extra_fields.is_empty());
    }

    #[test]
    fn test_filter_node_empty_node_all_modes_do_not_panic() {
        let node = Node {
            id: "empty".to_owned(),
            node_type: NodeType::Facts,
            summary: "empty".to_owned(),
            span: Span::new(1, 1),
            ..Default::default()
        };
        for mode in [
            LoadMode::Summary,
            LoadMode::Operational,
            LoadMode::Executable,
            LoadMode::Full,
        ] {
            let _ = filter_node(&node, mode);
        }
    }
}
