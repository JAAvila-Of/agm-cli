//! Loader: produces filtered views of an AGM AST based on loading modes.
//!
//! Supports summary, operational, executable, full, and profile-based
//! loading as defined in the AGM specification.

pub mod filter;
pub mod mode;
pub mod profile;

pub use filter::filter_node;
pub use mode::LoadMode;
pub use profile::LoadError;

use crate::model::file::AgmFile;

// ---------------------------------------------------------------------------
// load
// ---------------------------------------------------------------------------

/// Returns a filtered view of all nodes in `file` at the given `mode`.
///
/// Every node in the file is kept; only their fields are filtered.
#[must_use]
pub fn load(file: &AgmFile, mode: LoadMode) -> AgmFile {
    let nodes = file.nodes.iter().map(|n| filter_node(n, mode)).collect();

    AgmFile {
        header: file.header.clone(),
        nodes,
    }
}

// ---------------------------------------------------------------------------
// load_nodes
// ---------------------------------------------------------------------------

/// Returns a filtered view containing only the nodes whose IDs appear in
/// `node_ids`, in the same order as they appear in `file.nodes`.
///
/// Unknown IDs are silently ignored.
#[must_use]
pub fn load_nodes(file: &AgmFile, mode: LoadMode, node_ids: &[&str]) -> AgmFile {
    let id_set: std::collections::HashSet<&str> = node_ids.iter().copied().collect();

    let nodes = file
        .nodes
        .iter()
        .filter(|n| id_set.contains(n.id.as_str()))
        .map(|n| filter_node(n, mode))
        .collect();

    AgmFile {
        header: file.header.clone(),
        nodes,
    }
}

// ---------------------------------------------------------------------------
// load_profile
// ---------------------------------------------------------------------------

/// Resolves and applies the named load profile (or the file's `default_load`).
///
/// See [`profile::resolve_and_apply`] for full semantics.
pub fn load_profile(file: &AgmFile, profile_name: Option<&str>) -> Result<AgmFile, LoadError> {
    profile::resolve_and_apply(file, profile_name)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::model::fields::{NodeType, Priority, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;

    use super::*;

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
            summary: format!("node {id}"),
            priority: Some(Priority::High),
            items: Some(vec!["item1".to_owned()]),
            detail: Some("full detail".to_owned()),
            span: Span::new(1, 5),
            ..Default::default()
        }
    }

    fn make_file(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: minimal_header(),
            nodes,
        }
    }

    #[test]
    fn test_load_returns_all_nodes_filtered() {
        let file = make_file(vec![make_node("a"), make_node("b")]);
        let result = load(&file, LoadMode::Summary);
        assert_eq!(result.nodes.len(), 2);
        // `detail` is Full-only; must be absent in Summary mode.
        assert!(result.nodes[0].detail.is_none());
        assert!(result.nodes[1].detail.is_none());
        // priority is Summary-always; must be present.
        assert!(result.nodes[0].priority.is_some());
    }

    #[test]
    fn test_load_nodes_returns_only_requested_ids() {
        let file = make_file(vec![make_node("a"), make_node("b"), make_node("c")]);
        let result = load_nodes(&file, LoadMode::Summary, &["a", "c"]);
        assert_eq!(result.nodes.len(), 2);
        let ids: Vec<&str> = result.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"c"));
        assert!(!ids.contains(&"b"));
    }

    #[test]
    fn test_load_nodes_preserves_file_order() {
        let file = make_file(vec![make_node("x"), make_node("y"), make_node("z")]);
        let result = load_nodes(&file, LoadMode::Summary, &["z", "x"]);
        // Must follow file order (x before z), not the order of the id list.
        assert_eq!(result.nodes[0].id, "x");
        assert_eq!(result.nodes[1].id, "z");
    }

    #[test]
    fn test_load_nodes_ignores_unknown_ids() {
        let file = make_file(vec![make_node("a")]);
        let result = load_nodes(&file, LoadMode::Summary, &["a", "nonexistent"]);
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].id, "a");
    }
}
