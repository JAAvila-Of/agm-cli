//! Depends-chain cycle detection (spec S11).
//!
//! Pass 5 (cross-node): detects hard cycles in the `depends` graph using DFS
//! with three-color marking.

use std::collections::HashMap;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::file::AgmFile;

/// Three-color DFS state for cycle detection.
#[derive(PartialEq)]
enum Color {
    /// Currently on the DFS stack (in-progress).
    Gray,
    /// Fully explored (no cycle through this node).
    Black,
}

/// Builds the depends adjacency map: `node_id -> [dependency_ids]`.
///
/// Only includes edges to nodes that exist in the file (unknown refs are
/// handled by references.rs, not here).
fn build_depends_graph<'a>(
    file: &'a AgmFile,
    known_ids: &std::collections::HashSet<&str>,
) -> HashMap<&'a str, Vec<&'a str>> {
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in &file.nodes {
        let deps: Vec<&str> = node
            .depends
            .as_ref()
            .map(|d| {
                d.iter()
                    .filter(|id| known_ids.contains(id.as_str()))
                    .map(|s| s.as_str())
                    .collect()
            })
            .unwrap_or_default();
        graph.insert(node.id.as_str(), deps);
    }
    graph
}

/// DFS visitor that detects cycles.
///
/// Returns all cycles found as `AgmError` instances.
fn dfs_detect_cycle<'a>(
    node_id: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    colors: &mut HashMap<&'a str, Color>,
    stack: &mut Vec<&'a str>,
    node_lines: &HashMap<&'a str, usize>,
    file_name: &str,
    errors: &mut Vec<AgmError>,
) {
    colors.insert(node_id, Color::Gray);
    stack.push(node_id);

    if let Some(deps) = graph.get(node_id) {
        for &dep in deps {
            match colors.get(dep) {
                Some(Color::Gray) => {
                    // Found a back-edge — cycle detected
                    // Build path from where `dep` first appears in the stack
                    let cycle_start = stack.iter().position(|&n| n == dep).unwrap_or(0);
                    let mut cycle_nodes: Vec<&str> = stack[cycle_start..].to_vec();
                    cycle_nodes.push(dep); // close the cycle
                    let cycle_path = cycle_nodes.join(" -> ");

                    let line = node_lines.get(dep).copied().unwrap_or(0);
                    errors.push(AgmError::new(
                        ErrorCode::V005,
                        format!("Cycle detected in `depends`: `{cycle_path}`"),
                        ErrorLocation::file_line(file_name, line),
                    ));
                }
                Some(Color::Black) => {
                    // Already fully explored — no cycle through here
                }
                None => {
                    dfs_detect_cycle(dep, graph, colors, stack, node_lines, file_name, errors);
                }
            }
        }
    }

    stack.pop();
    colors.insert(node_id, Color::Black);
}

/// Validates that the `depends` graph contains no cycles.
///
/// Rule: V005 (cycle detected in `depends`).
#[must_use]
pub fn validate_cycles(file: &AgmFile, file_name: &str) -> Vec<AgmError> {
    if file.nodes.is_empty() {
        return Vec::new();
    }

    let known_ids: std::collections::HashSet<&str> =
        file.nodes.iter().map(|n| n.id.as_str()).collect();
    let node_lines: HashMap<&str, usize> = file
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.span.start_line))
        .collect();

    let graph = build_depends_graph(file, &known_ids);

    let mut colors: HashMap<&str, Color> = HashMap::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut errors: Vec<AgmError> = Vec::new();

    for node in &file.nodes {
        let id = node.id.as_str();
        if !matches!(colors.get(id), Some(Color::Black)) {
            dfs_detect_cycle(
                id,
                &graph,
                &mut colors,
                &mut stack,
                &node_lines,
                file_name,
                &mut errors,
            );
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::model::fields::{NodeType, Span};
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

    fn make_node(id: &str, line: usize) -> Node {
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
            span: Span::new(line, line + 2),
        }
    }

    #[test]
    fn test_validate_cycles_no_cycle_returns_empty() {
        let mut node_a = make_node("auth.a", 1);
        let node_b = make_node("auth.b", 5);
        node_a.depends = Some(vec!["auth.b".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b],
        };
        let errors = validate_cycles(&file, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_cycles_self_depends_returns_v005() {
        let mut node = make_node("auth.a", 1);
        node.depends = Some(vec!["auth.a".to_owned()]); // self-cycle
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let errors = validate_cycles(&file, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V005));
    }

    #[test]
    fn test_validate_cycles_two_node_returns_v005() {
        let mut node_a = make_node("auth.a", 1);
        let mut node_b = make_node("auth.b", 5);
        node_a.depends = Some(vec!["auth.b".to_owned()]);
        node_b.depends = Some(vec!["auth.a".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b],
        };
        let errors = validate_cycles(&file, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V005));
    }

    #[test]
    fn test_validate_cycles_three_node_returns_v005() {
        let mut node_a = make_node("a", 1);
        let mut node_b = make_node("b", 5);
        let mut node_c = make_node("c", 9);
        node_a.depends = Some(vec!["b".to_owned()]);
        node_b.depends = Some(vec!["c".to_owned()]);
        node_c.depends = Some(vec!["a".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b, node_c],
        };
        let errors = validate_cycles(&file, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V005));
    }

    #[test]
    fn test_validate_cycles_diamond_no_cycle_returns_empty() {
        // A -> B, A -> C, B -> D, C -> D (diamond — no cycle)
        let mut node_a = make_node("a", 1);
        let mut node_b = make_node("b", 5);
        let mut node_c = make_node("c", 9);
        let node_d = make_node("d", 13);
        node_a.depends = Some(vec!["b".to_owned(), "c".to_owned()]);
        node_b.depends = Some(vec!["d".to_owned()]);
        node_c.depends = Some(vec!["d".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b, node_c, node_d],
        };
        let errors = validate_cycles(&file, "test.agm");
        assert!(errors.is_empty(), "Diamond DAG should not report a cycle");
    }

    #[test]
    fn test_validate_cycles_multiple_disconnected_cycles_returns_all() {
        // Cycle 1: x -> y -> x
        // Cycle 2: p -> q -> p
        let mut x = make_node("x", 1);
        let mut y = make_node("y", 5);
        let mut p = make_node("p", 9);
        let mut q = make_node("q", 13);
        x.depends = Some(vec!["y".to_owned()]);
        y.depends = Some(vec!["x".to_owned()]);
        p.depends = Some(vec!["q".to_owned()]);
        q.depends = Some(vec!["p".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![x, y, p, q],
        };
        let errors = validate_cycles(&file, "test.agm");
        // Should detect at least 2 cycles
        let v005_count = errors.iter().filter(|e| e.code == ErrorCode::V005).count();
        assert!(
            v005_count >= 2,
            "Expected at least 2 cycle errors, got {v005_count}"
        );
    }

    #[test]
    fn test_validate_cycles_empty_file_returns_empty() {
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![],
        };
        let errors = validate_cycles(&file, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_cycles_unknown_dep_not_flagged() {
        // deps referencing non-existent nodes are ignored in cycle detection
        let mut node = make_node("auth.a", 1);
        node.depends = Some(vec!["nonexistent.node".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let errors = validate_cycles(&file, "test.agm");
        assert!(errors.is_empty());
    }
}
