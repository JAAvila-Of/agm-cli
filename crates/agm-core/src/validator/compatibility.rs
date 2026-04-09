//! Conflict detection between active nodes (spec S11).
//!
//! Pass 5 (cross-node): detects co-loaded conflicting active nodes.

use std::collections::HashSet;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation, Severity};
use crate::model::fields::NodeStatus;
use crate::model::file::AgmFile;
use crate::model::node::Node;

/// Returns true if the node is considered "active" (not deprecated, superseded, or draft).
fn is_active(node: &Node) -> bool {
    matches!(&node.status, None | Some(NodeStatus::Active))
}

/// Validates that no two active nodes declare each other in `conflicts`.
///
/// Rule: V013 (warning — conflicting active nodes co-loaded).
/// Each conflicting pair is reported at most once (deduplication by sorted pair).
#[must_use]
pub fn validate_compatibility(file: &AgmFile, file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();

    // Build a quick lookup from node id to node reference
    let node_map: std::collections::HashMap<&str, &Node> =
        file.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Track reported pairs to avoid duplicates: sorted (a, b) string pair
    let mut reported: HashSet<(String, String)> = HashSet::new();

    for node in &file.nodes {
        if !is_active(node) {
            continue;
        }
        let conflicts = match &node.conflicts {
            Some(c) => c,
            None => continue,
        };

        for conflict_id in conflicts {
            // Skip self-conflicts and skip if target doesn't exist
            if conflict_id == &node.id {
                continue;
            }
            let other = match node_map.get(conflict_id.as_str()) {
                Some(n) => n,
                None => continue, // V004 handles unresolved refs
            };

            if !is_active(other) {
                continue;
            }

            // Deduplicate: use sorted pair
            let pair = if node.id < *conflict_id {
                (node.id.clone(), conflict_id.clone())
            } else {
                (conflict_id.clone(), node.id.clone())
            };

            if reported.insert(pair) {
                errors.push(AgmError::with_severity(
                    ErrorCode::V013,
                    Severity::Warning,
                    format!(
                        "Conflicting active nodes co-loaded: `{}` and `{conflict_id}`",
                        node.id
                    ),
                    ErrorLocation::full(file_name, node.span.start_line, &node.id),
                ));
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::model::fields::{NodeStatus, NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;

    fn minimal_header() -> Header {
        Header {
            agm: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
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

    fn make_node(id: &str) -> Node {
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
            span: Span::new(5, 7),
        }
    }

    #[test]
    fn test_validate_compatibility_no_conflicts_returns_empty() {
        let node_a = make_node("auth.a");
        let node_b = make_node("auth.b");
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b],
        };
        let errors = validate_compatibility(&file, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_compatibility_conflicting_active_returns_v013() {
        let mut node_a = make_node("auth.a");
        let node_b = make_node("auth.b");
        node_a.conflicts = Some(vec!["auth.b".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b],
        };
        let errors = validate_compatibility(&file, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V013));
    }

    #[test]
    fn test_validate_compatibility_conflicting_one_deprecated_returns_empty() {
        let mut node_a = make_node("auth.a");
        let mut node_b = make_node("auth.b");
        node_a.conflicts = Some(vec!["auth.b".to_owned()]);
        node_b.status = Some(NodeStatus::Deprecated); // not active
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b],
        };
        let errors = validate_compatibility(&file, "test.agm");
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V013));
    }

    #[test]
    fn test_validate_compatibility_bidirectional_reports_once() {
        let mut node_a = make_node("auth.a");
        let mut node_b = make_node("auth.b");
        // Both declare conflicts with each other
        node_a.conflicts = Some(vec!["auth.b".to_owned()]);
        node_b.conflicts = Some(vec!["auth.a".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b],
        };
        let errors = validate_compatibility(&file, "test.agm");
        let v013_count = errors.iter().filter(|e| e.code == ErrorCode::V013).count();
        assert_eq!(
            v013_count, 1,
            "Bidirectional conflict should be reported only once"
        );
    }

    #[test]
    fn test_validate_compatibility_self_conflict_skipped() {
        let mut node = make_node("auth.a");
        node.conflicts = Some(vec!["auth.a".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let errors = validate_compatibility(&file, "test.agm");
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V013));
    }

    #[test]
    fn test_validate_compatibility_conflict_with_nonexistent_skipped() {
        let mut node = make_node("auth.a");
        node.conflicts = Some(vec!["nonexistent.node".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let errors = validate_compatibility(&file, "test.agm");
        // V004 handles unresolved refs; V013 should not fire here
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V013));
    }
}
