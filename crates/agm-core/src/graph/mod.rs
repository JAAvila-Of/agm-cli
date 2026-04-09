//! Graph engine: dependency graph operations on AGM nodes.
//!
//! Builds a `petgraph`-backed directed graph from AGM node relationships
//! and provides topological sort, cycle detection, and transitive dependency
//! queries.

use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

pub mod build;
pub mod cycles;
pub mod query;
pub mod topo;

pub use build::{build_graph, build_graph_with_imports};
pub use cycles::{CycleError, detect_cycles};
pub use query::{find_conflicts, transitive_dependents, transitive_deps};
pub use topo::topological_sort;

// ---------------------------------------------------------------------------
// RelationKind
// ---------------------------------------------------------------------------

/// The kind of relationship between two AGM nodes.
///
/// Maps 1:1 to the five relationship fields in the AGM spec (S15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationKind {
    /// Strong ordering dependency (`depends` field).
    Depends,
    /// Informational relationship (`related_to` field).
    RelatedTo,
    /// This node supersedes the target (`replaces` field).
    Replaces,
    /// Mutual exclusion (`conflicts` field).
    Conflicts,
    /// Cross-reference for context (`see_also` field).
    SeeAlso,
}

// ---------------------------------------------------------------------------
// AgmGraph
// ---------------------------------------------------------------------------

/// Directed graph of AGM nodes and their relationships.
///
/// Node weights are node ID strings. Edge weights are `RelationKind`.
/// Provides an internal index for O(1) ID-to-`NodeIndex` lookup.
#[derive(Debug, Clone)]
pub struct AgmGraph {
    /// The underlying petgraph directed graph.
    pub(crate) inner: DiGraph<String, RelationKind>,
    /// Maps node ID strings to their petgraph `NodeIndex` for O(1) lookup.
    pub(crate) index: HashMap<String, NodeIndex>,
}

impl AgmGraph {
    /// Returns the number of nodes in the graph.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Returns the number of edges in the graph.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// Returns `true` if a node with the given ID exists in the graph.
    #[must_use]
    pub fn contains_node(&self, node_id: &str) -> bool {
        self.index.contains_key(node_id)
    }

    /// Returns the `NodeIndex` for the given node ID, if it exists.
    #[must_use]
    pub fn node_index(&self, node_id: &str) -> Option<NodeIndex> {
        self.index.get(node_id).copied()
    }

    /// Returns all node IDs in the graph (unordered).
    #[must_use]
    pub fn node_ids(&self) -> Vec<&str> {
        self.index.keys().map(String::as_str).collect()
    }

    /// Returns the target node IDs of edges of a specific kind originating
    /// from the given node.
    #[must_use]
    pub fn edges_of_kind(&self, node_id: &str, kind: RelationKind) -> Vec<&str> {
        let Some(&idx) = self.index.get(node_id) else {
            return vec![];
        };
        self.inner
            .edges(idx)
            .filter(|e| *e.weight() == kind)
            .map(|e| self.inner[e.target()].as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// depends_subgraph
// ---------------------------------------------------------------------------

/// Creates a subgraph containing only `Depends` edges.
///
/// All nodes are preserved. Only edges with weight `RelationKind::Depends`
/// are copied. Returns the filtered graph and a mapping from new `NodeIndex`
/// to original node ID string.
pub(crate) fn depends_subgraph(graph: &AgmGraph) -> DiGraph<String, ()> {
    let mut sub: DiGraph<String, ()> = DiGraph::new();
    // Map original NodeIndex -> new NodeIndex in sub
    let mut old_to_new: HashMap<NodeIndex, NodeIndex> = HashMap::new();

    for node_idx in graph.inner.node_indices() {
        let id = graph.inner[node_idx].clone();
        let new_idx = sub.add_node(id);
        old_to_new.insert(node_idx, new_idx);
    }

    for edge in graph.inner.edge_references() {
        if *edge.weight() == RelationKind::Depends {
            let src = old_to_new[&edge.source()];
            let tgt = old_to_new[&edge.target()];
            sub.add_edge(src, tgt, ());
        }
    }

    sub
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod test_helpers {
    use std::collections::BTreeMap;

    use crate::model::fields::{NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;

    pub fn minimal_header() -> Header {
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

    pub fn make_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: format!("test node {id}"),
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

    pub fn make_file(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: minimal_header(),
            nodes,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use test_helpers::*;

    #[test]
    fn test_relation_kind_debug_and_clone() {
        let k = RelationKind::Depends;
        let k2 = k;
        assert_eq!(k, k2);
        assert_eq!(format!("{k:?}"), "Depends");

        let json = serde_json::to_string(&RelationKind::Conflicts).unwrap();
        let back: RelationKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, RelationKind::Conflicts);
    }

    #[test]
    fn test_agm_graph_empty_returns_zero_counts() {
        let graph = build_graph(&make_file(vec![]));
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }
}
