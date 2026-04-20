//! Group A — Setter coverage matrix.
//!
//! For each of the 9 builders verifies:
//! 1. Every public setter is applied and survives to the built value.
//! 2. All setters can be chained in a single fluent expression (compile = pass).
//! 3. Iterator-accepting setters work with Vec, arrays, iter().cloned(),
//!    and std::iter::once().

use agm_core::builder::{
    CodeBlockBuilder, DecisionBuilder, FactsBuilder, MemoryEntryBuilder, OrchestrationBuilder,
    RulesBuilder, TicketBuilder, VerifyCheckBuilder, WorkflowBuilder,
};
use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::context::{AgentContext, FileRange, LoadFile};
use agm_core::model::fields::{NodeType, Priority, SddPhase, Stability, TicketAction};
use agm_core::model::memory::{MemoryAction, MemoryScope, MemoryTtl};
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::verify::VerifyCheck;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minimal_code_block() -> CodeBlock {
    CodeBlockBuilder::create()
        .lang("rust")
        .target("src/lib.rs")
        .body("fn hello() {}")
        .build()
        .unwrap()
}

fn minimal_verify_check() -> VerifyCheck {
    VerifyCheckBuilder::command("cargo test").build().unwrap()
}

fn minimal_agent_ctx() -> AgentContext {
    AgentContext {
        load_nodes: None,
        load_files: Some(vec![LoadFile {
            path: "src/lib.rs".to_owned(),
            range: FileRange::Full,
        }]),
        system_hint: Some("hint".to_owned()),
        max_tokens: Some(4000),
        load_memory: None,
    }
}

fn minimal_group(id: &str) -> ParallelGroup {
    ParallelGroup {
        group: "g1".to_owned(),
        nodes: vec![id.to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    }
}

// ============================================================================
// TicketBuilder
// ============================================================================

#[test]
fn test_ticket_builder_every_setter_is_applied() {
    let cb = minimal_code_block();
    let ctx = minimal_agent_ctx();

    let node = TicketBuilder::new("set.ticket.all")
        .summary("every setter test")
        .title("Every Setter")
        .description("Full description here.")
        .priority(Priority::High)
        .action(TicketAction::Create)
        .sdd_phase(SddPhase::Apply)
        .labels(["auth", "backend"])
        .prompt("Do the thing.")
        .assignee("agent-01")
        .ticket_id("GH-99")
        .detail("Extra detail.")
        .stability(Stability::High)
        .notes("Review needed.")
        .tags(["rust", "api"])
        .agent_context(ctx)
        .code_blocks([cb])
        // depends/related_to cross-references require multi-node; use build_unchecked
        .depends(["set.ticket.dep"])
        .related_to(["set.ticket.rel"])
        .build_unchecked()
        .unwrap();

    assert_eq!(node.id, "set.ticket.all");
    assert_eq!(node.node_type, NodeType::Ticket);
    assert_eq!(node.summary, "every setter test");
    assert_eq!(node.title.as_deref(), Some("Every Setter"));
    assert_eq!(node.description.as_deref(), Some("Full description here."));
    assert_eq!(node.priority, Some(Priority::High));
    assert_eq!(node.action, Some(TicketAction::Create));
    assert_eq!(node.sdd_phase, Some(SddPhase::Apply));
    assert_eq!(node.labels.as_deref().unwrap(), &["auth", "backend"]);
    assert_eq!(node.prompt.as_deref(), Some("Do the thing."));
    assert_eq!(node.assignee.as_deref(), Some("agent-01"));
    assert_eq!(node.ticket_id.as_deref(), Some("GH-99"));
    assert_eq!(node.detail.as_deref(), Some("Extra detail."));
    assert_eq!(node.stability, Some(Stability::High));
    assert_eq!(node.notes.as_deref(), Some("Review needed."));
    assert_eq!(node.tags.as_deref().unwrap(), &["rust", "api"]);
    assert!(node.agent_context.is_some());
    assert_eq!(node.code_blocks.as_ref().unwrap().len(), 1);
    assert_eq!(node.depends.as_deref().unwrap(), &["set.ticket.dep"]);
    assert_eq!(node.related_to.as_deref().unwrap(), &["set.ticket.rel"]);
}

#[test]
fn test_ticket_builder_setter_returns_self_for_chaining() {
    // Compile-only: if setters don't return Self, this won't compile.
    let _ = TicketBuilder::new("chain.ticket.x")
        .summary("s")
        .title("T")
        .description("d")
        .priority(Priority::Normal)
        .action(TicketAction::Create)
        .sdd_phase(SddPhase::Verify)
        .labels(["a"])
        .prompt("p")
        .assignee("ag")
        .ticket_id("GH-1")
        .detail("dt")
        .stability(Stability::Medium)
        .notes("n")
        .tags(["t"])
        .build_unchecked()
        .unwrap();
}

#[test]
fn test_ticket_builder_setter_accepts_iterator_and_vec_for_labels() {
    let vec_labels: Vec<String> = vec!["auth".to_owned(), "security".to_owned()];
    let n1 = TicketBuilder::new("set.ticket.l1")
        .summary("s")
        .title("T")
        .description("d")
        .priority(Priority::Normal)
        .labels(vec_labels.clone())
        .build_unchecked()
        .unwrap();
    assert_eq!(n1.labels.as_deref().unwrap().len(), 2);

    let n2 = TicketBuilder::new("set.ticket.l2")
        .summary("s")
        .title("T")
        .description("d")
        .priority(Priority::Normal)
        .labels(["x", "y", "z"])
        .build_unchecked()
        .unwrap();
    assert_eq!(n2.labels.as_deref().unwrap().len(), 3);

    let n3 = TicketBuilder::new("set.ticket.l3")
        .summary("s")
        .title("T")
        .description("d")
        .priority(Priority::Normal)
        .labels(vec_labels.iter().cloned())
        .build_unchecked()
        .unwrap();
    assert_eq!(n3.labels.as_deref().unwrap().len(), 2);

    let n4 = TicketBuilder::new("set.ticket.l4")
        .summary("s")
        .title("T")
        .description("d")
        .priority(Priority::Normal)
        .labels(std::iter::once("solo"))
        .build_unchecked()
        .unwrap();
    assert_eq!(n4.labels.as_deref().unwrap().len(), 1);
}

#[test]
fn test_ticket_builder_setter_accepts_iterator_and_vec_for_depends() {
    let deps: Vec<String> = vec!["dep.a".to_owned(), "dep.b".to_owned()];
    let n1 = TicketBuilder::new("set.ticket.d1")
        .summary("s")
        .build_unchecked()
        .unwrap();
    // Test with no deps first — just checks unchecked builds fine
    assert!(n1.depends.is_none());

    let n2 = TicketBuilder::new("set.ticket.d2")
        .summary("s")
        .depends(deps.clone())
        .build_unchecked()
        .unwrap();
    assert_eq!(n2.depends.as_deref().unwrap().len(), 2);

    let n3 = TicketBuilder::new("set.ticket.d3")
        .summary("s")
        .depends(["x", "y"])
        .build_unchecked()
        .unwrap();
    assert_eq!(n3.depends.as_deref().unwrap().len(), 2);

    let n4 = TicketBuilder::new("set.ticket.d4")
        .summary("s")
        .depends(deps.iter().cloned())
        .build_unchecked()
        .unwrap();
    assert_eq!(n4.depends.as_deref().unwrap().len(), 2);
}

// ============================================================================
// WorkflowBuilder
// ============================================================================

#[test]
fn test_workflow_builder_every_setter_is_applied() {
    let cb = minimal_code_block();
    let cb2 = CodeBlock {
        action: CodeAction::Full,
        body: "fn main() {}".to_owned(),
        lang: Some("rust".to_owned()),
        target: None,
        anchor: None,
        old: None,
    };
    let check = minimal_verify_check();
    let ctx = minimal_agent_ctx();

    let node = WorkflowBuilder::new("set.workflow.all")
        .summary("workflow setter test")
        .steps(["step one", "step two"])
        .input(["param_a", "param_b"])
        .output(["result_x"])
        .code(cb2)
        .code_blocks([cb])
        .verify([check])
        .agent_context(ctx)
        .target("src/main.rs")
        .priority(Priority::High)
        .stability(Stability::Medium)
        .detail("workflow detail")
        .notes("workflow notes")
        .tags(["wf", "auth"])
        .depends(["set.workflow.dep"])
        .related_to(["set.workflow.rel"])
        .build_unchecked()
        .unwrap();

    assert_eq!(node.id, "set.workflow.all");
    assert_eq!(node.node_type, NodeType::Workflow);
    assert_eq!(node.summary, "workflow setter test");
    assert_eq!(node.steps.as_deref().unwrap(), &["step one", "step two"]);
    assert_eq!(node.input.as_deref().unwrap(), &["param_a", "param_b"]);
    assert_eq!(node.output.as_deref().unwrap(), &["result_x"]);
    assert!(node.code.is_some());
    assert_eq!(node.code_blocks.as_ref().unwrap().len(), 1);
    assert_eq!(node.verify.as_ref().unwrap().len(), 1);
    assert!(node.agent_context.is_some());
    assert_eq!(node.target.as_deref(), Some("src/main.rs"));
    assert_eq!(node.priority, Some(Priority::High));
    assert_eq!(node.stability, Some(Stability::Medium));
    assert_eq!(node.detail.as_deref(), Some("workflow detail"));
    assert_eq!(node.notes.as_deref(), Some("workflow notes"));
    assert_eq!(node.tags.as_deref().unwrap(), &["wf", "auth"]);
    assert_eq!(node.depends.as_deref().unwrap(), &["set.workflow.dep"]);
    assert_eq!(node.related_to.as_deref().unwrap(), &["set.workflow.rel"]);
}

#[test]
fn test_workflow_builder_setter_returns_self_for_chaining() {
    let _ = WorkflowBuilder::new("chain.wf.x")
        .summary("s")
        .steps(["a"])
        .input(["i"])
        .output(["o"])
        .target("src/x.rs")
        .priority(Priority::Normal)
        .stability(Stability::Low)
        .detail("d")
        .notes("n")
        .tags(["t"])
        .build_unchecked()
        .unwrap();
}

#[test]
fn test_workflow_builder_setter_accepts_iterator_and_vec_for_steps() {
    let steps: Vec<String> = vec!["step one".to_owned(), "step two".to_owned()];

    let n1 = WorkflowBuilder::new("set.wf.s1")
        .summary("s")
        .steps(steps.clone())
        .build_unchecked()
        .unwrap();
    assert_eq!(n1.steps.as_deref().unwrap().len(), 2);

    let n2 = WorkflowBuilder::new("set.wf.s2")
        .summary("s")
        .steps(["a", "b", "c"])
        .build_unchecked()
        .unwrap();
    assert_eq!(n2.steps.as_deref().unwrap().len(), 3);

    let n3 = WorkflowBuilder::new("set.wf.s3")
        .summary("s")
        .steps(steps.iter().cloned())
        .build_unchecked()
        .unwrap();
    assert_eq!(n3.steps.as_deref().unwrap().len(), 2);

    let n4 = WorkflowBuilder::new("set.wf.s4")
        .summary("s")
        .steps(std::iter::once("solo step"))
        .build_unchecked()
        .unwrap();
    assert_eq!(n4.steps.as_deref().unwrap().len(), 1);
}

// ============================================================================
// OrchestrationBuilder
// ============================================================================

#[test]
fn test_orchestration_builder_every_setter_is_applied() {
    let g1 = minimal_group("set.orch.all");
    let g2 = ParallelGroup {
        group: "g2".to_owned(),
        nodes: vec!["set.orch.all".to_owned()],
        strategy: Strategy::Parallel,
        requires: Some(vec!["g1".to_owned()]),
        max_concurrency: Some(2),
    };

    let node = OrchestrationBuilder::new("set.orch.all")
        .summary("orchestration setter test")
        .parallel_groups([g1, g2])
        .detail("orch detail")
        .notes("orch notes")
        .depends(["set.orch.dep"])
        .tags(["deploy", "ci"])
        .build_unchecked()
        .unwrap();

    assert_eq!(node.id, "set.orch.all");
    assert_eq!(node.node_type, NodeType::Orchestration);
    assert_eq!(node.summary, "orchestration setter test");
    assert_eq!(node.parallel_groups.as_ref().unwrap().len(), 2);
    assert_eq!(node.detail.as_deref(), Some("orch detail"));
    assert_eq!(node.notes.as_deref(), Some("orch notes"));
    assert_eq!(node.depends.as_deref().unwrap(), &["set.orch.dep"]);
    assert_eq!(node.tags.as_deref().unwrap(), &["deploy", "ci"]);
}

#[test]
fn test_orchestration_builder_setter_returns_self_for_chaining() {
    let _ = OrchestrationBuilder::new("chain.orch.x")
        .summary("s")
        .parallel_groups([minimal_group("chain.orch.x")])
        .detail("d")
        .notes("n")
        .tags(["t"])
        .build_unchecked()
        .unwrap();
}

#[test]
fn test_orchestration_builder_setter_accepts_iterator_and_vec_for_groups() {
    let groups: Vec<ParallelGroup> =
        vec![minimal_group("set.orch.g1"), minimal_group("set.orch.g1")];

    let n1 = OrchestrationBuilder::new("set.orch.g1")
        .summary("s")
        .parallel_groups(groups)
        .build_unchecked()
        .unwrap();
    assert_eq!(n1.parallel_groups.as_ref().unwrap().len(), 2);

    let groups2 = vec![minimal_group("set.orch.g2"), minimal_group("set.orch.g2")];
    let n2 = OrchestrationBuilder::new("set.orch.g2")
        .summary("s")
        .parallel_groups(groups2)
        .build_unchecked()
        .unwrap();
    assert_eq!(n2.parallel_groups.as_ref().unwrap().len(), 2);
}

// ============================================================================
// FactsBuilder
// ============================================================================

#[test]
fn test_facts_builder_every_setter_is_applied() {
    let node = FactsBuilder::new("set.facts.all")
        .summary("facts setter test")
        .items(["item a", "item b"])
        .detail("facts detail")
        .stability(Stability::High)
        .notes("facts notes")
        .depends(["set.facts.dep"])
        .tags(["auth", "policy"])
        .build_unchecked()
        .unwrap();

    assert_eq!(node.id, "set.facts.all");
    assert_eq!(node.node_type, NodeType::Facts);
    assert_eq!(node.summary, "facts setter test");
    assert_eq!(node.items.as_deref().unwrap(), &["item a", "item b"]);
    assert_eq!(node.detail.as_deref(), Some("facts detail"));
    assert_eq!(node.stability, Some(Stability::High));
    assert_eq!(node.notes.as_deref(), Some("facts notes"));
    assert_eq!(node.depends.as_deref().unwrap(), &["set.facts.dep"]);
    assert_eq!(node.tags.as_deref().unwrap(), &["auth", "policy"]);
}

#[test]
fn test_facts_builder_setter_returns_self_for_chaining() {
    let _ = FactsBuilder::new("chain.facts.x")
        .summary("s")
        .items(["i"])
        .detail("d")
        .stability(Stability::Low)
        .notes("n")
        .tags(["t"])
        .build_unchecked()
        .unwrap();
}

#[test]
fn test_facts_builder_setter_accepts_iterator_and_vec_for_items() {
    let items: Vec<String> = vec!["a".to_owned(), "b".to_owned()];

    let n1 = FactsBuilder::new("set.facts.i1")
        .summary("s")
        .items(items.clone())
        .build_unchecked()
        .unwrap();
    assert_eq!(n1.items.as_deref().unwrap().len(), 2);

    let n2 = FactsBuilder::new("set.facts.i2")
        .summary("s")
        .items(["x", "y", "z"])
        .build_unchecked()
        .unwrap();
    assert_eq!(n2.items.as_deref().unwrap().len(), 3);

    let n3 = FactsBuilder::new("set.facts.i3")
        .summary("s")
        .items(items.iter().cloned())
        .build_unchecked()
        .unwrap();
    assert_eq!(n3.items.as_deref().unwrap().len(), 2);

    let n4 = FactsBuilder::new("set.facts.i4")
        .summary("s")
        .items(std::iter::once("solo"))
        .build_unchecked()
        .unwrap();
    assert_eq!(n4.items.as_deref().unwrap().len(), 1);
}

// ============================================================================
// RulesBuilder
// ============================================================================

#[test]
fn test_rules_builder_every_setter_is_applied() {
    let node = RulesBuilder::new("set.rules.all")
        .summary("rules setter test")
        .items(["require HTTPS", "rate-limit"])
        .detail("rules detail")
        .stability(Stability::High)
        .notes("rules notes")
        .depends(["set.rules.dep"])
        .tags(["security"])
        .build_unchecked()
        .unwrap();

    assert_eq!(node.id, "set.rules.all");
    assert_eq!(node.node_type, NodeType::Rules);
    assert_eq!(node.summary, "rules setter test");
    assert_eq!(
        node.items.as_deref().unwrap(),
        &["require HTTPS", "rate-limit"]
    );
    assert_eq!(node.detail.as_deref(), Some("rules detail"));
    assert_eq!(node.stability, Some(Stability::High));
    assert_eq!(node.notes.as_deref(), Some("rules notes"));
    assert_eq!(node.depends.as_deref().unwrap(), &["set.rules.dep"]);
    assert_eq!(node.tags.as_deref().unwrap(), &["security"]);
}

#[test]
fn test_rules_builder_setter_returns_self_for_chaining() {
    let _ = RulesBuilder::new("chain.rules.x")
        .summary("s")
        .items(["i"])
        .detail("d")
        .stability(Stability::Medium)
        .notes("n")
        .tags(["t"])
        .build_unchecked()
        .unwrap();
}

#[test]
fn test_rules_builder_setter_accepts_iterator_and_vec_for_items() {
    let items: Vec<String> = vec!["rule a".to_owned(), "rule b".to_owned()];

    let n1 = RulesBuilder::new("set.rules.i1")
        .summary("s")
        .items(items.clone())
        .build_unchecked()
        .unwrap();
    assert_eq!(n1.items.as_deref().unwrap().len(), 2);

    let n2 = RulesBuilder::new("set.rules.i2")
        .summary("s")
        .items(items.iter().cloned())
        .build_unchecked()
        .unwrap();
    assert_eq!(n2.items.as_deref().unwrap().len(), 2);
}

// ============================================================================
// DecisionBuilder
// ============================================================================

#[test]
fn test_decision_builder_every_setter_is_applied() {
    let node = DecisionBuilder::new("set.decision.all")
        .summary("decision setter test")
        .rationale(["ACID required", "existing expertise"])
        .tradeoffs(["complex scaling"])
        .resolution(["use PostgreSQL 16"])
        .detail("decision detail")
        .stability(Stability::High)
        .notes("decision notes")
        .depends(["set.decision.dep"])
        .tags(["architecture"])
        .build_unchecked()
        .unwrap();

    assert_eq!(node.id, "set.decision.all");
    assert_eq!(node.node_type, NodeType::Decision);
    assert_eq!(node.summary, "decision setter test");
    assert_eq!(node.rationale.as_deref().unwrap().len(), 2);
    assert_eq!(node.tradeoffs.as_deref().unwrap().len(), 1);
    assert_eq!(node.resolution.as_deref().unwrap().len(), 1);
    assert_eq!(node.detail.as_deref(), Some("decision detail"));
    assert_eq!(node.stability, Some(Stability::High));
    assert_eq!(node.notes.as_deref(), Some("decision notes"));
    assert_eq!(node.depends.as_deref().unwrap(), &["set.decision.dep"]);
    assert_eq!(node.tags.as_deref().unwrap(), &["architecture"]);
}

#[test]
fn test_decision_builder_setter_returns_self_for_chaining() {
    let _ = DecisionBuilder::new("chain.decision.x")
        .summary("s")
        .rationale(["r"])
        .tradeoffs(["t"])
        .resolution(["res"])
        .detail("d")
        .stability(Stability::Low)
        .notes("n")
        .tags(["tag"])
        .build_unchecked()
        .unwrap();
}

#[test]
fn test_decision_builder_setter_accepts_iterator_and_vec_for_rationale() {
    let rationale: Vec<String> = vec!["reason a".to_owned(), "reason b".to_owned()];

    let n1 = DecisionBuilder::new("set.decision.r1")
        .summary("s")
        .rationale(rationale.clone())
        .build_unchecked()
        .unwrap();
    assert_eq!(n1.rationale.as_deref().unwrap().len(), 2);

    let n2 = DecisionBuilder::new("set.decision.r2")
        .summary("s")
        .rationale(["x", "y", "z"])
        .build_unchecked()
        .unwrap();
    assert_eq!(n2.rationale.as_deref().unwrap().len(), 3);

    let n3 = DecisionBuilder::new("set.decision.r3")
        .summary("s")
        .rationale(rationale.iter().cloned())
        .build_unchecked()
        .unwrap();
    assert_eq!(n3.rationale.as_deref().unwrap().len(), 2);
}

// ============================================================================
// MemoryEntryBuilder
// ============================================================================

#[test]
fn test_memory_entry_builder_every_setter_is_applied() {
    let entry = MemoryEntryBuilder::new("repo.pattern", "rust.repository", MemoryAction::Upsert)
        .value("row_to_column uses get()")
        .scope(MemoryScope::Project)
        .ttl(MemoryTtl::Permanent)
        .build()
        .unwrap();

    assert_eq!(entry.key, "repo.pattern");
    assert_eq!(entry.topic, "rust.repository");
    assert_eq!(entry.action, MemoryAction::Upsert);
    assert_eq!(entry.value.as_deref(), Some("row_to_column uses get()"));
    assert_eq!(entry.scope, Some(MemoryScope::Project));
    assert_eq!(entry.ttl, Some(MemoryTtl::Permanent));
}

#[test]
fn test_memory_entry_builder_search_setters_applied() {
    let entry = MemoryEntryBuilder::new("s.key", "s.topic", MemoryAction::Search)
        .query("how are optional fields handled")
        .max_results(10)
        .build()
        .unwrap();

    assert_eq!(
        entry.query.as_deref(),
        Some("how are optional fields handled")
    );
    assert_eq!(entry.max_results, Some(10));
}

#[test]
fn test_memory_entry_builder_setter_returns_self_for_chaining() {
    let _ = MemoryEntryBuilder::new("k", "t", MemoryAction::Upsert)
        .value("v")
        .scope(MemoryScope::Session)
        .ttl(MemoryTtl::Session)
        .query("q")
        .max_results(5)
        .build()
        .unwrap();
}

// ============================================================================
// CodeBlockBuilder
// ============================================================================

#[test]
fn test_code_block_builder_every_setter_is_applied() {
    // Create: lang, target, body
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/lib.rs")
        .body("fn hello() {}")
        .build()
        .unwrap();
    assert_eq!(cb.action, CodeAction::Create);
    assert_eq!(cb.lang.as_deref(), Some("rust"));
    assert_eq!(cb.target.as_deref(), Some("src/lib.rs"));
    assert_eq!(cb.body, "fn hello() {}");

    // Replace with old: old setter
    let cb_r = CodeBlockBuilder::replace()
        .lang("python")
        .target("main.py")
        .old("def old(): pass")
        .body("def new(): pass")
        .build()
        .unwrap();
    assert_eq!(cb_r.action, CodeAction::Replace);
    assert_eq!(cb_r.old.as_deref(), Some("def old(): pass"));
    assert!(cb_r.anchor.is_none());

    // anchor is only valid for Create/Append, not Replace (spec §23.4).
    // Verify the anchor setter still compiles and is reachable via Create.
    let cb_a = CodeBlockBuilder::create()
        .target("main.py")
        .anchor("def target_fn(")
        .body("def target_fn(): pass")
        .build()
        .unwrap();
    assert_eq!(cb_a.anchor.as_deref(), Some("def target_fn("));
    assert!(cb_a.old.is_none());
}

#[test]
fn test_code_block_builder_setter_returns_self_for_chaining() {
    let _ = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/lib.rs")
        .body("fn x() {}")
        .build()
        .unwrap();
}

// ============================================================================
// VerifyCheckBuilder
// ============================================================================

#[test]
fn test_verify_check_builder_every_setter_is_applied() {
    let cmd = VerifyCheckBuilder::command("cargo test")
        .expect("exit_code_0")
        .build()
        .unwrap();
    match cmd {
        VerifyCheck::Command { run, expect } => {
            assert_eq!(run, "cargo test");
            assert_eq!(expect.as_deref(), Some("exit_code_0"));
        }
        _ => panic!("wrong variant"),
    }

    let fe = VerifyCheckBuilder::file_exists("src/lib.rs")
        .build()
        .unwrap();
    assert!(matches!(fe, VerifyCheck::FileExists { .. }));

    let fc = VerifyCheckBuilder::file_contains("src/lib.rs", "fn main")
        .build()
        .unwrap();
    match fc {
        VerifyCheck::FileContains { file, pattern } => {
            assert_eq!(file, "src/lib.rs");
            assert_eq!(pattern, "fn main");
        }
        _ => panic!("wrong variant"),
    }

    let fnc = VerifyCheckBuilder::file_not_contains("src/lib.rs", "todo!()")
        .build()
        .unwrap();
    assert!(matches!(fnc, VerifyCheck::FileNotContains { .. }));

    let ns = VerifyCheckBuilder::node_status("auth.login", "completed")
        .build()
        .unwrap();
    match ns {
        VerifyCheck::NodeStatus { node, status } => {
            assert_eq!(node, "auth.login");
            assert_eq!(status, "completed");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_verify_check_builder_setter_returns_self_for_chaining() {
    let _ = VerifyCheckBuilder::command("cargo check")
        .expect("exit_code_0")
        .build()
        .unwrap();
}
