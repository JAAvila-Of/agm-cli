//! Group J — Node::render_canonical / render_node_only.
//!
//! Tests the two rendering methods exposed on Node: render_canonical (includes
//! scratch header) and render_node_only (node block only, no header).

use agm_core::builder::{
    CodeBlockBuilder, DecisionBuilder, FactsBuilder, OrchestrationBuilder, RulesBuilder,
    TicketBuilder, VerifyCheckBuilder, WorkflowBuilder,
};
use agm_core::model::fields::{NodeType, Priority};
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::parser;
use agm_core::validator;

// ---------------------------------------------------------------------------
// Constants matching the scratch header in common.rs
// ---------------------------------------------------------------------------
const SCRATCH_AGM: &str = "agm: 1.0";
const SCRATCH_PACKAGE: &str = "package: scratch.builder";
const SCRATCH_VERSION: &str = "version: 0.1.0";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minimal_facts() -> agm_core::model::node::Node {
    FactsBuilder::new("render.facts.min")
        .summary("render test facts")
        .items(["item one"])
        .build()
        .unwrap()
}

fn minimal_ticket() -> agm_core::model::node::Node {
    TicketBuilder::new("render.ticket.min")
        .summary("render test ticket")
        .title("Render Test")
        .description("desc")
        .priority(Priority::Normal)
        .build()
        .unwrap()
}

fn minimal_orchestration() -> agm_core::model::node::Node {
    let group = ParallelGroup {
        group: "g1".to_owned(),
        nodes: vec!["render.orch.min".to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    };
    OrchestrationBuilder::new("render.orch.min")
        .summary("render test orchestration")
        .parallel_groups([group])
        .build()
        .unwrap()
}

// ============================================================================
// J1 — render_canonical contains scratch header
// ============================================================================

#[test]
fn test_render_canonical_contains_scratch_header() {
    let node = minimal_facts();
    let text = node.render_canonical();

    assert!(
        text.contains(SCRATCH_AGM),
        "render_canonical must contain 'agm: 1.0', got:\n{text}"
    );
    assert!(
        text.contains(SCRATCH_PACKAGE),
        "render_canonical must contain 'package: scratch.builder'"
    );
    assert!(
        text.contains(SCRATCH_VERSION),
        "render_canonical must contain 'version: 0.1.0'"
    );
}

#[test]
fn test_render_canonical_contains_node_block() {
    let node = minimal_facts();
    let text = node.render_canonical();

    assert!(
        text.contains("node render.facts.min"),
        "render_canonical must contain 'node render.facts.min'"
    );
    assert!(
        text.contains("type: facts"),
        "render_canonical must contain 'type: facts'"
    );
    assert!(
        text.contains("summary:"),
        "render_canonical must contain 'summary:'"
    );
}

// ============================================================================
// J2 — render_node_only strips header
// ============================================================================

#[test]
fn test_render_node_only_strips_header() {
    let node = minimal_facts();
    let text = node.render_node_only();

    assert!(
        !text.contains("agm:"),
        "render_node_only must not contain 'agm:'"
    );
    assert!(
        !text.contains("package:"),
        "render_node_only must not contain 'package:'"
    );
    assert!(
        !text.contains("version:"),
        "render_node_only must not contain 'version:'"
    );
}

#[test]
fn test_render_node_only_starts_with_node_keyword() {
    let node = minimal_facts();
    let text = node.render_node_only();

    assert!(
        text.trim().starts_with("node "),
        "render_node_only must start with 'node ', got:\n{text}"
    );
}

// ============================================================================
// J3 — render_node_only byte-equals render_canonical minus header lines
// ============================================================================

#[test]
fn test_render_node_only_byte_equals_render_canonical_minus_header_lines() {
    let node = minimal_facts();
    let canonical = node.render_canonical();
    let node_only = node.render_node_only();

    // Extract the node portion from canonical: skip until line starting with "node "
    let canonical_node_portion: String = canonical
        .lines()
        .skip_while(|line| !line.starts_with("node "))
        .map(|line| format!("{line}\n"))
        .collect();

    // The canonical node portion must match render_node_only
    // (allow for a potential trailing newline difference)
    assert_eq!(
        canonical_node_portion.trim_end(),
        node_only.trim_end(),
        "canonical minus header must equal render_node_only"
    );
}

// ============================================================================
// J4 — render_canonical on every node type
// ============================================================================

#[test]
fn test_render_canonical_on_facts() {
    let node = minimal_facts();
    let text = node.render_canonical();
    assert!(text.contains("type: facts"));
    assert!(text.contains("node render.facts.min"));
}

#[test]
fn test_render_canonical_on_rules() {
    let node = RulesBuilder::new("render.rules.min")
        .summary("rules render test")
        .items(["require HTTPS"])
        .build()
        .unwrap();
    let text = node.render_canonical();
    assert!(text.contains("type: rules"));
    assert!(text.contains("node render.rules.min"));
}

#[test]
fn test_render_canonical_on_workflow() {
    let node = WorkflowBuilder::new("render.workflow.min")
        .summary("workflow render test")
        .steps(["step one"])
        .build()
        .unwrap();
    let text = node.render_canonical();
    assert!(text.contains("type: workflow"));
    assert!(text.contains("node render.workflow.min"));
}

#[test]
fn test_render_canonical_on_decision() {
    let node = DecisionBuilder::new("render.decision.min")
        .summary("decision render test")
        .rationale(["ACID required"])
        .build()
        .unwrap();
    let text = node.render_canonical();
    assert!(text.contains("type: decision"));
    assert!(text.contains("node render.decision.min"));
}

#[test]
fn test_render_canonical_on_ticket() {
    let node = minimal_ticket();
    let text = node.render_canonical();
    assert!(text.contains("type: ticket"));
    assert!(text.contains("node render.ticket.min"));
    assert!(text.contains("title:"));
    assert!(text.contains("priority:"));
}

#[test]
fn test_render_canonical_on_orchestration() {
    let node = minimal_orchestration();
    let text = node.render_canonical();
    assert!(text.contains("type: orchestration"));
    assert!(text.contains("node render.orch.min"));
    assert!(text.contains("parallel_groups:"));
}

// ============================================================================
// J5 — render_canonical is idempotent when re-parsed as AgmFile
// ============================================================================

#[test]
fn test_render_canonical_is_idempotent_when_reparsed_as_agm_file() {
    let node = FactsBuilder::new("render.idem.facts")
        .summary("idempotence test")
        .items(["item one", "item two"])
        .detail("some detail text")
        .build()
        .unwrap();

    // Round 1
    let r1 = node.render_canonical();
    let file1 = parser::parse(&r1).expect("r1 must parse");
    let diags1 = validator::validate(&file1, &r1, "<test>", &Default::default());
    assert!(!diags1.has_errors(), "r1 must validate without errors");

    // Round 2 (from reparsed node)
    let r2 = file1.nodes[0].render_canonical();
    let file2 = parser::parse(&r2).expect("r2 must parse");
    let diags2 = validator::validate(&file2, &r2, "<test>", &Default::default());
    assert!(!diags2.has_errors(), "r2 must validate without errors");

    // Canonical form must be stable
    assert_eq!(r1, r2, "render must be idempotent: r1 != r2");
}

#[test]
fn test_render_canonical_idempotent_workflow_with_code_blocks() {
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/lib.rs")
        .body("pub fn hello() -> &'static str { \"hello\" }")
        .build()
        .unwrap();
    let check = VerifyCheckBuilder::command("cargo test")
        .expect("exit_code_0")
        .build()
        .unwrap();

    let node = WorkflowBuilder::new("render.idem.wf")
        .summary("idempotence workflow test")
        .steps(["step one", "step two"])
        .code_blocks([cb])
        .verify([check])
        .build()
        .unwrap();

    let r1 = node.render_canonical();
    let file1 = parser::parse(&r1).expect("r1 must parse");
    let r2 = file1.nodes[0].render_canonical();
    assert_eq!(r1, r2, "workflow render must be idempotent");
}

#[test]
fn test_render_canonical_idempotent_ticket() {
    let node = minimal_ticket();
    let r1 = node.render_canonical();
    let file1 = parser::parse(&r1).expect("r1 must parse");
    let r2 = file1.nodes[0].render_canonical();
    assert_eq!(r1, r2, "ticket render must be idempotent");
}

#[test]
fn test_render_canonical_idempotent_orchestration() {
    let node = minimal_orchestration();
    let r1 = node.render_canonical();
    let file1 = parser::parse(&r1).expect("r1 must parse");
    let r2 = file1.nodes[0].render_canonical();
    assert_eq!(r1, r2, "orchestration render must be idempotent");
}

// ============================================================================
// J6 — render_node_only produces parseable AGM when wrapped in a header
// ============================================================================

#[test]
fn test_render_node_only_can_be_reparsed_when_header_prepended() {
    let node = minimal_facts();
    let node_only = node.render_node_only();

    // Wrap in a minimal header to make it parseable
    let full = format!(
        "agm: 1.0\npackage: reparse.test\nversion: 0.1.0\n\n{}",
        node_only
    );

    let file = parser::parse(&full).expect("node_only with prepended header must parse");
    assert_eq!(file.nodes.len(), 1);
    assert_eq!(file.nodes[0].id, "render.facts.min");
    assert_eq!(file.nodes[0].node_type, NodeType::Facts);
}

// ============================================================================
// J7 — render_node_only preserves all field content
// ============================================================================

#[test]
fn test_render_node_only_preserves_all_field_content() {
    let node = TicketBuilder::new("render.fields.ticket")
        .summary("field content test")
        .title("Field Content Test")
        .description("Complete description with all fields.")
        .priority(Priority::High)
        .labels(["auth", "backend"])
        .build()
        .unwrap();

    let node_only = node.render_node_only();

    assert!(
        node_only.contains("node render.fields.ticket"),
        "ID must be present"
    );
    assert!(node_only.contains("type: ticket"), "type must be present");
    assert!(
        node_only.contains("Field Content Test"),
        "title must be present"
    );
    assert!(node_only.contains("high"), "priority must be present");
    assert!(node_only.contains("auth"), "first label must be present");
    assert!(
        node_only.contains("backend"),
        "second label must be present"
    );
}
