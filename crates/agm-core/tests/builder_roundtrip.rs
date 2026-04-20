//! Group C — Render ↔ parse round-trip fidelity.
//!
//! For each node type:
//! - minimal round-trip (build → render → parse → re-render → assert byte-equal + field equality)
//! - maximal round-trip (every optional field set)
//! - multinode file with one of each type preserves order
//! - triple-render idempotence (render 3×, rounds 2 and 3 equal round 1)

use agm_core::builder::{
    CodeBlockBuilder, DecisionBuilder, FactsBuilder, OrchestrationBuilder, RulesBuilder,
    TicketBuilder, VerifyCheckBuilder, WorkflowBuilder,
};
use agm_core::model::context::{AgentContext, FileRange, LoadFile};
use agm_core::model::fields::{NodeType, Priority, SddPhase, Stability, TicketAction};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::parser;
use agm_core::renderer::canonical::render_canonical;
use agm_core::validator;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_header(package: &str) -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: package.to_owned(),
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

fn parse_and_validate(text: &str) -> AgmFile {
    let file = parser::parse(text).expect("re-parse failed");
    let diags = validator::validate(&file, text, "<test>", &Default::default());
    assert!(
        !diags.has_errors(),
        "re-parsed file has errors: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
    file
}

/// Render a single Node, re-parse it, return the first node.
fn roundtrip_node(node: &Node) -> Node {
    let text = node.render_canonical();
    let file = parse_and_validate(&text);
    assert_eq!(file.nodes.len(), 1);
    file.nodes.into_iter().next().unwrap()
}

/// Assert that rendering the node three times produces identical output on rounds 2 and 3.
fn assert_triple_render_idempotent(node: &Node) {
    let r1 = node.render_canonical();
    let file2 = parser::parse(&r1).expect("parse r1 failed");
    let r2 = file2.nodes[0].render_canonical();
    let file3 = parser::parse(&r2).expect("parse r2 failed");
    let r3 = file3.nodes[0].render_canonical();

    assert_eq!(r1, r2, "render round 2 must equal round 1");
    assert_eq!(r2, r3, "render round 3 must equal round 2");
}

// ============================================================================
// Facts
// ============================================================================

#[test]
fn test_roundtrip_facts_minimal() {
    let node = FactsBuilder::new("rt.facts.min")
        .summary("minimal facts node")
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.node_type, NodeType::Facts);
    assert_eq!(reparsed.summary, node.summary);

    // Re-render must be byte-equal
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

#[test]
fn test_roundtrip_facts_maximal() {
    let node = FactsBuilder::new("rt.facts.max")
        .summary("maximal facts node")
        .items(["item one", "item two", "item three"])
        .detail("detailed description of these facts")
        .stability(Stability::High)
        .notes("see RFC-42")
        .tags(["auth", "security", "policy"])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.summary, node.summary);
    assert_eq!(reparsed.items.as_deref().unwrap().len(), 3);
    assert!(reparsed.detail.is_some());
    assert_eq!(reparsed.stability, Some(Stability::High));
    assert!(reparsed.notes.is_some());
    assert_eq!(reparsed.tags.as_deref().unwrap().len(), 3);

    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

// ============================================================================
// Rules
// ============================================================================

#[test]
fn test_roundtrip_rules_minimal() {
    let node = RulesBuilder::new("rt.rules.min")
        .summary("minimal rules node")
        .items(["require HTTPS"])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.node_type, NodeType::Rules);
    assert_eq!(reparsed.items.as_deref().unwrap().len(), 1);
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

#[test]
fn test_roundtrip_rules_maximal() {
    let node = RulesBuilder::new("rt.rules.max")
        .summary("maximal rules node")
        .items(["require HTTPS", "rate-limit login", "validate PKCE"])
        .detail("enforcement context")
        .stability(Stability::Medium)
        .notes("quarterly review")
        .tags(["security", "auth"])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.items.as_deref().unwrap().len(), 3);
    assert!(reparsed.detail.is_some());
    assert!(reparsed.notes.is_some());
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

// ============================================================================
// Workflow
// ============================================================================

#[test]
fn test_roundtrip_workflow_minimal() {
    let node = WorkflowBuilder::new("rt.workflow.min")
        .summary("minimal workflow")
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.node_type, NodeType::Workflow);
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

#[test]
fn test_roundtrip_workflow_maximal() {
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/auth.rs")
        .body("pub fn authenticate() -> bool { true }")
        .build()
        .unwrap();

    let check = VerifyCheckBuilder::command("cargo test")
        .expect("exit_code_0")
        .build()
        .unwrap();

    let ctx = AgentContext {
        load_nodes: None,
        load_files: Some(vec![LoadFile {
            path: "src/auth.rs".to_owned(),
            range: FileRange::Full,
        }]),
        system_hint: Some("focus on auth".to_owned()),
        max_tokens: Some(4000),
        load_memory: None,
    };

    let node = WorkflowBuilder::new("rt.workflow.max")
        .summary("maximal workflow")
        .steps(["step one", "step two", "step three"])
        .input(["host", "return_url"])
        .output(["sid_cookie", "user_id"])
        .code_blocks([cb])
        .verify([check])
        .agent_context(ctx)
        .target("src/handlers.rs")
        .priority(Priority::High)
        .stability(Stability::Medium)
        .detail("workflow implementation notes")
        .notes("see RFC-6749")
        .tags(["auth", "oauth2"])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.steps.as_deref().unwrap().len(), 3);
    assert_eq!(reparsed.input.as_deref().unwrap().len(), 2);
    assert_eq!(reparsed.output.as_deref().unwrap().len(), 2);
    assert_eq!(reparsed.code_blocks.as_ref().unwrap().len(), 1);
    assert_eq!(reparsed.verify.as_ref().unwrap().len(), 1);
    assert!(reparsed.agent_context.is_some());
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

// ============================================================================
// Decision
// ============================================================================

#[test]
fn test_roundtrip_decision_minimal() {
    let node = DecisionBuilder::new("rt.decision.min")
        .summary("chose PostgreSQL")
        .rationale(["ACID guarantees required"])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.node_type, NodeType::Decision);
    assert_eq!(reparsed.rationale.as_deref().unwrap().len(), 1);
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

#[test]
fn test_roundtrip_decision_maximal() {
    let node = DecisionBuilder::new("rt.decision.max")
        .summary("maximal decision node")
        .rationale(["ACID required", "existing expertise", "pgvector support"])
        .tradeoffs(["complex scaling", "schema migrations"])
        .resolution(["use PostgreSQL 16", "enable pgvector"])
        .detail("background context on this decision")
        .stability(Stability::High)
        .notes("revisit in Q3")
        .tags(["architecture", "database"])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.rationale.as_deref().unwrap().len(), 3);
    assert_eq!(reparsed.tradeoffs.as_deref().unwrap().len(), 2);
    assert_eq!(reparsed.resolution.as_deref().unwrap().len(), 2);
    assert!(reparsed.detail.is_some());
    assert!(reparsed.notes.is_some());
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

// ============================================================================
// Orchestration
// ============================================================================

#[test]
fn test_roundtrip_orchestration_minimal() {
    let group = ParallelGroup {
        group: "g1".to_owned(),
        nodes: vec!["rt.orch.min".to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    };
    let node = OrchestrationBuilder::new("rt.orch.min")
        .summary("minimal orchestration")
        .parallel_groups([group])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.node_type, NodeType::Orchestration);
    assert_eq!(reparsed.parallel_groups.as_ref().unwrap().len(), 1);
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

#[test]
fn test_roundtrip_orchestration_maximal() {
    let g1 = ParallelGroup {
        group: "1-schema".to_owned(),
        nodes: vec!["rt.orch.max".to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    };
    let g2 = ParallelGroup {
        group: "2-backend".to_owned(),
        nodes: vec!["rt.orch.max".to_owned()],
        strategy: Strategy::Parallel,
        requires: Some(vec!["1-schema".to_owned()]),
        max_concurrency: Some(3),
    };
    let node = OrchestrationBuilder::new("rt.orch.max")
        .summary("maximal orchestration")
        .parallel_groups([g1, g2])
        .detail("run in CI only")
        .notes("requires database env vars")
        .tags(["deploy", "ci"])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.parallel_groups.as_ref().unwrap().len(), 2);
    let g2_reparsed = &reparsed.parallel_groups.as_ref().unwrap()[1];
    assert_eq!(g2_reparsed.requires.as_deref().unwrap(), &["1-schema"]);
    assert_eq!(g2_reparsed.max_concurrency, Some(3));
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

// ============================================================================
// Ticket
// ============================================================================

#[test]
fn test_roundtrip_ticket_minimal() {
    let node = TicketBuilder::new("rt.ticket.min")
        .summary("minimal ticket")
        .title("Minimal Ticket")
        .description("Minimal description.")
        .priority(Priority::Normal)
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.node_type, NodeType::Ticket);
    assert_eq!(reparsed.title.as_deref(), Some("Minimal Ticket"));
    assert_eq!(reparsed.priority, Some(Priority::Normal));
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

#[test]
fn test_roundtrip_ticket_maximal() {
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/auth.rs")
        .body("pub fn auth() {}")
        .build()
        .unwrap();

    let node = TicketBuilder::new("rt.ticket.max")
        .summary("maximal ticket")
        .title("Maximal Ticket")
        .description("Full description of the ticket.")
        .priority(Priority::High)
        .action(TicketAction::Create)
        .sdd_phase(SddPhase::Apply)
        .labels(["auth", "backend", "security"])
        .prompt("Implement auth module.")
        .assignee("agent-01")
        .ticket_id("GH-42")
        .detail("additional implementation notes")
        .stability(Stability::Medium)
        .notes("see RFC-6749")
        .tags(["rust", "oauth2"])
        .code_blocks([cb])
        .build()
        .unwrap();

    let reparsed = roundtrip_node(&node);
    assert_eq!(reparsed.title.as_deref(), Some("Maximal Ticket"));
    assert_eq!(reparsed.priority, Some(Priority::High));
    assert_eq!(reparsed.action, Some(TicketAction::Create));
    assert_eq!(reparsed.sdd_phase, Some(SddPhase::Apply));
    assert_eq!(reparsed.labels.as_deref().unwrap().len(), 3);
    assert_eq!(reparsed.assignee.as_deref(), Some("agent-01"));
    assert_eq!(reparsed.ticket_id.as_deref(), Some("GH-42"));
    assert_eq!(reparsed.stability, Some(Stability::Medium));
    assert_eq!(reparsed.code_blocks.as_ref().unwrap().len(), 1);
    assert_eq!(node.render_canonical(), reparsed.render_canonical());
}

// ============================================================================
// Multinode file — all types, ordering preserved
// ============================================================================

#[test]
fn test_roundtrip_multinode_file_all_types_preserves_order() {
    let facts = FactsBuilder::new("order.facts.a")
        .summary("facts node")
        .items(["item"])
        .build_unchecked()
        .unwrap();

    let rules = RulesBuilder::new("order.rules.b")
        .summary("rules node")
        .items(["rule"])
        .build_unchecked()
        .unwrap();

    let workflow = WorkflowBuilder::new("order.workflow.c")
        .summary("workflow node")
        .steps(["step"])
        .build_unchecked()
        .unwrap();

    let decision = DecisionBuilder::new("order.decision.d")
        .summary("decision node")
        .rationale(["reason"])
        .build_unchecked()
        .unwrap();

    let ticket = TicketBuilder::new("order.ticket.e")
        .summary("ticket node")
        .title("Ticket E")
        .description("desc")
        .priority(Priority::Normal)
        .build_unchecked()
        .unwrap();

    let orch_group = ParallelGroup {
        group: "g1".to_owned(),
        nodes: vec!["order.orch.f".to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    };
    let orch = OrchestrationBuilder::new("order.orch.f")
        .summary("orchestration node")
        .parallel_groups([orch_group])
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header("order.test"),
        nodes: vec![facts, rules, workflow, decision, ticket, orch],
    };

    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("multinode must parse");

    assert_eq!(parsed.nodes.len(), 6);
    // Order preserved
    assert_eq!(parsed.nodes[0].id, "order.facts.a");
    assert_eq!(parsed.nodes[1].id, "order.rules.b");
    assert_eq!(parsed.nodes[2].id, "order.workflow.c");
    assert_eq!(parsed.nodes[3].id, "order.decision.d");
    assert_eq!(parsed.nodes[4].id, "order.ticket.e");
    assert_eq!(parsed.nodes[5].id, "order.orch.f");

    // Types preserved
    assert_eq!(parsed.nodes[0].node_type, NodeType::Facts);
    assert_eq!(parsed.nodes[1].node_type, NodeType::Rules);
    assert_eq!(parsed.nodes[2].node_type, NodeType::Workflow);
    assert_eq!(parsed.nodes[3].node_type, NodeType::Decision);
    assert_eq!(parsed.nodes[4].node_type, NodeType::Ticket);
    assert_eq!(parsed.nodes[5].node_type, NodeType::Orchestration);
}

// ============================================================================
// Triple-render idempotence
// ============================================================================

#[test]
fn test_roundtrip_idempotent_after_three_renders_facts() {
    let node = FactsBuilder::new("triple.facts.a")
        .summary("triple render facts")
        .items(["item one", "item two"])
        .detail("some detail")
        .build()
        .unwrap();
    assert_triple_render_idempotent(&node);
}

#[test]
fn test_roundtrip_idempotent_after_three_renders_workflow() {
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/lib.rs")
        .body("fn x() {}")
        .build()
        .unwrap();
    let node = WorkflowBuilder::new("triple.wf.a")
        .summary("triple render workflow")
        .steps(["s1", "s2"])
        .input(["i1"])
        .output(["o1"])
        .code_blocks([cb])
        .build()
        .unwrap();
    assert_triple_render_idempotent(&node);
}

#[test]
fn test_roundtrip_idempotent_after_three_renders_ticket() {
    let node = TicketBuilder::new("triple.ticket.a")
        .summary("triple render ticket")
        .title("Triple Ticket")
        .description("desc")
        .priority(Priority::High)
        .action(TicketAction::Create)
        .labels(["a", "b"])
        .build()
        .unwrap();
    assert_triple_render_idempotent(&node);
}

#[test]
fn test_roundtrip_idempotent_after_three_renders_orchestration() {
    let group = ParallelGroup {
        group: "g1".to_owned(),
        nodes: vec!["triple.orch.a".to_owned()],
        strategy: Strategy::Parallel,
        requires: None,
        max_concurrency: Some(2),
    };
    let node = OrchestrationBuilder::new("triple.orch.a")
        .summary("triple render orchestration")
        .parallel_groups([group])
        .detail("notes")
        .build()
        .unwrap();
    assert_triple_render_idempotent(&node);
}

#[test]
fn test_roundtrip_idempotent_after_three_renders_decision() {
    let node = DecisionBuilder::new("triple.decision.a")
        .summary("triple render decision")
        .rationale(["r1", "r2"])
        .tradeoffs(["t1"])
        .resolution(["resolve this way"])
        .build()
        .unwrap();
    assert_triple_render_idempotent(&node);
}
