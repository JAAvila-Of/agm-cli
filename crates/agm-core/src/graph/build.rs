//! Graph construction from `AgmFile` and resolved imports.

use std::collections::HashMap;

use petgraph::graph::DiGraph;

use crate::import::ResolvedPackage;
use crate::model::file::AgmFile;

use super::{AgmGraph, RelationKind};

// ---------------------------------------------------------------------------
// build_graph
// ---------------------------------------------------------------------------

/// Builds a directed graph from a single AGM file.
///
/// Creates one graph node per `Node` in the file. For each relationship
/// field (`depends`, `related_to`, `replaces`, `conflicts`, `see_also`),
/// adds a directed edge from the source node to the target node only if
/// the target node exists in the graph. Unknown targets are silently
/// skipped (the validator handles unknown reference errors separately).
#[must_use]
pub fn build_graph(file: &AgmFile) -> AgmGraph {
    let mut inner: DiGraph<String, RelationKind> = DiGraph::new();
    let mut index = HashMap::new();

    // First pass: insert all nodes.
    for node in &file.nodes {
        let idx = inner.add_node(node.id.clone());
        index.insert(node.id.clone(), idx);
    }

    // Second pass: insert edges.
    add_edges_from_nodes(&file.nodes, &mut inner, &index);

    AgmGraph { inner, index }
}

// ---------------------------------------------------------------------------
// build_graph_with_imports
// ---------------------------------------------------------------------------

/// Builds a directed graph from an AGM file plus its resolved imports.
///
/// Imported nodes are added with fully qualified IDs (`{package}.{local_id}`).
/// Edges from the main file that reference imported node IDs (using the
/// fully qualified form) are resolved against the import graph.
#[must_use]
pub fn build_graph_with_imports(file: &AgmFile, imports: &[ResolvedPackage]) -> AgmGraph {
    let mut inner: DiGraph<String, RelationKind> = DiGraph::new();
    let mut index = HashMap::new();

    // Step 1: Insert all local nodes with plain IDs.
    for node in &file.nodes {
        if !index.contains_key(&node.id) {
            let idx = inner.add_node(node.id.clone());
            index.insert(node.id.clone(), idx);
        }
    }

    // Step 2: Insert all imported nodes with qualified IDs.
    for pkg in imports {
        for node in &pkg.file.nodes {
            let qualified_id = format!("{}.{}", pkg.package, node.id);
            index
                .entry(qualified_id)
                .or_insert_with_key(|key| inner.add_node(key.clone()));
        }
    }

    // Step 3: Add edges for all local nodes (targets looked up in full index).
    add_edges_from_nodes(&file.nodes, &mut inner, &index);

    // Step 4: Add edges for all imported nodes (targets qualified within same
    // package first, then looked up in the full index).
    for pkg in imports {
        for node in &pkg.file.nodes {
            let qualified_source = format!("{}.{}", pkg.package, node.id);
            let Some(&src_idx) = index.get(&qualified_source) else {
                continue;
            };

            add_edges_for_targets(
                src_idx,
                node.depends.as_deref().unwrap_or(&[]),
                RelationKind::Depends,
                &pkg.package,
                &mut inner,
                &index,
            );
            add_edges_for_targets(
                src_idx,
                node.related_to.as_deref().unwrap_or(&[]),
                RelationKind::RelatedTo,
                &pkg.package,
                &mut inner,
                &index,
            );
            add_edges_for_targets(
                src_idx,
                node.replaces.as_deref().unwrap_or(&[]),
                RelationKind::Replaces,
                &pkg.package,
                &mut inner,
                &index,
            );
            add_edges_for_targets(
                src_idx,
                node.conflicts.as_deref().unwrap_or(&[]),
                RelationKind::Conflicts,
                &pkg.package,
                &mut inner,
                &index,
            );
            add_edges_for_targets(
                src_idx,
                node.see_also.as_deref().unwrap_or(&[]),
                RelationKind::SeeAlso,
                &pkg.package,
                &mut inner,
                &index,
            );
        }
    }

    AgmGraph { inner, index }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Adds edges for all nodes in `nodes` based on their relationship fields.
/// Targets are looked up directly in `index` (no qualification applied).
fn add_edges_from_nodes(
    nodes: &[crate::model::node::Node],
    inner: &mut DiGraph<String, RelationKind>,
    index: &HashMap<String, petgraph::graph::NodeIndex>,
) {
    for node in nodes {
        let Some(&src_idx) = index.get(&node.id) else {
            continue;
        };

        for (targets, kind) in [
            (
                node.depends.as_deref().unwrap_or(&[]),
                RelationKind::Depends,
            ),
            (
                node.related_to.as_deref().unwrap_or(&[]),
                RelationKind::RelatedTo,
            ),
            (
                node.replaces.as_deref().unwrap_or(&[]),
                RelationKind::Replaces,
            ),
            (
                node.conflicts.as_deref().unwrap_or(&[]),
                RelationKind::Conflicts,
            ),
            (
                node.see_also.as_deref().unwrap_or(&[]),
                RelationKind::SeeAlso,
            ),
        ] {
            for target in targets {
                if let Some(&tgt_idx) = index.get(target) {
                    inner.add_edge(src_idx, tgt_idx, kind);
                }
            }
        }
    }
}

/// Adds edges from a qualified source node for imported package nodes.
/// Tries `target` as-is first, then qualifies it as `{package}.{target}`.
fn add_edges_for_targets(
    src_idx: petgraph::graph::NodeIndex,
    targets: &[String],
    kind: RelationKind,
    package: &str,
    inner: &mut DiGraph<String, RelationKind>,
    index: &HashMap<String, petgraph::graph::NodeIndex>,
) {
    for target in targets {
        let tgt_idx = if let Some(&idx) = index.get(target.as_str()) {
            idx
        } else {
            let qualified = format!("{package}.{target}");
            if let Some(&idx) = index.get(&qualified) {
                idx
            } else {
                continue;
            }
        };
        inner.add_edge(src_idx, tgt_idx, kind);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::graph::test_helpers::*;

    #[test]
    fn test_build_graph_empty_file_returns_empty_graph() {
        let file = make_file(vec![]);
        let graph = build_graph(&file);
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_build_graph_single_node_no_deps_returns_one_node() {
        let file = make_file(vec![make_node("a")]);
        let graph = build_graph(&file);
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
        assert!(graph.contains_node("a"));
    }

    #[test]
    fn test_build_graph_depends_edges_created() {
        let mut a = make_node("a");
        a.depends = Some(vec!["b".to_owned()]);
        let b = make_node("b");
        let file = make_file(vec![a, b]);
        let graph = build_graph(&file);
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        let edges = graph.edges_of_kind("a", RelationKind::Depends);
        assert_eq!(edges, vec!["b"]);
    }

    #[test]
    fn test_build_graph_all_relationship_types() {
        let mut a = make_node("a");
        let b = make_node("b");
        let c = make_node("c");
        let d = make_node("d");
        let e = make_node("e");
        let f = make_node("f");
        a.depends = Some(vec!["b".to_owned()]);
        a.related_to = Some(vec!["c".to_owned()]);
        a.replaces = Some(vec!["d".to_owned()]);
        a.conflicts = Some(vec!["e".to_owned()]);
        a.see_also = Some(vec!["f".to_owned()]);
        let file = make_file(vec![a, b, c, d, e, f]);
        let graph = build_graph(&file);
        assert_eq!(graph.node_count(), 6);
        assert_eq!(graph.edge_count(), 5);
        assert!(!graph.edges_of_kind("a", RelationKind::Depends).is_empty());
        assert!(!graph.edges_of_kind("a", RelationKind::RelatedTo).is_empty());
        assert!(!graph.edges_of_kind("a", RelationKind::Replaces).is_empty());
        assert!(!graph.edges_of_kind("a", RelationKind::Conflicts).is_empty());
        assert!(!graph.edges_of_kind("a", RelationKind::SeeAlso).is_empty());
    }

    #[test]
    fn test_build_graph_unknown_target_skipped() {
        let mut a = make_node("a");
        a.depends = Some(vec!["nonexistent".to_owned()]);
        let file = make_file(vec![a]);
        let graph = build_graph(&file);
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_build_graph_with_imports_adds_qualified_nodes() {
        let file = make_file(vec![]);
        let imported_node = make_node("auth.rules");
        let pkg = ResolvedPackage {
            package: "shared.security".to_owned(),
            version: semver::Version::parse("1.0.0").unwrap(),
            path: PathBuf::from("shared.security.agm"),
            file: make_file(vec![imported_node]),
        };
        let graph = build_graph_with_imports(&file, &[pkg]);
        assert!(graph.contains_node("shared.security.auth.rules"));
    }

    #[test]
    fn test_build_graph_with_imports_cross_package_edge() {
        let mut local = make_node("local.node");
        local.depends = Some(vec!["shared.security.auth.rules".to_owned()]);
        let file = make_file(vec![local]);

        let imported_node = make_node("auth.rules");
        let pkg = ResolvedPackage {
            package: "shared.security".to_owned(),
            version: semver::Version::parse("1.0.0").unwrap(),
            path: PathBuf::from("shared.security.agm"),
            file: make_file(vec![imported_node]),
        };
        let graph = build_graph_with_imports(&file, &[pkg]);
        assert_eq!(graph.node_count(), 2);
        let edges = graph.edges_of_kind("local.node", RelationKind::Depends);
        assert!(edges.contains(&"shared.security.auth.rules"));
    }
}
