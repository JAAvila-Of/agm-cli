//! Transitive dependency queries and conflict detection.

use std::collections::{BTreeSet, HashSet};

use petgraph::Direction;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use super::{AgmGraph, RelationKind};

// ---------------------------------------------------------------------------
// transitive_deps
// ---------------------------------------------------------------------------

/// Returns all transitive dependencies of the given node, following only
/// `Depends` edges.
///
/// Performs a DFS from `node_id` along outgoing `Depends` edges.
/// The result does NOT include `node_id` itself.
///
/// Returns an empty set if the node has no dependencies or does not exist.
#[must_use]
pub fn transitive_deps(graph: &AgmGraph, node_id: &str) -> HashSet<String> {
    let Some(&start) = graph.index.get(node_id) else {
        return HashSet::new();
    };

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut stack: Vec<NodeIndex> = vec![start];
    visited.insert(start);

    while let Some(current) = stack.pop() {
        for edge in graph.inner.edges(current) {
            if *edge.weight() == RelationKind::Depends {
                let target = edge.target();
                if visited.insert(target) {
                    stack.push(target);
                }
            }
        }
    }

    // Exclude the start node itself.
    visited.remove(&start);
    visited
        .into_iter()
        .map(|idx| graph.inner[idx].clone())
        .collect()
}

// ---------------------------------------------------------------------------
// transitive_dependents
// ---------------------------------------------------------------------------

/// Returns all transitive dependents (reverse dependencies) of the given
/// node, following `Depends` edges in reverse.
///
/// "What nodes transitively depend on `node_id`?"
///
/// Performs a DFS from `node_id` along incoming `Depends` edges.
/// The result does NOT include `node_id` itself.
///
/// Returns an empty set if nothing depends on this node or it does not exist.
#[must_use]
pub fn transitive_dependents(graph: &AgmGraph, node_id: &str) -> HashSet<String> {
    let Some(&start) = graph.index.get(node_id) else {
        return HashSet::new();
    };

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut stack: Vec<NodeIndex> = vec![start];
    visited.insert(start);

    while let Some(current) = stack.pop() {
        for edge in graph.inner.edges_directed(current, Direction::Incoming) {
            if *edge.weight() == RelationKind::Depends {
                let source = edge.source();
                if visited.insert(source) {
                    stack.push(source);
                }
            }
        }
    }

    // Exclude the start node itself.
    visited.remove(&start);
    visited
        .into_iter()
        .map(|idx| graph.inner[idx].clone())
        .collect()
}

// ---------------------------------------------------------------------------
// find_conflicts
// ---------------------------------------------------------------------------

/// Finds all pairs of nodes connected by `Conflicts` edges.
///
/// Returns a `Vec<(String, String)>` where each tuple is an ordered pair
/// `(a, b)` with `a < b` lexicographically (to avoid duplicates, since
/// conflicts are semantically bidirectional even though the edge is directed).
///
/// Returns each pair once regardless of edge direction.
#[must_use]
pub fn find_conflicts(graph: &AgmGraph) -> Vec<(String, String)> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();

    for edge in graph.inner.edge_references() {
        if *edge.weight() == RelationKind::Conflicts {
            let src = graph.inner[edge.source()].clone();
            let tgt = graph.inner[edge.target()].clone();
            let pair = if src <= tgt { (src, tgt) } else { (tgt, src) };
            seen.insert(pair);
        }
    }

    seen.into_iter().collect()
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
    fn test_transitive_deps_linear_chain() {
        // a -> b -> c
        let mut a = make_node("a");
        let mut b = make_node("b");
        let c = make_node("c");
        a.depends = Some(vec!["b".to_owned()]);
        b.depends = Some(vec!["c".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b, c]));
        let deps = transitive_deps(&graph, "a");
        assert_eq!(deps, HashSet::from(["b".to_owned(), "c".to_owned()]));
    }

    #[test]
    fn test_transitive_deps_nonexistent_node_returns_empty() {
        let graph = build_graph(&make_file(vec![make_node("a")]));
        let deps = transitive_deps(&graph, "nonexistent");
        assert!(deps.is_empty());
    }

    #[test]
    fn test_transitive_dependents_returns_reverse() {
        // a -> b -> c; transitive_dependents(c) = {a, b}
        let mut a = make_node("a");
        let mut b = make_node("b");
        let c = make_node("c");
        a.depends = Some(vec!["b".to_owned()]);
        b.depends = Some(vec!["c".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b, c]));
        let dependents = transitive_dependents(&graph, "c");
        assert_eq!(dependents, HashSet::from(["a".to_owned(), "b".to_owned()]));
    }

    #[test]
    fn test_find_conflicts_returns_ordered_pairs() {
        let mut a = make_node("a");
        let mut c = make_node("c");
        let b = make_node("b");
        let d = make_node("d");
        // a conflicts b, c conflicts d
        a.conflicts = Some(vec!["b".to_owned()]);
        c.conflicts = Some(vec!["d".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b, c, d]));
        let conflicts = find_conflicts(&graph);
        assert_eq!(conflicts.len(), 2);
        // Each pair should be lexicographically ordered.
        for (l, r) in &conflicts {
            assert!(l <= r, "pair ({l}, {r}) is not ordered");
        }
        assert!(conflicts.contains(&("a".to_owned(), "b".to_owned())));
        assert!(conflicts.contains(&("c".to_owned(), "d".to_owned())));
    }
}
