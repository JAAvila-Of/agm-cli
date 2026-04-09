//! Topological sorting over `Depends` edges only.

use petgraph::algo::toposort;

use super::cycles::{CycleError, detect_cycles};
use super::{AgmGraph, depends_subgraph};

// ---------------------------------------------------------------------------
// topological_sort
// ---------------------------------------------------------------------------

/// Returns a topological ordering of node IDs considering only `Depends` edges.
///
/// The ordering is valid for scheduling: a node appears after all its
/// dependencies. If the depends-subgraph contains a cycle, returns
/// `Err(CycleError)` with the cycle path.
///
/// Non-`Depends` edges (`RelatedTo`, `Replaces`, `Conflicts`, `SeeAlso`) are
/// ignored for ordering purposes.
pub fn topological_sort(graph: &AgmGraph) -> Result<Vec<String>, CycleError> {
    let sub = depends_subgraph(graph);

    match toposort(&sub, None) {
        Ok(indices) => {
            // petgraph's toposort returns nodes such that for an edge u -> v,
            // u appears before v. In AGM, `depends` means "I need you first",
            // so the dependency (v) must appear before the dependent (u).
            // Reversing gives the correct scheduling order: dependencies first.
            let ids = indices
                .into_iter()
                .rev()
                .map(|idx| sub[idx].clone())
                .collect();
            Ok(ids)
        }
        Err(_cycle) => {
            // toposort only gives a single node in the cycle; use detect_cycles
            // to get the actual cycle members for a useful error message.
            let cycles = detect_cycles(graph);
            let path = cycles.into_iter().next().unwrap_or_default();
            Err(CycleError::new(path))
        }
    }
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
    fn test_topological_sort_empty_graph_returns_empty() {
        let graph = build_graph(&make_file(vec![]));
        let result = topological_sort(&graph).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_topological_sort_dag_returns_valid_ordering() {
        // Chain: a depends on b, b depends on c.
        // Valid topo order (dependencies first): c, b, a.
        let mut a = make_node("a");
        let mut b = make_node("b");
        let c = make_node("c");
        a.depends = Some(vec!["b".to_owned()]);
        b.depends = Some(vec!["c".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b, c]));
        let result = topological_sort(&graph).unwrap();
        assert_eq!(result.len(), 3);
        let pos_a = result.iter().position(|x| x == "a").unwrap();
        let pos_b = result.iter().position(|x| x == "b").unwrap();
        let pos_c = result.iter().position(|x| x == "c").unwrap();
        // a must come after b, b must come after c.
        assert!(pos_b < pos_a, "b should appear before a");
        assert!(pos_c < pos_b, "c should appear before b");
    }

    #[test]
    fn test_topological_sort_cycle_returns_cycle_error() {
        let mut a = make_node("a");
        let mut b = make_node("b");
        a.depends = Some(vec!["b".to_owned()]);
        b.depends = Some(vec!["a".to_owned()]);
        let graph = build_graph(&make_file(vec![a, b]));
        let err = topological_sort(&graph).unwrap_err();
        let path = err.display_path();
        assert!(
            path.contains('a') && path.contains('b'),
            "cycle path should mention both a and b, got: {path}"
        );
    }
}
