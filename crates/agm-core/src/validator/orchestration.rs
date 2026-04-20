//! Orchestration node validation (spec S13.10, S27).
//!
//! Pass 3 (structural): validates parallel_groups on orchestration-type nodes.

use std::collections::{HashMap, HashSet};

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::fields::NodeType;
use crate::model::node::Node;
use crate::model::orchestration::ParallelGroup;

/// DFS cycle detection on the group `requires` graph.
///
/// Returns errors for any cycle found in the group dependency graph.
fn detect_requires_cycles(groups: &[ParallelGroup], node: &Node, file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();

    let group_names: HashSet<&str> = groups.iter().map(|g| g.group.as_str()).collect();
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();

    for group in groups {
        let reqs: Vec<&str> = group
            .requires
            .as_ref()
            .map(|r| {
                r.iter()
                    .filter(|req| group_names.contains(req.as_str()))
                    .map(|s| s.as_str())
                    .collect()
            })
            .unwrap_or_default();
        graph.insert(group.group.as_str(), reqs);
    }

    let mut colors: HashMap<&str, u8> = HashMap::new(); // 0=white, 1=gray, 2=black
    let mut stack: Vec<&str> = Vec::new();

    for group in groups {
        let gid = group.group.as_str();
        if colors.get(gid).copied().unwrap_or(0) != 2 {
            dfs_requires(
                gid,
                &graph,
                &mut colors,
                &mut stack,
                node,
                file_name,
                &mut errors,
            );
        }
    }

    errors
}

fn dfs_requires<'a>(
    group_id: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    colors: &mut HashMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
    node: &Node,
    file_name: &str,
    errors: &mut Vec<AgmError>,
) {
    colors.insert(group_id, 1); // gray
    stack.push(group_id);

    if let Some(reqs) = graph.get(group_id) {
        for &req in reqs {
            match colors.get(req).copied().unwrap_or(0) {
                1 => {
                    // back-edge — cycle
                    let start = stack.iter().position(|&g| g == req).unwrap_or(0);
                    let mut cycle: Vec<&str> = stack[start..].to_vec();
                    cycle.push(req);
                    let cycle_path = cycle.join(" -> ");
                    errors.push(AgmError::new(
                        ErrorCode::V019,
                        format!(
                            "Cycle in orchestration `requires`: `{cycle_path}` in node `{}`",
                            node.id
                        ),
                        ErrorLocation::full(file_name, node.span.start_line, &node.id),
                    ));
                }
                2 => {} // black, already explored
                _ => {
                    dfs_requires(req, graph, colors, stack, node, file_name, errors);
                }
            }
        }
    }

    stack.pop();
    colors.insert(group_id, 2); // black
}

/// Validates orchestration-related fields on a node.
///
/// Rules:
/// - V018: `orchestration` type node without `parallel_groups`
/// - V018: group missing required sub-fields (empty group name, empty nodes list)
/// - V018: group node reference to non-existent node (skipped in `single_node` scope)
/// - V018: `requires` references non-existent group
/// - V019: cycle in group `requires` graph
///
/// When `single_node` is `true`, the check that each group node reference exists
/// in `all_ids` is skipped — referenced nodes may live in sibling files.
#[must_use]
pub fn validate_orchestration(
    node: &Node,
    all_ids: &HashSet<String>,
    file_name: &str,
    single_node: bool,
) -> Vec<AgmError> {
    let mut errors = Vec::new();
    let line = node.span.start_line;
    let id = node.id.as_str();

    // V018 — orchestration-type node must have parallel_groups
    if node.node_type == NodeType::Orchestration && node.parallel_groups.is_none() {
        errors.push(AgmError::new(
            ErrorCode::V018,
            format!("Orchestration node `{id}` missing required field: `parallel_groups`"),
            ErrorLocation::full(file_name, line, id),
        ));
        return errors; // can't validate groups if none exist
    }

    let groups = match &node.parallel_groups {
        Some(g) => g,
        None => return errors,
    };

    let group_names: HashSet<&str> = groups.iter().map(|g| g.group.as_str()).collect();

    for group in groups {
        // V018 — group name must not be empty
        if group.group.trim().is_empty() {
            errors.push(AgmError::new(
                ErrorCode::V018,
                format!("Orchestration group in node `{id}` has empty `group` name"),
                ErrorLocation::full(file_name, line, id),
            ));
        }

        // V018 — group must have at least one node
        if group.nodes.is_empty() {
            errors.push(AgmError::new(
                ErrorCode::V018,
                format!(
                    "Orchestration group `{}` in node `{id}` has empty `nodes` list",
                    group.group
                ),
                ErrorLocation::full(file_name, line, id),
            ));
        }

        // V018 — each node referenced in the group must exist.
        // Skipped in SingleNode scope: referenced nodes may live in sibling files.
        if !single_node {
            for ref_node_id in &group.nodes {
                if !all_ids.contains(ref_node_id.as_str()) {
                    errors.push(AgmError::new(
                        ErrorCode::V018,
                        format!(
                            "Orchestration group `{}` references non-existent node `{ref_node_id}`",
                            group.group
                        ),
                        ErrorLocation::full(file_name, line, id),
                    ));
                }
            }
        }

        // V018 — requires must reference existing group names
        if let Some(ref reqs) = group.requires {
            for req in reqs {
                if !group_names.contains(req.as_str()) {
                    errors.push(AgmError::new(
                        ErrorCode::V018,
                        format!(
                            "Orchestration group `{}` in node `{id}` requires non-existent group `{req}`",
                            group.group
                        ),
                        ErrorLocation::full(file_name, line, id),
                    ));
                }
            }
        }
    }

    // V019 — detect cycles in the requires graph
    errors.extend(detect_requires_cycles(groups, node, file_name));

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::model::fields::{NodeType, Span};
    use crate::model::node::Node;
    use crate::model::orchestration::{ParallelGroup, Strategy};

    fn make_node(id: &str, node_type: NodeType) -> Node {
        Node {
            id: id.to_owned(),
            node_type,
            summary: "a node".to_owned(),
            span: Span::new(5, 7),
            ..Default::default()
        }
    }

    fn ids(node_ids: &[&str]) -> HashSet<String> {
        node_ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_validate_orchestration_valid_returns_empty() {
        let mut node = make_node("orch.main", NodeType::Orchestration);
        node.parallel_groups = Some(vec![ParallelGroup {
            group: "phase1".to_owned(),
            nodes: vec!["step.a".to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        }]);
        let all_ids = ids(&["step.a"]);
        let errors = validate_orchestration(&node, &all_ids, "test.agm", false);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_orchestration_no_parallel_groups_returns_v018() {
        let node = make_node("orch.main", NodeType::Orchestration);
        let all_ids = HashSet::new();
        let errors = validate_orchestration(&node, &all_ids, "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V018));
    }

    #[test]
    fn test_validate_orchestration_non_orchestration_no_groups_returns_empty() {
        let node = make_node("facts.node", NodeType::Facts);
        let all_ids = HashSet::new();
        let errors = validate_orchestration(&node, &all_ids, "test.agm", false);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_orchestration_missing_node_ref_returns_v018() {
        let mut node = make_node("orch.main", NodeType::Orchestration);
        node.parallel_groups = Some(vec![ParallelGroup {
            group: "phase1".to_owned(),
            nodes: vec!["missing.step".to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        }]);
        let all_ids = HashSet::new(); // missing.step not in all_ids
        let errors = validate_orchestration(&node, &all_ids, "test.agm", false);
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V018 && e.message.contains("missing.step"))
        );
    }

    #[test]
    fn test_validate_orchestration_empty_group_nodes_returns_v018() {
        let mut node = make_node("orch.main", NodeType::Orchestration);
        node.parallel_groups = Some(vec![ParallelGroup {
            group: "phase1".to_owned(),
            nodes: vec![], // empty
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        }]);
        let all_ids = HashSet::new();
        let errors = validate_orchestration(&node, &all_ids, "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V018));
    }

    #[test]
    fn test_validate_orchestration_requires_cycle_returns_v019() {
        let mut node = make_node("orch.main", NodeType::Orchestration);
        node.parallel_groups = Some(vec![
            ParallelGroup {
                group: "phase1".to_owned(),
                nodes: vec!["step.a".to_owned()],
                strategy: Strategy::Sequential,
                requires: Some(vec!["phase2".to_owned()]),
                max_concurrency: None,
            },
            ParallelGroup {
                group: "phase2".to_owned(),
                nodes: vec!["step.b".to_owned()],
                strategy: Strategy::Sequential,
                requires: Some(vec!["phase1".to_owned()]),
                max_concurrency: None,
            },
        ]);
        let all_ids = ids(&["step.a", "step.b"]);
        let errors = validate_orchestration(&node, &all_ids, "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V019));
    }

    #[test]
    fn test_validate_orchestration_requires_nonexistent_group_returns_v018() {
        let mut node = make_node("orch.main", NodeType::Orchestration);
        node.parallel_groups = Some(vec![ParallelGroup {
            group: "phase1".to_owned(),
            nodes: vec!["step.a".to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["nonexistent.group".to_owned()]),
            max_concurrency: None,
        }]);
        let all_ids = ids(&["step.a"]);
        let errors = validate_orchestration(&node, &all_ids, "test.agm", false);
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V018 && e.message.contains("nonexistent.group"))
        );
    }
}
