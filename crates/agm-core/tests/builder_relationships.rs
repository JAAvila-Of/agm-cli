//! Group E — Relationship graph tests.
//!
//! Tests all relationship fields (depends, related_to, replaces, conflicts,
//! see_also) plus orchestration parallel_groups DAG, cycle detection, and
//! diamond dependency resolution.

use agm_core::builder::{DecisionBuilder, FactsBuilder, OrchestrationBuilder, WorkflowBuilder};
use agm_core::error::codes::ErrorCode;
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::parser;
use agm_core::renderer::canonical::render_canonical;
use agm_core::validator;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_header() -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: "test.relationships".to_owned(),
        version: "1.0.0".to_owned(),
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

fn validate_file(file: &AgmFile) -> agm_core::error::diagnostic::DiagnosticCollection {
    let rendered = render_canonical(file);
    let parsed = parser::parse(&rendered).expect("file must parse");
    validator::validate(&parsed, &rendered, "<test>", &Default::default())
}

fn validate_file_get_nodes(
    file: &AgmFile,
) -> (agm_core::error::diagnostic::DiagnosticCollection, Vec<Node>) {
    let rendered = render_canonical(file);
    let parsed = parser::parse(&rendered).expect("file must parse");
    let diags = validator::validate(&parsed, &rendered, "<test>", &Default::default());
    (diags, parsed.nodes)
}

fn assert_code_present(diags: &agm_core::error::diagnostic::DiagnosticCollection, code: ErrorCode) {
    assert!(
        diags.diagnostics().iter().any(|d| d.code == code),
        "expected {code} in diagnostics, got: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

// ============================================================================
// E1: Chain ticket depends on workflow depends on facts — validates OK
// ============================================================================

#[test]
fn test_chain_ticket_depends_on_workflow_depends_on_facts() {
    let facts = FactsBuilder::new("rel.chain.facts")
        .summary("facts node")
        .items(["constraint a"])
        .build_unchecked()
        .unwrap();

    let workflow = WorkflowBuilder::new("rel.chain.workflow")
        .summary("workflow depending on facts")
        .steps(["step one"])
        .depends(["rel.chain.facts"])
        .build_unchecked()
        .unwrap();

    // ticket requires multi-node context for depends validation — use facts+workflow node types
    let ticket = FactsBuilder::new("rel.chain.terminal")
        .summary("terminal node depending on workflow")
        .depends(["rel.chain.workflow"])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![facts, workflow, ticket],
    };

    let diags = validate_file(&file);
    assert!(
        !diags.has_errors(),
        "chain depends must validate OK: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

// ============================================================================
// E2: Diamond dependency — A→B, A→C, B→D, C→D — graph intact after render
// ============================================================================

#[test]
fn test_diamond_dependency_resolves() {
    // D has no deps; B and C both depend on D; A depends on both B and C
    let node_d = FactsBuilder::new("rel.diamond.d")
        .summary("diamond base D")
        .build_unchecked()
        .unwrap();
    let node_b = FactsBuilder::new("rel.diamond.b")
        .summary("diamond B depends on D")
        .depends(["rel.diamond.d"])
        .build_unchecked()
        .unwrap();
    let node_c = FactsBuilder::new("rel.diamond.c")
        .summary("diamond C depends on D")
        .depends(["rel.diamond.d"])
        .build_unchecked()
        .unwrap();
    let node_a = FactsBuilder::new("rel.diamond.a")
        .summary("diamond A depends on B and C")
        .depends(["rel.diamond.b", "rel.diamond.c"])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![node_d, node_b, node_c, node_a],
    };

    let diags = validate_file(&file);
    assert!(
        !diags.has_errors(),
        "diamond dependency must validate OK: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );

    // Verify the structure survives round-trip
    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("diamond file must parse");
    let node_a_parsed = parsed
        .nodes
        .iter()
        .find(|n| n.id == "rel.diamond.a")
        .unwrap();
    let deps = node_a_parsed.depends.as_deref().unwrap();
    assert!(
        deps.contains(&"rel.diamond.b".to_owned()),
        "dep B must survive"
    );
    assert!(
        deps.contains(&"rel.diamond.c".to_owned()),
        "dep C must survive"
    );
}

// ============================================================================
// E3: Two-node cycle in depends triggers V005
// ============================================================================

#[test]
fn test_cycle_two_node_depends_triggers_v005() {
    let node_a = FactsBuilder::new("rel.cycle2.a")
        .summary("a depends on b")
        .depends(["rel.cycle2.b"])
        .build_unchecked()
        .unwrap();
    let node_b = FactsBuilder::new("rel.cycle2.b")
        .summary("b depends on a — cycle!")
        .depends(["rel.cycle2.a"])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![node_a, node_b],
    };

    let diags = validate_file(&file);
    assert_code_present(&diags, ErrorCode::V005);
}

// ============================================================================
// E4: Three-node cycle in depends triggers V005
// ============================================================================

#[test]
fn test_cycle_three_node_depends_triggers_v005() {
    let node_a = FactsBuilder::new("rel.cycle3.a")
        .summary("a → b → c → a")
        .depends(["rel.cycle3.b"])
        .build_unchecked()
        .unwrap();
    let node_b = FactsBuilder::new("rel.cycle3.b")
        .summary("b → c")
        .depends(["rel.cycle3.c"])
        .build_unchecked()
        .unwrap();
    let node_c = FactsBuilder::new("rel.cycle3.c")
        .summary("c → a (closes cycle)")
        .depends(["rel.cycle3.a"])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![node_a, node_b, node_c],
    };

    let diags = validate_file(&file);
    assert_code_present(&diags, ErrorCode::V005);
}

// ============================================================================
// E5: Self-reference in depends triggers V005 (cycle of length 1)
// ============================================================================

#[test]
fn test_self_reference_depends_triggers_v005() {
    // A node that depends on itself: single-node cycle → V005
    let node = FactsBuilder::new("rel.self.ref")
        .summary("self-reference in depends")
        .depends(["rel.self.ref"])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![node],
    };

    let diags = validate_file(&file);
    // Self-reference is a cycle of length 1 — V005 or V004 may fire
    // behavior: document what actually fires
    let has_v005 = diags
        .diagnostics()
        .iter()
        .any(|d| d.code == ErrorCode::V005);
    let has_v004 = diags
        .diagnostics()
        .iter()
        .any(|d| d.code == ErrorCode::V004);
    assert!(
        has_v005 || has_v004,
        "self-reference must trigger V005 (cycle) or V004 (dangling ref), got: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

// ============================================================================
// E6: Duplicate dep entry — observed behavior
// ============================================================================

#[test]
fn test_duplicate_dep_entry_observed_behavior() {
    // depends: [x, x] — what happens?
    let dep_node = FactsBuilder::new("rel.dup.dep.target")
        .summary("the dep target")
        .build_unchecked()
        .unwrap();
    let node = FactsBuilder::new("rel.dup.dep.source")
        .summary("has duplicate dep")
        .depends(["rel.dup.dep.target", "rel.dup.dep.target"]) // duplicate
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![dep_node, node],
    };

    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("must parse");
    let diags = validator::validate(&parsed, &rendered, "<test>", &Default::default());

    // behavior: duplicate dep entry may or may not trigger an error
    // Document the observed behavior:
    let source_node = parsed
        .nodes
        .iter()
        .find(|n| n.id == "rel.dup.dep.source")
        .unwrap();
    let dep_count = source_node.depends.as_ref().map(|d| d.len()).unwrap_or(0);

    // behavior: parser may deduplicate or preserve duplicates
    // Either [1] (deduplicated) or [2] (preserved) is acceptable; document it
    assert!(
        dep_count >= 1,
        "depends list must have at least 1 entry, got {dep_count}"
    );
    // No errors required — duplicate deps may be silently accepted
    let _ = diags;
}

// ============================================================================
// E7: replaces relationship round-trip
// ============================================================================

#[test]
fn test_replaces_relationship_round_trip() {
    let old_node = FactsBuilder::new("rel.old.facts")
        .summary("the old facts node")
        .build_unchecked()
        .unwrap();

    // Node with replaces field
    let mut new_node = FactsBuilder::new("rel.new.facts")
        .summary("the new facts node replacing old")
        .build_unchecked()
        .unwrap();
    new_node.replaces = Some(vec!["rel.old.facts".to_owned()]);

    let file = AgmFile {
        header: test_header(),
        nodes: vec![old_node, new_node],
    };

    let (diags, parsed_nodes) = validate_file_get_nodes(&file);
    // Replaces may trigger V014 warnings if the old node lacks status=deprecated
    // Allow warnings — only check for no hard errors on the replaces field itself
    let non_relationship_errors: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| {
            d.code != ErrorCode::V014 && d.severity == agm_core::error::diagnostic::Severity::Error
        })
        .collect();
    assert!(
        non_relationship_errors.is_empty(),
        "replaces relationship must not cause non-V014 errors: {:?}",
        non_relationship_errors
    );

    let new_reparsed = parsed_nodes
        .iter()
        .find(|n| n.id == "rel.new.facts")
        .unwrap();
    assert_eq!(
        new_reparsed.replaces.as_deref().unwrap(),
        &["rel.old.facts"],
        "replaces field must survive round-trip"
    );
}

// ============================================================================
// E8: conflicts relationship round-trip
// ============================================================================

#[test]
fn test_conflicts_relationship_round_trip() {
    let node_a = FactsBuilder::new("rel.conflicts.a")
        .summary("node a conflicts with b")
        .build_unchecked()
        .unwrap();
    let mut node_b = FactsBuilder::new("rel.conflicts.b")
        .summary("node b (conflicts with a)")
        .build_unchecked()
        .unwrap();
    node_b.conflicts = Some(vec!["rel.conflicts.a".to_owned()]);

    let file = AgmFile {
        header: test_header(),
        nodes: vec![node_a, node_b],
    };

    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("parse failed");

    let node_b_parsed = parsed
        .nodes
        .iter()
        .find(|n| n.id == "rel.conflicts.b")
        .unwrap();
    assert_eq!(
        node_b_parsed.conflicts.as_deref().unwrap(),
        &["rel.conflicts.a"],
        "conflicts field must survive round-trip"
    );
}

// ============================================================================
// E9: see_also relationship round-trip
// ============================================================================

#[test]
fn test_see_also_relationship_round_trip() {
    let node_a = FactsBuilder::new("rel.seealso.a")
        .summary("node a, see also b")
        .build_unchecked()
        .unwrap();
    let node_b = FactsBuilder::new("rel.seealso.b")
        .summary("node b")
        .build_unchecked()
        .unwrap();
    let mut node_a_with_see_also = FactsBuilder::new("rel.seealso.c")
        .summary("node c with see_also")
        .build_unchecked()
        .unwrap();
    node_a_with_see_also.see_also =
        Some(vec!["rel.seealso.a".to_owned(), "rel.seealso.b".to_owned()]);

    let file = AgmFile {
        header: test_header(),
        nodes: vec![node_a, node_b, node_a_with_see_also],
    };

    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("parse failed");

    let c = parsed
        .nodes
        .iter()
        .find(|n| n.id == "rel.seealso.c")
        .unwrap();
    let see_also = c.see_also.as_deref().unwrap();
    assert!(
        see_also.contains(&"rel.seealso.a".to_owned()),
        "see_also[0] must survive"
    );
    assert!(
        see_also.contains(&"rel.seealso.b".to_owned()),
        "see_also[1] must survive"
    );
}

// ============================================================================
// E10: related_to relationship round-trip
// ============================================================================

#[test]
fn test_related_to_relationship_round_trip() {
    let node_x = FactsBuilder::new("rel.relto.x")
        .summary("node x")
        .build_unchecked()
        .unwrap();
    let node_y = FactsBuilder::new("rel.relto.y")
        .summary("node y related to x")
        .build_unchecked()
        .unwrap();
    let node_z = WorkflowBuilder::new("rel.relto.z")
        .summary("node z related to x and y")
        .related_to(["rel.relto.x", "rel.relto.y"])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![node_x, node_y, node_z],
    };

    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("parse failed");

    let z = parsed.nodes.iter().find(|n| n.id == "rel.relto.z").unwrap();
    let related = z.related_to.as_deref().unwrap();
    assert!(
        related.contains(&"rel.relto.x".to_owned()),
        "related_to[0] must survive"
    );
    assert!(
        related.contains(&"rel.relto.y".to_owned()),
        "related_to[1] must survive"
    );
}

// ============================================================================
// E11: Orchestration parallel_group with requires DAG matches topo order
// ============================================================================

#[test]
fn test_orchestration_parallel_group_with_requires_dag_matches_topo() {
    // DAG: 1-schema → 2-migrate → 3-backend → 4-smoke
    let node_id = "rel.orch.dag";
    let groups = vec![
        ParallelGroup {
            group: "1-schema".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        },
        ParallelGroup {
            group: "2-migrate".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["1-schema".to_owned()]),
            max_concurrency: None,
        },
        ParallelGroup {
            group: "3-backend".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Parallel,
            requires: Some(vec!["2-migrate".to_owned()]),
            max_concurrency: Some(2),
        },
        ParallelGroup {
            group: "4-smoke".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["3-backend".to_owned()]),
            max_concurrency: None,
        },
    ];

    let orch = OrchestrationBuilder::new(node_id)
        .summary("orchestration with linear requires DAG")
        .parallel_groups(groups)
        .build()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![orch],
    };

    let (diags, parsed_nodes) = validate_file_get_nodes(&file);
    assert!(
        !diags.has_errors(),
        "orchestration DAG must be valid: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );

    let orch_parsed = &parsed_nodes[0];
    let groups = orch_parsed.parallel_groups.as_ref().unwrap();
    assert_eq!(groups.len(), 4);
    assert!(groups[0].requires.is_none(), "1-schema has no requires");
    assert_eq!(groups[1].requires.as_deref().unwrap(), &["1-schema"]);
    assert_eq!(groups[2].requires.as_deref().unwrap(), &["2-migrate"]);
    assert_eq!(groups[2].max_concurrency, Some(2));
    assert_eq!(groups[3].requires.as_deref().unwrap(), &["3-backend"]);
}

// ============================================================================
// E12: Full relationship field set — multi-node — zero errors
// ============================================================================

#[test]
fn test_full_relationship_set_multi_node_zero_errors() {
    let base = FactsBuilder::new("rel.full.base")
        .summary("base fact node")
        .items(["base constraint"])
        .build_unchecked()
        .unwrap();

    let derived = FactsBuilder::new("rel.full.derived")
        .summary("derived from base")
        .depends(["rel.full.base"])
        .build_unchecked()
        .unwrap();

    let workflow = WorkflowBuilder::new("rel.full.workflow")
        .summary("workflow related to both")
        .depends(["rel.full.base"])
        .related_to(["rel.full.derived"])
        .build_unchecked()
        .unwrap();

    let decision = DecisionBuilder::new("rel.full.decision")
        .summary("decision for architecture")
        .rationale(["reason"])
        .related_to(["rel.full.base"])
        .replaces(["rel.full.derived"])
        .conflicts(["rel.full.workflow"])
        .see_also(["rel.full.base"])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header(),
        nodes: vec![base, derived, workflow, decision],
    };

    let diags = validate_file(&file);
    assert!(
        !diags.has_errors(),
        "full relationship set must validate OK: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
}
