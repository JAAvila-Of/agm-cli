//! Integration tests for the graph engine using fixture `.agm` files.

use std::collections::HashSet;

use agm_core::graph::{
    build_graph, detect_cycles, find_conflicts, topological_sort, transitive_deps,
};
use agm_core::parser;

fn fixture(relative: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// auth_platform.agm tests (graph/auth_platform.agm)
// ---------------------------------------------------------------------------

#[test]
fn test_auth_platform_graph_node_and_edge_counts() {
    let src = fixture("graph/auth_platform.agm");
    let file = parser::parse(&src).expect("parse should succeed");
    let graph = build_graph(&file);

    // 10 nodes defined in the fixture.
    assert_eq!(
        graph.node_count(),
        10,
        "expected 10 nodes, got {}",
        graph.node_count()
    );

    // Edges:
    // auth.login depends: [auth.constraints, auth.session, auth.tenant.resolve]        = 3
    // auth.refresh depends: [auth.session, auth.constraints]                           = 2
    // auth.logout depends: [auth.session]                                              = 1
    // auth.logout.no.upstream.endsession depends: [auth.logout]                       = 1
    // auth.session.model.decision depends: [auth.constraints, auth.session]            = 2
    // auth.session.model.decision conflicts: [auth.browser.token.model]               = 1
    // Total = 10 edges
    assert_eq!(
        graph.edge_count(),
        10,
        "expected 10 edges, got {}",
        graph.edge_count()
    );
}

#[test]
fn test_auth_platform_transitive_deps_login() {
    let src = fixture("graph/auth_platform.agm");
    let file = parser::parse(&src).expect("parse should succeed");
    let graph = build_graph(&file);

    let deps = transitive_deps(&graph, "auth.login");
    let expected: HashSet<String> = ["auth.constraints", "auth.session", "auth.tenant.resolve"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(
        deps, expected,
        "transitive_deps(auth.login) = {deps:?}, expected {expected:?}"
    );
}

#[test]
fn test_auth_platform_conflict_detection() {
    let src = fixture("graph/auth_platform.agm");
    let file = parser::parse(&src).expect("parse should succeed");
    let graph = build_graph(&file);

    let conflicts = find_conflicts(&graph);

    assert_eq!(
        conflicts,
        vec![(
            "auth.browser.token.model".to_owned(),
            "auth.session.model.decision".to_owned()
        )]
    );
}

// ---------------------------------------------------------------------------
// Cycle fixture tests (graph/cycle.agm)
// ---------------------------------------------------------------------------

#[test]
fn test_cycle_fixture_detected() {
    let src = fixture("graph/cycle.agm");
    let file = parser::parse(&src).expect("parse should succeed");
    let graph = build_graph(&file);

    let cycles = detect_cycles(&graph);
    assert!(
        !cycles.is_empty(),
        "detect_cycles should return non-empty for cycle.agm"
    );
}

#[test]
fn test_topo_sort_cycle_fixture_returns_error() {
    let src = fixture("graph/cycle.agm");
    let file = parser::parse(&src).expect("parse should succeed");
    let graph = build_graph(&file);

    let result = topological_sort(&graph);
    assert!(
        result.is_err(),
        "topological_sort should return Err(CycleError) for cycle.agm"
    );
}
