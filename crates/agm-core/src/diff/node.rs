//! Node-level diff: match nodes by ID between two AGM files.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::file::AgmFile;

use super::ChangeSeverity;
use super::fields::FieldChange;

/// Diff of a single node that exists in both left and right files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeDiff {
    pub node_id: String,
    pub field_changes: Vec<FieldChange>,
    pub has_breaking_change: bool,
}

/// Result of matching nodes between two files by ID.
pub(crate) struct NodeMatchResult {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<NodeDiff>,
    pub unchanged_count: usize,
}

/// Matches nodes by ID between two files and diffs matched pairs.
#[must_use]
pub(crate) fn diff_nodes(left: &AgmFile, right: &AgmFile) -> NodeMatchResult {
    let left_map: BTreeMap<&str, &crate::model::node::Node> =
        left.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let right_map: BTreeMap<&str, &crate::model::node::Node> =
        right.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut unchanged_count = 0usize;

    // Nodes only in left -> removed
    for id in left_map.keys() {
        if !right_map.contains_key(id) {
            removed.push((*id).to_owned());
        }
    }

    // Nodes only in right -> added
    for id in right_map.keys() {
        if !left_map.contains_key(id) {
            added.push((*id).to_owned());
        }
    }

    // Nodes in both -> diff
    for (id, left_node) in &left_map {
        if let Some(right_node) = right_map.get(id) {
            let field_changes = super::fields::diff_all_fields(left_node, right_node);
            if field_changes.is_empty() {
                unchanged_count += 1;
            } else {
                let has_breaking_change = field_changes
                    .iter()
                    .any(|fc| fc.severity == ChangeSeverity::Breaking);
                modified.push(NodeDiff {
                    node_id: (*id).to_owned(),
                    field_changes,
                    has_breaking_change,
                });
            }
        }
    }

    NodeMatchResult {
        added,
        removed,
        modified,
        unchanged_count,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::model::fields::{NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;

    use super::*;

    fn header() -> Header {
        Header {
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
        }
    }

    fn minimal_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: "a node".to_owned(),
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
            span: Span::new(1, 5),
        }
    }

    fn file_with_nodes(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: header(),
            nodes,
        }
    }

    #[test]
    fn test_diff_nodes_identical_returns_empty() {
        let node = minimal_node("test.a");
        let file = file_with_nodes(vec![node]);
        let result = diff_nodes(&file, &file);
        assert!(result.added.is_empty());
        assert!(result.removed.is_empty());
        assert!(result.modified.is_empty());
        assert_eq!(result.unchanged_count, 1);
    }

    #[test]
    fn test_diff_nodes_added_node_detected() {
        let left = file_with_nodes(vec![minimal_node("test.a")]);
        let right = file_with_nodes(vec![minimal_node("test.a"), minimal_node("test.b")]);
        let result = diff_nodes(&left, &right);
        assert_eq!(result.added, vec!["test.b".to_owned()]);
        assert!(result.removed.is_empty());
    }

    #[test]
    fn test_diff_nodes_removed_node_detected() {
        let left = file_with_nodes(vec![minimal_node("test.a"), minimal_node("test.b")]);
        let right = file_with_nodes(vec![minimal_node("test.a")]);
        let result = diff_nodes(&left, &right);
        assert!(result.added.is_empty());
        assert_eq!(result.removed, vec!["test.b".to_owned()]);
    }

    #[test]
    fn test_diff_nodes_modified_node_detected() {
        let left_node = minimal_node("test.a");
        let mut right_node = minimal_node("test.a");
        right_node.summary = "changed summary".to_owned();

        let left = file_with_nodes(vec![left_node]);
        let right = file_with_nodes(vec![right_node]);
        let result = diff_nodes(&left, &right);
        assert!(result.added.is_empty());
        assert!(result.removed.is_empty());
        assert_eq!(result.modified.len(), 1);
        assert_eq!(result.modified[0].node_id, "test.a");
    }

    #[test]
    fn test_diff_nodes_multiple_changes_all_tracked() {
        let left = file_with_nodes(vec![
            minimal_node("test.a"),
            minimal_node("test.b"),
            minimal_node("test.c"),
        ]);
        let mut right_a = minimal_node("test.a");
        right_a.summary = "modified".to_owned();
        let right = file_with_nodes(vec![
            right_a,
            // test.b removed
            minimal_node("test.c"),
            minimal_node("test.d"), // added
        ]);
        let result = diff_nodes(&left, &right);
        assert_eq!(result.added, vec!["test.d".to_owned()]);
        assert_eq!(result.removed, vec!["test.b".to_owned()]);
        assert_eq!(result.modified.len(), 1);
        assert_eq!(result.unchanged_count, 1);
    }

    #[test]
    fn test_diff_nodes_unchanged_counted() {
        let nodes = vec![
            minimal_node("test.a"),
            minimal_node("test.b"),
            minimal_node("test.c"),
        ];
        let left = file_with_nodes(nodes.clone());
        let right = file_with_nodes(nodes);
        let result = diff_nodes(&left, &right);
        assert_eq!(result.unchanged_count, 3);
    }

    #[test]
    fn test_diff_nodes_order_independent() {
        // Same nodes in different order = no diff
        let left = file_with_nodes(vec![minimal_node("test.a"), minimal_node("test.b")]);
        let right = file_with_nodes(vec![minimal_node("test.b"), minimal_node("test.a")]);
        let result = diff_nodes(&left, &right);
        assert!(result.added.is_empty());
        assert!(result.removed.is_empty());
        assert!(result.modified.is_empty());
        assert_eq!(result.unchanged_count, 2);
    }
}
