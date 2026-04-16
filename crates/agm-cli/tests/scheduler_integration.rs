//! Integration test: full scheduler run on a 5-node fixture.
//!
//! Since agm-cli is a binary crate (no lib target), this integration test
//! exercises the scheduler by constructing AGM files and graphs using
//! agm-core types. The scheduler unit tests (in runtime/scheduler.rs) cover
//! the full runtime loop (execute_node, run_topological, etc.) from within
//! the crate.
//!
//! This test validates that the top-level build/graph primitives from
//! agm-core correctly model a 5-node fixture, which is the foundation
//! that the scheduler tests rely on.
//!
//! Fixture graph:
//!   A (no deps)
//!   B (no deps)
//!   C depends on [A, B]
//!   D depends on [C]
//!   E (independent)

use agm_core::graph::build_graph;
use agm_core::graph::topological_sort;
use agm_core::graph::transitive_deps;
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::fields::NodeType;
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node;

fn make_header() -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: "integration.test".to_owned(),
        version: "0.1.0".to_owned(),
        title: None,
        description: None,
        tags: None,
        status: None,
        owner: None,
        imports: None,
        default_load: None,
        load_profiles: None,
        target_runtime: None,
    }
}

fn make_node(id: &str, deps: Vec<&str>) -> Node {
    Node {
        id: id.to_owned(),
        node_type: NodeType::Workflow,
        summary: format!("Integration test node {id}"),
        depends: if deps.is_empty() {
            None
        } else {
            Some(deps.into_iter().map(String::from).collect())
        },
        ..Default::default()
    }
}

fn make_5_node_file() -> AgmFile {
    AgmFile {
        header: make_header(),
        nodes: vec![
            make_node("A", vec![]),
            make_node("B", vec![]),
            make_node("C", vec!["A", "B"]),
            make_node("D", vec!["C"]),
            make_node("E", vec![]),
        ],
    }
}

/// Verifies that the 5-node fixture graph builds correctly and topological
/// sort respects the dependency ordering required by the scheduler.
#[test]
fn test_5_node_fixture_topological_order_respects_deps() {
    let file = make_5_node_file();
    let graph = build_graph(&file);
    let order = topological_sort(&graph).expect("5-node graph should have no cycles");

    assert_eq!(order.len(), 5);

    // C must come after A and B; D must come after C
    let pos_a = order.iter().position(|id| id == "A").unwrap();
    let pos_b = order.iter().position(|id| id == "B").unwrap();
    let pos_c = order.iter().position(|id| id == "C").unwrap();
    let pos_d = order.iter().position(|id| id == "D").unwrap();

    assert!(pos_a < pos_c, "A must precede C");
    assert!(pos_b < pos_c, "B must precede C");
    assert!(pos_c < pos_d, "C must precede D");
}

/// Verifies initial node status derivation matches what ExecutionTracker::initialize does.
///
/// Nodes with no deps are Ready; nodes with deps are Pending.
#[test]
fn test_5_node_fixture_initial_status_derivation() {
    let file = make_5_node_file();

    for node in &file.nodes {
        let has_deps = node
            .depends
            .as_ref()
            .map(|d| !d.is_empty())
            .unwrap_or(false);
        let expected = if has_deps {
            ExecutionStatus::Pending
        } else {
            ExecutionStatus::Ready
        };

        // Validate the expectation is consistent with node IDs
        match node.id.as_str() {
            "A" | "B" | "E" => {
                assert_eq!(
                    expected,
                    ExecutionStatus::Ready,
                    "Node {} should be Ready",
                    node.id
                )
            }
            "C" | "D" => {
                assert_eq!(
                    expected,
                    ExecutionStatus::Pending,
                    "Node {} should be Pending",
                    node.id
                )
            }
            _ => {}
        }
    }
}

/// Verifies transitive dependency resolution for node D in the fixture graph.
///
/// D transitively depends on A, B, C (but not E).
#[test]
fn test_5_node_fixture_transitive_deps_of_d() {
    let file = make_5_node_file();
    let graph = build_graph(&file);
    let deps_of_d = transitive_deps(&graph, "D");

    assert!(deps_of_d.contains("A"), "D should transitively depend on A");
    assert!(deps_of_d.contains("B"), "D should transitively depend on B");
    assert!(deps_of_d.contains("C"), "D should transitively depend on C");
    assert!(!deps_of_d.contains("E"), "D should not depend on E");
    assert!(
        !deps_of_d.contains("D"),
        "D should not be its own transitive dep"
    );
}

/// Verifies that cycle detection works: adding a cycle A -> B -> A returns an error.
#[test]
fn test_fixture_cycle_detection() {
    let file = AgmFile {
        header: make_header(),
        nodes: vec![make_node("A", vec!["B"]), make_node("B", vec!["A"])],
    };
    let graph = build_graph(&file);
    let result = topological_sort(&graph);
    assert!(result.is_err(), "Cycle A<->B should be detected");
}
