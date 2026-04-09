//! Cycle detection using Tarjan's SCC algorithm.

use petgraph::algo::tarjan_scc;
use petgraph::visit::EdgeRef;
use thiserror::Error;

use super::{AgmGraph, depends_subgraph};

// ---------------------------------------------------------------------------
// CycleError
// ---------------------------------------------------------------------------

/// Error returned when a cycle is detected in the `depends` graph.
#[derive(Debug, Clone, PartialEq, Error)]
#[error("cycle detected in depends graph: {}", .cycle.join(" -> "))]
pub struct CycleError {
    /// The node IDs forming the cycle, in order.
    /// The first and last elements are the same node (closing the loop).
    /// Example: `["a", "b", "c", "a"]`
    pub cycle: Vec<String>,
}

impl CycleError {
    /// Creates a new `CycleError` from a cycle path.
    ///
    /// `path` should be the ordered sequence of nodes in the cycle.
    /// The closing node (same as first) will be appended automatically
    /// if not already present.
    #[must_use]
    pub fn new(mut path: Vec<String>) -> Self {
        if path.len() >= 2 && path.first() != path.last() {
            let first = path[0].clone();
            path.push(first);
        }
        Self { cycle: path }
    }

    /// Returns the cycle path as a human-readable string: `"a -> b -> c -> a"`.
    #[must_use]
    pub fn display_path(&self) -> String {
        self.cycle.join(" -> ")
    }
}

// ---------------------------------------------------------------------------
// detect_cycles
// ---------------------------------------------------------------------------

/// Detects all cycles in the `depends` subgraph.
///
/// Uses Tarjan's SCC algorithm (`petgraph::algo::tarjan_scc`) to find
/// strongly connected components of size > 1, which represent cycles.
/// Also checks size-1 SCCs for self-loops.
///
/// Returns a `Vec<Vec<String>>` where each inner `Vec` is the set of node
/// IDs participating in a cycle. Returns an empty `Vec` if the graph is
/// acyclic.
///
/// Only considers `Depends` edges.
#[must_use]
pub fn detect_cycles(graph: &AgmGraph) -> Vec<Vec<String>> {
    let sub = depends_subgraph(graph);
    let sccs = tarjan_scc(&sub);

    let mut cycles: Vec<Vec<String>> = Vec::new();

    for scc in sccs {
        if scc.len() > 1 {
            // Multiple nodes in one SCC = cycle.
            let ids: Vec<String> = scc.iter().map(|&idx| sub[idx].clone()).collect();
            cycles.push(ids);
        } else if scc.len() == 1 {
            // Check for self-loop.
            let node_idx = scc[0];
            let has_self_loop = sub.edges(node_idx).any(|e| e.target() == node_idx);
            if has_self_loop {
                cycles.push(vec![sub[node_idx].clone()]);
            }
        }
    }

    cycles
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build::build_graph;
    use crate::graph::test_helpers::*;

    #[test]
    fn test_detect_cycles_acyclic_returns_empty() {
        let mut a = make_node("a");
        let b = make_node("b");
        let c = make_node("c");
        a.depends = Some(vec!["b".to_owned(), "c".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b, c]));
        assert!(detect_cycles(&graph).is_empty());
    }

    #[test]
    fn test_detect_cycles_self_loop_detected() {
        let mut a = make_node("a");
        a.depends = Some(vec!["a".to_owned()]);
        let graph = build_graph(&make_file(vec![a]));
        let cycles = detect_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["a"]);
    }

    #[test]
    fn test_detect_cycles_two_node_cycle_detected() {
        let mut a = make_node("a");
        let mut b = make_node("b");
        a.depends = Some(vec!["b".to_owned()]);
        b.depends = Some(vec!["a".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b]));
        let cycles = detect_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        let mut ids = cycles[0].clone();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn test_detect_cycles_multiple_sccs_detected() {
        // Two disconnected cycles: (a->b->a) and (c->d->c)
        let mut a = make_node("a");
        let mut b = make_node("b");
        let mut c = make_node("c");
        let mut d = make_node("d");
        a.depends = Some(vec!["b".to_owned()]);
        b.depends = Some(vec!["a".to_owned()]);
        c.depends = Some(vec!["d".to_owned()]);
        d.depends = Some(vec!["c".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b, c, d]));
        let cycles = detect_cycles(&graph);
        assert_eq!(cycles.len(), 2);
    }
}
