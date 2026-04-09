//! AGM node type (spec S11, S12).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::code::CodeBlock;
use super::context::AgentContext;
use super::execution::ExecutionStatus;
use super::fields::{Confidence, FieldValue, NodeStatus, NodeType, Priority, Span, Stability};
use super::memory::MemoryEntry;
use super::orchestration::ParallelGroup;
use super::verify::VerifyCheck;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    #[serde(rename = "node")]
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub summary: String,

    // Control fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NodeStatus>,

    // Relationship fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_to: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub see_also: Option<Vec<String>>,

    // Operational fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Vec<String>>,

    // Explanatory fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tradeoffs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    // Executable fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_blocks: Option<Vec<CodeBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<Vec<VerifyCheck>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_context: Option<AgentContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    // Execution state fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_status: Option<ExecutionStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_log: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<u32>,

    // Orchestration fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_groups: Option<Vec<ParallelGroup>>,

    // Memory fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<Vec<MemoryEntry>>,

    // Context fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applies_when: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,

    // Extension fields
    #[serde(flatten)]
    pub extra_fields: BTreeMap<String, FieldValue>,

    // Source span (not serialized)
    #[serde(skip)]
    pub span: Span,
}

#[cfg(test)]
mod tests {
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
    fn test_node_minimal_serde_roundtrip() {
        let node = minimal_node();
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node.id, back.id);
        assert_eq!(node.node_type, back.node_type);
        assert_eq!(node.summary, back.summary);
    }

    #[test]
    fn test_node_json_uses_spec_field_names() {
        let node = minimal_node();
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"node\""));
        assert!(json.contains("\"type\""));
        assert!(!json.contains("\"id\""));
        assert!(!json.contains("\"node_type\""));
    }

    #[test]
    fn test_node_optional_fields_absent_in_json() {
        let node = minimal_node();
        let json = serde_json::to_string(&node).unwrap();
        assert!(!json.contains("priority"));
        assert!(!json.contains("depends"));
        assert!(!json.contains("steps"));
        assert!(!json.contains("code"));
        assert!(!json.contains("execution_status"));
        assert!(!json.contains("memory"));
    }

    #[test]
    fn test_node_with_relationships_serde() {
        let mut node = minimal_node();
        node.depends = Some(vec!["auth.constraints".to_owned()]);
        node.related_to = Some(vec!["auth.session".to_owned()]);
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node.depends, back.depends);
        assert_eq!(node.related_to, back.related_to);
    }

    #[test]
    fn test_node_with_execution_state_serde() {
        let mut node = minimal_node();
        node.execution_status = Some(ExecutionStatus::Completed);
        node.executed_by = Some("agent-01".to_owned());
        node.executed_at = Some("2026-04-03T14:32:00Z".to_owned());
        node.retry_count = Some(0);
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node.execution_status, back.execution_status);
        assert_eq!(node.executed_by, back.executed_by);
    }

    #[test]
    fn test_node_extra_fields_preserved() {
        let mut node = minimal_node();
        node.extra_fields.insert(
            "custom_field".to_owned(),
            FieldValue::Scalar("value".to_owned()),
        );
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("custom_field"));
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.extra_fields.get("custom_field"),
            Some(&FieldValue::Scalar("value".to_owned()))
        );
    }

    #[test]
    fn test_node_span_not_serialized() {
        let mut node = minimal_node();
        node.span = Span::new(5, 20);
        let json = serde_json::to_string(&node).unwrap();
        assert!(!json.contains("start_line"));
        assert!(!json.contains("end_line"));
        assert!(!json.contains("span"));
    }

    #[test]
    fn test_node_deserialize_from_spec_appendix_b() {
        let json = r#"{
            "node": "auth.login",
            "type": "workflow",
            "stability": "medium",
            "priority": "critical",
            "depends": ["auth.constraints", "auth.session"],
            "input": ["host", "return_url"],
            "output": ["redirect_url", "sid_cookie"],
            "summary": "resolve tenant -> redirect -> callback -> create sid",
            "steps": ["resolve tenant", "redirect to provider"],
            "execution_status": "pending",
            "retry_count": 0
        }"#;
        let node: Node = serde_json::from_str(json).unwrap();
        assert_eq!(node.id, "auth.login");
        assert_eq!(node.node_type, NodeType::Workflow);
        assert_eq!(node.priority, Some(Priority::Critical));
        assert_eq!(node.stability, Some(Stability::Medium));
        assert_eq!(node.depends.as_ref().unwrap().len(), 2);
        assert_eq!(node.steps.as_ref().unwrap().len(), 2);
        assert_eq!(node.execution_status, Some(ExecutionStatus::Pending));
        assert_eq!(node.retry_count, Some(0));
    }
}
