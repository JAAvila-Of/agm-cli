//! Graph renderers: DOT and Mermaid output from node relationships.

use petgraph::visit::EdgeRef;

use crate::graph::{RelationKind, build_graph};
use crate::model::fields::NodeType;
use crate::model::file::AgmFile;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Renders the node relationship graph in Graphviz DOT format.
#[must_use]
pub fn render_graph_dot(file: &AgmFile) -> String {
    let graph = build_graph(file);
    let mut buf = String::new();

    let graph_name = &file.header.package;
    buf.push_str(&format!("digraph \"{graph_name}\" {{\n"));
    buf.push_str("    rankdir=LR;\n");
    buf.push_str("    node [shape=box, style=\"rounded,filled\", fontname=\"Helvetica\"];\n");
    buf.push('\n');

    // Node declarations
    buf.push_str("    // Node declarations\n");
    for node in &file.nodes {
        let color = dot_node_color(&node.node_type);
        let label = format!("{}\\n[{}]", node.id, node.node_type);
        buf.push_str(&format!(
            "    \"{}\" [label=\"{}\", fillcolor=\"{}\"];\n",
            node.id, label, color
        ));
    }

    buf.push('\n');

    // Edges
    buf.push_str("    // Edges\n");
    for edge in graph.inner.edge_references() {
        let src = &graph.inner[edge.source()];
        let tgt = &graph.inner[edge.target()];
        let kind = edge.weight();
        let label = rel_kind_label(kind);
        let style_attrs = dot_edge_style(kind);
        buf.push_str(&format!(
            "    \"{}\" -> \"{}\" [label=\"{}\"{style_attrs}];\n",
            src, tgt, label
        ));
    }

    buf.push_str("}\n");
    buf
}

/// Renders the node relationship graph in Mermaid format.
#[must_use]
pub fn render_graph_mermaid(file: &AgmFile) -> String {
    let graph = build_graph(file);
    let mut buf = String::new();

    buf.push_str("graph LR\n");

    // Node declarations
    for node in &file.nodes {
        let mermaid_id = mermaid_id(&node.id);
        let label = format!("{}<br/>[{}]", node.id, node.node_type);
        buf.push_str(&format!("    {mermaid_id}[\"{label}\"]\n"));
    }

    buf.push('\n');

    // Edges
    for edge in graph.inner.edge_references() {
        let src_id = mermaid_id(&graph.inner[edge.source()]);
        let tgt_id = mermaid_id(&graph.inner[edge.target()]);
        let kind = edge.weight();
        let label = rel_kind_label(kind);
        let arrow = mermaid_arrow(kind);
        buf.push_str(&format!("    {src_id} {arrow}|{label}| {tgt_id}\n"));
    }

    buf
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dot_node_color(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Facts => "#e8f4fd",
        NodeType::Rules => "#fef9e7",
        NodeType::Workflow => "#e8fde8",
        NodeType::Entity => "#f3e8fd",
        NodeType::Decision => "#fde8c8",
        NodeType::Exception => "#fde8e8",
        NodeType::AntiPattern => "#fde8f0",
        NodeType::Orchestration => "#f0f0f0",
        _ => "#ffffff",
    }
}

fn rel_kind_label(kind: &RelationKind) -> &'static str {
    match kind {
        RelationKind::Depends => "depends",
        RelationKind::RelatedTo => "related_to",
        RelationKind::Replaces => "replaces",
        RelationKind::Conflicts => "conflicts",
        RelationKind::SeeAlso => "see_also",
    }
}

fn dot_edge_style(kind: &RelationKind) -> String {
    match kind {
        RelationKind::Depends => String::new(),
        RelationKind::RelatedTo => ", style=dashed, color=blue".into(),
        RelationKind::Replaces => ", style=bold, color=orange".into(),
        RelationKind::Conflicts => ", style=dashed, color=red".into(),
        RelationKind::SeeAlso => ", style=dotted, color=gray".into(),
    }
}

fn mermaid_arrow(kind: &RelationKind) -> &'static str {
    match kind {
        RelationKind::Depends => "-->",
        RelationKind::RelatedTo => "-.->",
        RelationKind::Replaces => "==>",
        RelationKind::Conflicts => "-.-",
        RelationKind::SeeAlso => "-.->",
    }
}

fn mermaid_id(node_id: &str) -> String {
    node_id.replace('.', "_")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::NodeType;
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;

    fn make_node(id: &str, node_type: NodeType) -> Node {
        Node {
            id: id.to_owned(),
            node_type,
            summary: format!("node {id}"),
            ..Default::default()
        }
    }

    fn file_with_nodes(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: Header {
                agm: "1".to_owned(),
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
            },
            nodes,
        }
    }

    #[test]
    fn test_render_dot_empty_graph_valid_dot() {
        let file = file_with_nodes(vec![]);
        let output = render_graph_dot(&file);
        assert!(output.starts_with("digraph"));
        assert!(output.ends_with("}\n"));
    }

    #[test]
    fn test_render_dot_has_edges() {
        let mut a = make_node("auth.login", NodeType::Workflow);
        let b = make_node("auth.constraints", NodeType::Rules);
        a.depends = Some(vec!["auth.constraints".into()]);
        let file = file_with_nodes(vec![a, b]);
        let output = render_graph_dot(&file);
        assert!(output.contains("auth.login"));
        assert!(output.contains("auth.constraints"));
        assert!(output.contains("depends"));
        assert!(output.contains("->"));
    }

    #[test]
    fn test_render_dot_conflicts_edge_styled() {
        let mut a = make_node("decision.a", NodeType::Decision);
        let b = make_node("pattern.b", NodeType::AntiPattern);
        a.conflicts = Some(vec!["pattern.b".into()]);
        let file = file_with_nodes(vec![a, b]);
        let output = render_graph_dot(&file);
        assert!(output.contains("conflicts"));
        assert!(output.contains("color=red"));
    }

    #[test]
    fn test_render_mermaid_replaces_dots() {
        let a = make_node("auth.login", NodeType::Workflow);
        let file = file_with_nodes(vec![a]);
        let output = render_graph_mermaid(&file);
        // Node ID in Mermaid should have _ instead of .
        assert!(output.contains("auth_login"));
        assert!(!output.contains("auth_login[\"auth_login"));
        // Label should preserve dots
        assert!(output.contains("auth.login"));
    }

    #[test]
    fn test_render_mermaid_empty_graph_valid() {
        let file = file_with_nodes(vec![]);
        let output = render_graph_mermaid(&file);
        assert!(output.starts_with("graph LR\n"));
    }

    #[test]
    fn test_render_mermaid_has_edges() {
        let mut a = make_node("auth.login", NodeType::Workflow);
        let b = make_node("auth.session", NodeType::Entity);
        a.depends = Some(vec!["auth.session".into()]);
        let file = file_with_nodes(vec![a, b]);
        let output = render_graph_mermaid(&file);
        assert!(output.contains("auth_login"));
        assert!(output.contains("auth_session"));
        assert!(output.contains("-->"));
    }

    #[test]
    fn test_render_dot_node_colors_by_type() {
        let facts = make_node("f.one", NodeType::Facts);
        let workflow = make_node("w.one", NodeType::Workflow);
        let file = file_with_nodes(vec![facts, workflow]);
        let output = render_graph_dot(&file);
        assert!(output.contains("#e8f4fd")); // facts color
        assert!(output.contains("#e8fde8")); // workflow color
    }
}
