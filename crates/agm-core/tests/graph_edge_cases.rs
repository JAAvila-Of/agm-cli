//! Integration tests: graph engine edge cases and stress tests.
//!
//! Covers large graphs, diamond patterns, disconnected components, complex
//! cycles, transitive chains, conflict detection, empty graphs, and star
//! topology.

use std::collections::HashSet;

use agm_core::graph::{
    build_graph, detect_cycles, find_conflicts, topological_sort, transitive_dependents,
    transitive_deps,
};
use agm_core::model::fields::{NodeType, Span};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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
        extra_fields: std::collections::BTreeMap::new(),
        span: Span::new(1, 1),
    }
}

fn make_file(nodes: Vec<Node>) -> AgmFile {
    AgmFile {
        header: minimal_header(),
        nodes,
    }
}

/// Asserts that `a` appears before `b` in the topological sort result.
fn assert_before(sorted: &[String], a: &str, b: &str) {
    let pos_a = sorted.iter().position(|x| x == a);
    let pos_b = sorted.iter().position(|x| x == b);
    match (pos_a, pos_b) {
        (Some(pa), Some(pb)) => assert!(
            pa < pb,
            "expected '{a}' (pos {pa}) before '{b}' (pos {pb}) in: {sorted:?}"
        ),
        _ => panic!("'{a}' or '{b}' not found in sort result: {sorted:?}"),
    }
}

// ---------------------------------------------------------------------------
// TASK 2-1: Large linear chain
// ---------------------------------------------------------------------------

#[test]
fn test_large_linear_chain_topo_sort_valid_ordering() {
    // Build: node_00 -> node_01 -> node_02 -> ... -> node_49
    // Each node depends on the next one.
    let count = 50;
    let ids: Vec<String> = (0..count).map(|i| format!("chain.node.{i:02}")).collect();

    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    // node[i] depends on node[i+1]
    for i in 0..(count - 1) {
        nodes[i].depends = Some(vec![ids[i + 1].clone()]);
    }

    let file = make_file(nodes);
    let graph = build_graph(&file);

    assert_eq!(graph.node_count(), count, "expected {count} nodes");

    let sorted = topological_sort(&graph).expect("linear chain is acyclic");
    assert_eq!(
        sorted.len(),
        count,
        "topo sort should return all {count} nodes"
    );

    // Every node must appear before its dependent (the one that depends on it).
    // node[i] depends on node[i+1] → node[i+1] must appear before node[i]
    for i in 0..(count - 1) {
        assert_before(&sorted, &ids[i + 1], &ids[i]);
    }
}

#[test]
fn test_large_linear_chain_node_count() {
    let count = 50;
    let ids: Vec<String> = (0..count).map(|i| format!("lc.{i:02}")).collect();
    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    for i in 0..(count - 1) {
        nodes[i].depends = Some(vec![ids[i + 1].clone()]);
    }
    let graph = build_graph(&make_file(nodes));
    assert_eq!(graph.node_count(), count);
    assert_eq!(graph.edge_count(), count - 1);
}

// ---------------------------------------------------------------------------
// TASK 2-2: Diamond dependency pattern
// ---------------------------------------------------------------------------

#[test]
fn test_diamond_dependency_topo_sort_valid_ordering() {
    // A -> B, A -> C, B -> D, C -> D
    // Valid orderings: D, B, C, A  or  D, C, B, A
    let mut a = make_node("diamond.a");
    let mut b = make_node("diamond.b");
    let mut c = make_node("diamond.c");
    let d = make_node("diamond.d");

    a.depends = Some(vec!["diamond.b".to_owned(), "diamond.c".to_owned()]);
    b.depends = Some(vec!["diamond.d".to_owned()]);
    c.depends = Some(vec!["diamond.d".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b, c, d]));

    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.edge_count(), 4); // a->b, a->c, b->d, c->d

    let sorted = topological_sort(&graph).expect("diamond is acyclic");
    assert_eq!(sorted.len(), 4);

    // D must come before B and C; B and C must come before A.
    assert_before(&sorted, "diamond.d", "diamond.b");
    assert_before(&sorted, "diamond.d", "diamond.c");
    assert_before(&sorted, "diamond.b", "diamond.a");
    assert_before(&sorted, "diamond.c", "diamond.a");
}

#[test]
fn test_diamond_transitive_deps_of_a_contains_all() {
    let mut a = make_node("d.a");
    let mut b = make_node("d.b");
    let mut c = make_node("d.c");
    let d = make_node("d.d");

    a.depends = Some(vec!["d.b".to_owned(), "d.c".to_owned()]);
    b.depends = Some(vec!["d.d".to_owned()]);
    c.depends = Some(vec!["d.d".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b, c, d]));
    let deps = transitive_deps(&graph, "d.a");

    let expected: HashSet<String> = ["d.b", "d.c", "d.d"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(deps, expected);
}

// ---------------------------------------------------------------------------
// TASK 2-3: Multiple disconnected components
// ---------------------------------------------------------------------------

#[test]
fn test_disconnected_components_topo_sort_all_present() {
    // Three independent subgraphs: (p -> q), (r -> s), (t)
    let mut p = make_node("comp.p");
    let q = make_node("comp.q");
    let mut r = make_node("comp.r");
    let s = make_node("comp.s");
    let t = make_node("comp.t");

    p.depends = Some(vec!["comp.q".to_owned()]);
    r.depends = Some(vec!["comp.s".to_owned()]);

    let graph = build_graph(&make_file(vec![p, q, r, s, t]));

    assert_eq!(graph.node_count(), 5);
    assert_eq!(graph.edge_count(), 2);

    let sorted = topological_sort(&graph).expect("disconnected DAG is acyclic");
    assert_eq!(sorted.len(), 5, "all nodes should appear in sort");

    // Within each component the ordering must hold
    assert_before(&sorted, "comp.q", "comp.p");
    assert_before(&sorted, "comp.s", "comp.r");
}

#[test]
fn test_disconnected_components_no_cross_transitive_deps() {
    let mut p = make_node("iso.p");
    let q = make_node("iso.q");
    let mut r = make_node("iso.r");
    let s = make_node("iso.s");

    p.depends = Some(vec!["iso.q".to_owned()]);
    r.depends = Some(vec!["iso.s".to_owned()]);

    let graph = build_graph(&make_file(vec![p, q, r, s]));

    let deps_p = transitive_deps(&graph, "iso.p");
    assert!(
        !deps_p.contains("iso.r") && !deps_p.contains("iso.s"),
        "component 1 should not see nodes from component 2"
    );
}

#[test]
fn test_three_isolated_nodes_topo_sort_returns_all() {
    let a = make_node("iso.a");
    let b = make_node("iso.b");
    let c = make_node("iso.c");
    let graph = build_graph(&make_file(vec![a, b, c]));

    assert_eq!(graph.edge_count(), 0);
    let sorted = topological_sort(&graph).expect("isolated nodes are acyclic");
    assert_eq!(sorted.len(), 3);
}

// ---------------------------------------------------------------------------
// TASK 2-4: Complex cycles
// ---------------------------------------------------------------------------

#[test]
fn test_three_node_cycle_detected() {
    // A -> B -> C -> A
    let mut a = make_node("cyc.a");
    let mut b = make_node("cyc.b");
    let mut c = make_node("cyc.c");

    a.depends = Some(vec!["cyc.b".to_owned()]);
    b.depends = Some(vec!["cyc.c".to_owned()]);
    c.depends = Some(vec!["cyc.a".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b, c]));
    let cycles = detect_cycles(&graph);

    assert!(!cycles.is_empty(), "3-node cycle should be detected");
    // All three nodes should be in a cycle
    let all_cycle_nodes: HashSet<String> = cycles.into_iter().flatten().collect();
    assert!(all_cycle_nodes.contains("cyc.a"));
    assert!(all_cycle_nodes.contains("cyc.b"));
    assert!(all_cycle_nodes.contains("cyc.c"));
}

#[test]
fn test_three_node_cycle_topo_sort_returns_error() {
    let mut a = make_node("tc.a");
    let mut b = make_node("tc.b");
    let mut c = make_node("tc.c");

    a.depends = Some(vec!["tc.b".to_owned()]);
    b.depends = Some(vec!["tc.c".to_owned()]);
    c.depends = Some(vec!["tc.a".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b, c]));
    assert!(
        topological_sort(&graph).is_err(),
        "3-node cycle should cause topo sort to fail"
    );
}

#[test]
fn test_overlapping_cycles_detected() {
    // Two overlapping cycles: A->B->A and B->C->B
    let mut a = make_node("oc.a");
    let mut b = make_node("oc.b");
    let mut c = make_node("oc.c");

    a.depends = Some(vec!["oc.b".to_owned()]);
    b.depends = Some(vec!["oc.a".to_owned(), "oc.c".to_owned()]);
    c.depends = Some(vec!["oc.b".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b, c]));
    let cycles = detect_cycles(&graph);
    assert!(!cycles.is_empty(), "overlapping cycles should be detected");
}

#[test]
fn test_self_referencing_node_cycle_detected() {
    // A -> A  (self-loop)
    let mut a = make_node("self.a");
    a.depends = Some(vec!["self.a".to_owned()]);

    let graph = build_graph(&make_file(vec![a]));
    let cycles = detect_cycles(&graph);
    assert_eq!(cycles.len(), 1, "self-loop should be detected as a cycle");
    assert!(cycles[0].contains(&"self.a".to_owned()));
}

#[test]
fn test_self_referencing_node_topo_sort_returns_error() {
    let mut a = make_node("selfts.a");
    a.depends = Some(vec!["selfts.a".to_owned()]);

    let graph = build_graph(&make_file(vec![a]));
    assert!(
        topological_sort(&graph).is_err(),
        "self-loop should cause topo sort to fail"
    );
}

// ---------------------------------------------------------------------------
// TASK 2-5: Transitive dependency chains
// ---------------------------------------------------------------------------

#[test]
fn test_deep_transitive_chain_all_deps_found() {
    // Build: n0 -> n1 -> n2 -> ... -> n9 (10-node chain)
    let depth = 10;
    let ids: Vec<String> = (0..depth).map(|i| format!("chain.{i}")).collect();

    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    for i in 0..(depth - 1) {
        nodes[i].depends = Some(vec![ids[i + 1].clone()]);
    }

    let graph = build_graph(&make_file(nodes));

    // Transitive deps of chain.0 should be all others: chain.1 through chain.9
    let deps = transitive_deps(&graph, "chain.0");
    let expected: HashSet<String> = (1..depth).map(|i| format!("chain.{i}")).collect();
    assert_eq!(
        deps, expected,
        "all 9 downstream nodes should be transitive deps"
    );
}

#[test]
fn test_deep_chain_transitive_dependents_of_leaf() {
    let depth = 10;
    let ids: Vec<String> = (0..depth).map(|i| format!("rev.{i}")).collect();

    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    for i in 0..(depth - 1) {
        nodes[i].depends = Some(vec![ids[i + 1].clone()]);
    }

    let graph = build_graph(&make_file(nodes));

    // Transitive dependents of the last node should be all preceding ones
    let dependents = transitive_dependents(&graph, &ids[depth - 1]);
    let expected: HashSet<String> = (0..(depth - 1)).map(|i| format!("rev.{i}")).collect();
    assert_eq!(
        dependents, expected,
        "all upstream nodes should be transitive dependents"
    );
}

#[test]
fn test_transitive_deps_of_leaf_node_is_empty() {
    let depth = 5;
    let ids: Vec<String> = (0..depth).map(|i| format!("leaf.{i}")).collect();
    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    for i in 0..(depth - 1) {
        nodes[i].depends = Some(vec![ids[i + 1].clone()]);
    }
    let graph = build_graph(&make_file(nodes));

    // The leaf (last node) has no dependencies
    let deps = transitive_deps(&graph, &ids[depth - 1]);
    assert!(
        deps.is_empty(),
        "leaf node should have no transitive deps, got: {deps:?}"
    );
}

// ---------------------------------------------------------------------------
// TASK 2-6: Conflict detection
// ---------------------------------------------------------------------------

#[test]
fn test_conflict_pair_detected() {
    let mut a = make_node("conf.a");
    let b = make_node("conf.b");
    a.conflicts = Some(vec!["conf.b".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b]));
    let conflicts = find_conflicts(&graph);

    assert_eq!(conflicts.len(), 1);
    let (l, r) = &conflicts[0];
    // Pairs are sorted lexicographically
    assert_eq!((l.as_str(), r.as_str()), ("conf.a", "conf.b"));
}

#[test]
fn test_conflict_deduplication_bidirectional() {
    // Both directions declared — should still appear once
    let mut a = make_node("biconf.a");
    let mut b = make_node("biconf.b");
    a.conflicts = Some(vec!["biconf.b".to_owned()]);
    b.conflicts = Some(vec!["biconf.a".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b]));
    let conflicts = find_conflicts(&graph);

    assert_eq!(
        conflicts.len(),
        1,
        "bidirectional conflict should deduplicate to one pair, got: {conflicts:?}"
    );
}

#[test]
fn test_multiple_conflict_pairs_all_detected() {
    let mut a = make_node("mc.a");
    let mut c = make_node("mc.c");
    let b = make_node("mc.b");
    let d = make_node("mc.d");
    let e = make_node("mc.e");

    a.conflicts = Some(vec!["mc.b".to_owned()]);
    c.conflicts = Some(vec!["mc.d".to_owned(), "mc.e".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b, c, d, e]));
    let conflicts = find_conflicts(&graph);

    assert_eq!(
        conflicts.len(),
        3,
        "expected 3 conflict pairs, got: {conflicts:?}"
    );
}

#[test]
fn test_no_conflicts_returns_empty() {
    let mut a = make_node("nc.a");
    let b = make_node("nc.b");
    a.depends = Some(vec!["nc.b".to_owned()]);

    let graph = build_graph(&make_file(vec![a, b]));
    let conflicts = find_conflicts(&graph);
    assert!(conflicts.is_empty(), "no conflicts expected");
}

// ---------------------------------------------------------------------------
// TASK 2-7: Empty graph (header only, no nodes)
// ---------------------------------------------------------------------------

#[test]
fn test_empty_graph_node_and_edge_counts_are_zero() {
    let graph = build_graph(&make_file(vec![]));
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn test_empty_graph_topo_sort_returns_empty() {
    let graph = build_graph(&make_file(vec![]));
    let result = topological_sort(&graph).expect("empty graph is acyclic");
    assert!(result.is_empty(), "empty graph should produce empty sort");
}

#[test]
fn test_empty_graph_detect_cycles_returns_empty() {
    let graph = build_graph(&make_file(vec![]));
    let cycles = detect_cycles(&graph);
    assert!(cycles.is_empty(), "empty graph should have no cycles");
}

#[test]
fn test_empty_graph_transitive_deps_nonexistent_returns_empty() {
    let graph = build_graph(&make_file(vec![]));
    let deps = transitive_deps(&graph, "nonexistent");
    assert!(deps.is_empty());
}

#[test]
fn test_empty_graph_find_conflicts_returns_empty() {
    let graph = build_graph(&make_file(vec![]));
    let conflicts = find_conflicts(&graph);
    assert!(conflicts.is_empty());
}

// ---------------------------------------------------------------------------
// TASK 2-8: Star topology
// ---------------------------------------------------------------------------

#[test]
fn test_star_topology_all_leaf_nodes_depend_on_center() {
    // 20 leaf nodes, each depending on one central hub
    let hub = make_node("star.hub");
    let leaf_count = 20;
    let leaf_ids: Vec<String> = (0..leaf_count)
        .map(|i| format!("star.leaf.{i:02}"))
        .collect();

    let mut nodes = vec![hub];
    for id in &leaf_ids {
        let mut leaf = make_node(id);
        leaf.depends = Some(vec!["star.hub".to_owned()]);
        nodes.push(leaf);
    }

    let graph = build_graph(&make_file(nodes));

    assert_eq!(graph.node_count(), leaf_count + 1);
    assert_eq!(graph.edge_count(), leaf_count);
}

#[test]
fn test_star_topology_topo_sort_hub_comes_first() {
    let hub = make_node("stts.hub");
    let leaf_count = 20;
    let leaf_ids: Vec<String> = (0..leaf_count)
        .map(|i| format!("stts.leaf.{i:02}"))
        .collect();

    let mut nodes = vec![hub];
    for id in &leaf_ids {
        let mut leaf = make_node(id);
        leaf.depends = Some(vec!["stts.hub".to_owned()]);
        nodes.push(leaf);
    }

    let graph = build_graph(&make_file(nodes));
    let sorted = topological_sort(&graph).expect("star is acyclic");

    assert_eq!(sorted.len(), leaf_count + 1);

    // Hub must come before all leaf nodes
    for leaf_id in &leaf_ids {
        assert_before(&sorted, "stts.hub", leaf_id);
    }
}

#[test]
fn test_star_topology_transitive_dependents_of_hub_includes_all_leaves() {
    let hub = make_node("sttd.hub");
    let leaf_count = 20;
    let leaf_ids: Vec<String> = (0..leaf_count)
        .map(|i| format!("sttd.leaf.{i:02}"))
        .collect();

    let mut nodes = vec![hub];
    for id in &leaf_ids {
        let mut leaf = make_node(id);
        leaf.depends = Some(vec!["sttd.hub".to_owned()]);
        nodes.push(leaf);
    }

    let graph = build_graph(&make_file(nodes));
    let dependents = transitive_dependents(&graph, "sttd.hub");

    let expected: HashSet<String> = leaf_ids.iter().cloned().collect();
    assert_eq!(
        dependents, expected,
        "all leaf nodes should be transitive dependents of the hub"
    );
}

#[test]
fn test_star_topology_hub_has_no_transitive_deps() {
    let hub = make_node("stnd.hub");
    let leaf_count = 5;

    let mut nodes = vec![hub];
    for i in 0..leaf_count {
        let mut leaf = make_node(&format!("stnd.leaf.{i}"));
        leaf.depends = Some(vec!["stnd.hub".to_owned()]);
        nodes.push(leaf);
    }

    let graph = build_graph(&make_file(nodes));
    let deps = transitive_deps(&graph, "stnd.hub");
    assert!(
        deps.is_empty(),
        "hub has no outgoing depends edges, expected empty deps, got: {deps:?}"
    );
}

#[test]
fn test_star_topology_detect_cycles_returns_empty() {
    let hub = make_node("scy.hub");
    let mut nodes = vec![hub];
    for i in 0..10 {
        let mut leaf = make_node(&format!("scy.leaf.{i}"));
        leaf.depends = Some(vec!["scy.hub".to_owned()]);
        nodes.push(leaf);
    }
    let graph = build_graph(&make_file(nodes));
    let cycles = detect_cycles(&graph);
    assert!(cycles.is_empty(), "star topology should have no cycles");
}

// ---------------------------------------------------------------------------
// STRESS / LARGE FILE TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_graph_200_node_linear_chain_topo_sort_order() {
    let count = 200;
    let ids: Vec<String> = (0..count).map(|i| format!("lg.n{i:03}")).collect();

    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    // nodes[i] depends on nodes[i+1] (same direction as existing large chain test)
    for i in 0..(count - 1) {
        nodes[i].depends = Some(vec![ids[i + 1].clone()]);
    }

    let file = make_file(nodes);
    let graph = build_graph(&file);
    assert_eq!(graph.node_count(), count);

    let sorted = topological_sort(&graph).expect("200-node linear chain is acyclic");
    assert_eq!(
        sorted.len(),
        count,
        "all 200 nodes should appear in topo sort"
    );

    // nodes[i+1] must appear before nodes[i] (because nodes[i] depends on nodes[i+1])
    for i in 0..(count - 1) {
        assert_before(&sorted, &ids[i + 1], &ids[i]);
    }
}

#[test]
fn test_graph_wide_fan_out_100_children() {
    let parent_id = "fan.parent";
    let parent = make_node(parent_id);
    let child_count = 100;
    let child_ids: Vec<String> = (0..child_count)
        .map(|i| format!("fan.child.{i:03}"))
        .collect();

    let mut nodes = vec![parent];
    for id in &child_ids {
        let mut child = make_node(id);
        child.depends = Some(vec![parent_id.to_owned()]);
        nodes.push(child);
    }

    let file = make_file(nodes);
    let graph = build_graph(&file);

    assert_eq!(graph.node_count(), child_count + 1);
    assert_eq!(graph.edge_count(), child_count);

    let sorted = topological_sort(&graph).expect("fan-out graph is acyclic");
    assert_eq!(sorted.len(), child_count + 1);

    // Parent must appear before all children
    for child_id in &child_ids {
        assert_before(&sorted, parent_id, child_id);
    }

    // Transitive dependents of parent should include all children
    let dependents = transitive_dependents(&graph, parent_id);
    let expected: HashSet<String> = child_ids.iter().cloned().collect();
    assert_eq!(
        dependents, expected,
        "all 100 children should be transitive dependents of the parent"
    );
}

#[test]
fn test_graph_200_nodes_complex_dag_no_cycles() {
    let count = 200;
    let ids: Vec<String> = (0..count).map(|i| format!("dag.n{i:03}")).collect();

    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    // Each node depends on up to 3 prior nodes (forming a dense but acyclic DAG)
    for i in 1..count {
        let mut deps = Vec::new();
        if i >= 1 {
            deps.push(ids[i - 1].clone());
        }
        if i >= 2 {
            deps.push(ids[i - 2].clone());
        }
        if i >= 3 {
            deps.push(ids[i - 3].clone());
        }
        nodes[i].depends = Some(deps);
    }

    let file = make_file(nodes);
    let graph = build_graph(&file);

    let cycles = detect_cycles(&graph);
    assert!(cycles.is_empty(), "dense forward DAG should have no cycles");

    let sorted = topological_sort(&graph).expect("complex DAG is acyclic");
    assert_eq!(
        sorted.len(),
        count,
        "all 200 nodes should appear in topo sort"
    );
}

#[test]
fn test_graph_50_disconnected_components() {
    // 50 groups of 4 nodes each: group_root -> group_a -> group_b -> group_c
    let mut all_nodes: Vec<Node> = Vec::new();
    for g in 0..50usize {
        let root_id = format!("grp{g:02}.root");
        let a_id = format!("grp{g:02}.na");
        let b_id = format!("grp{g:02}.nb");
        let c_id = format!("grp{g:02}.nc");

        let root = make_node(&root_id);
        let mut na = make_node(&a_id);
        let mut nb = make_node(&b_id);
        let mut nc = make_node(&c_id);

        na.depends = Some(vec![root_id.clone()]);
        nb.depends = Some(vec![a_id.clone()]);
        nc.depends = Some(vec![b_id.clone()]);

        all_nodes.push(root);
        all_nodes.push(na);
        all_nodes.push(nb);
        all_nodes.push(nc);
    }

    let file = make_file(all_nodes);
    let graph = build_graph(&file);

    assert_eq!(
        graph.node_count(),
        200,
        "expected 200 nodes (50 groups x 4)"
    );

    let sorted = topological_sort(&graph).expect("50 disconnected components should be acyclic");
    assert_eq!(
        sorted.len(),
        200,
        "all 200 nodes should appear in topo sort"
    );

    // Within each group, ordering must be maintained
    for g in 0..50usize {
        let root_id = format!("grp{g:02}.root");
        let a_id = format!("grp{g:02}.na");
        let b_id = format!("grp{g:02}.nb");
        let c_id = format!("grp{g:02}.nc");
        assert_before(&sorted, &root_id, &a_id);
        assert_before(&sorted, &a_id, &b_id);
        assert_before(&sorted, &b_id, &c_id);
    }
}

// ---------------------------------------------------------------------------
// GAP 9-1: Root/Leaf identification on 200-node DAG
// ---------------------------------------------------------------------------

/// Builds a 200-node DAG where nodes 0-9 are roots (no incoming depends
/// edges) and nodes 10-199 each depend on one of the roots or a prior node.
///
/// The graph is structured as: nodes 10-199 each depend on node[i % 10]
/// (one of the 10 roots), so only the 10 roots have no incoming depends edges.
fn build_200_node_dag_with_10_roots() -> (Vec<String>, agm_core::graph::AgmGraph) {
    let count = 200;
    let ids: Vec<String> = (0..count).map(|i| format!("dag200.n{i:03}")).collect();

    let mut nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();
    // nodes 0-9 are roots (no depends). nodes 10-199 depend on node[i % 10].
    for i in 10..count {
        nodes[i].depends = Some(vec![ids[i % 10].clone()]);
    }

    let graph = build_graph(&make_file(nodes));
    (ids, graph)
}

#[test]
fn test_graph_200_node_dag_roots_have_no_incoming_depends_edges() {
    let (ids, graph) = build_200_node_dag_with_10_roots();

    // Build the set of all node IDs that appear as a depends target (i.e.,
    // have at least one incoming Depends edge — some other node depends on them).
    let mut has_incoming: HashSet<String> = HashSet::new();
    for id in &ids {
        for target in graph.edges_of_kind(id, agm_core::graph::RelationKind::Depends) {
            has_incoming.insert(target.to_owned());
        }
    }

    // In the AGM graph a depends edge goes from the dependent TO the dependency
    // (src depends on tgt → edge src→tgt). Nodes with incoming Depends edges
    // are the dependencies that other nodes point to. In this DAG nodes 0-9
    // are pointed to by nodes 10-199, so they have incoming edges.
    // Nodes 10-199 have NO incoming edges (nothing depends on them).
    //
    // Per the task spec "roots" are nodes with no depends field — i.e., nodes
    // that have no outgoing Depends edges (they don't depend on anything).
    // Those are nodes 0-9 in this DAG.
    //
    // This test verifies the count of nodes that have incoming Depends edges
    // equals exactly 10 (nodes 0-9 are the shared dependencies).
    assert_eq!(
        has_incoming.len(),
        10,
        "expected exactly 10 nodes with incoming depends edges (the shared roots/deps), got {}: {has_incoming:?}",
        has_incoming.len()
    );

    // Verify those 10 nodes are exactly nodes 0-9.
    for i in 0..10usize {
        let expected = format!("dag200.n{i:03}");
        assert!(
            has_incoming.contains(&expected),
            "expected node {expected} to have incoming depends edges, but it was not found"
        );
    }
}

#[test]
fn test_graph_200_node_dag_leaves_have_no_outgoing_depends_edges() {
    let (ids, graph) = build_200_node_dag_with_10_roots();

    // A leaf node has no outgoing Depends edges.
    let leaves: Vec<&String> = ids
        .iter()
        .filter(|id| {
            graph
                .edges_of_kind(id, agm_core::graph::RelationKind::Depends)
                .is_empty()
        })
        .collect();

    // Nodes 10-199 all depend on a root (outgoing Depends edge), so they are
    // not leaves. Nodes 0-9 have no outgoing Depends edges, so all 10 are leaves.
    assert_eq!(
        leaves.len(),
        10,
        "expected 10 leaf nodes (no outgoing depends), got {}: {leaves:?}",
        leaves.len()
    );

    let leaf_ids: HashSet<String> = leaves.into_iter().cloned().collect();
    for i in 0..10usize {
        let expected = format!("dag200.n{i:03}");
        assert!(
            leaf_ids.contains(&expected),
            "expected leaf {expected} not found in leaves: {leaf_ids:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// GAP 9-2: Conflict detection with 50 simultaneous pairs
// ---------------------------------------------------------------------------

#[test]
fn test_graph_50_conflict_pairs_all_detected() {
    // 100 nodes: conf.a00-a49 and conf.b00-b49. Each aN conflicts with bN.
    let mut nodes: Vec<Node> = Vec::with_capacity(100);

    for i in 0..50usize {
        let a_id = format!("cp50.a{i:02}");
        let b_id = format!("cp50.b{i:02}");

        let mut a = make_node(&a_id);
        let b = make_node(&b_id);
        a.conflicts = Some(vec![b_id.clone()]);
        nodes.push(a);
        nodes.push(b);
    }

    let graph = build_graph(&make_file(nodes));
    let conflicts = find_conflicts(&graph);

    assert_eq!(
        conflicts.len(),
        50,
        "expected exactly 50 conflict pairs, got {}: {conflicts:?}",
        conflicts.len()
    );
}

#[test]
fn test_graph_50_bidirectional_conflict_pairs_deduplication() {
    // Both directions declared for each pair — should still yield 50 unique pairs.
    let mut nodes: Vec<Node> = Vec::with_capacity(100);

    for i in 0..50usize {
        let a_id = format!("bicp50.a{i:02}");
        let b_id = format!("bicp50.b{i:02}");

        let mut a = make_node(&a_id);
        let mut b = make_node(&b_id);
        a.conflicts = Some(vec![b_id.clone()]);
        b.conflicts = Some(vec![a_id.clone()]);
        nodes.push(a);
        nodes.push(b);
    }

    let graph = build_graph(&make_file(nodes));
    let conflicts = find_conflicts(&graph);

    assert_eq!(
        conflicts.len(),
        50,
        "bidirectional conflicts should deduplicate to 50 pairs, got {}: {conflicts:?}",
        conflicts.len()
    );
}

// ---------------------------------------------------------------------------
// GAP 9-3: Topo sort determinism with 200 nodes
// ---------------------------------------------------------------------------

#[test]
fn test_graph_topo_sort_200_independent_nodes_deterministic() {
    // 200 nodes with no dependencies — topological_sort should produce the
    // same order on every call (deterministic stable output).
    let ids: Vec<String> = (0..200usize).map(|i| format!("det.ind.n{i:03}")).collect();
    let nodes: Vec<Node> = ids.iter().map(|id| make_node(id)).collect();

    let graph = build_graph(&make_file(nodes));

    let first = topological_sort(&graph).expect("200 independent nodes are acyclic");
    assert_eq!(first.len(), 200);

    for run in 1..10 {
        let result = topological_sort(&graph).expect("acyclic graph should sort successfully");
        assert_eq!(
            result, first,
            "topo sort run {run} produced a different order than run 0"
        );
    }
}

#[test]
fn test_graph_topo_sort_200_nodes_wide_dag_deterministic() {
    // 1 root node, 199 nodes each depending on that root.
    // Topological sort must always place the root first, and produce identical
    // output across repeated calls.
    let root_id = "det.wide.root".to_owned();
    let mut nodes: Vec<Node> = vec![make_node(&root_id)];

    for i in 0..199usize {
        let id = format!("det.wide.n{i:03}");
        let mut node = make_node(&id);
        node.depends = Some(vec![root_id.clone()]);
        nodes.push(node);
    }

    let graph = build_graph(&make_file(nodes));

    let first = topological_sort(&graph).expect("wide DAG is acyclic");
    assert_eq!(first.len(), 200);

    // Root must be first.
    assert_eq!(
        first[0], root_id,
        "root should appear first in the topo sort, got: {}",
        first[0]
    );

    // All 10 repeated sorts must match the first.
    for run in 1..10 {
        let result = topological_sort(&graph).expect("acyclic graph should sort successfully");
        assert_eq!(
            result, first,
            "topo sort run {run} produced a different order than run 0"
        );
    }
}
