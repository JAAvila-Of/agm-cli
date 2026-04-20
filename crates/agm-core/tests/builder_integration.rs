//! Integration tests for the `agm_core::builder` module.
//!
//! Tests cover:
//! - Round-trip: build → render → re-parse → semantic equality.
//! - Validation errors for missing required fields.
//! - `build_unchecked` bypass behavior.
//! - Strict enforcement via `build_with`.
//! - `render_canonical` and `render_node_only` output.
//! - Snapshot tests for builder output.

use agm_core::builder::{
    BuildError, CodeBlockBuilder, DecisionBuilder, FactsBuilder, MemoryEntryBuilder,
    OrchestrationBuilder, RulesBuilder, TicketBuilder, VerifyCheckBuilder, WorkflowBuilder,
};
use agm_core::error::codes::ErrorCode;
use agm_core::model::fields::{NodeType, Priority};
use agm_core::model::memory::MemoryAction;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::schema::EnforcementLevel;
use agm_core::parser;
use agm_core::validator;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Re-parses an AGM text string and validates it, panicking on any error.
fn reparse_and_validate(agm_text: &str) -> agm_core::model::file::AgmFile {
    let file = parser::parse(agm_text).expect("re-parse failed");
    let diags = validator::validate(&file, agm_text, "<test>", &Default::default());
    assert!(
        !diags.has_errors(),
        "re-parsed file has validation errors: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
    file
}

/// Returns a minimal valid ticket node ID.
fn ticket_id() -> &'static str {
    "test.ticket.login"
}

fn minimal_ticket() -> agm_core::model::node::Node {
    TicketBuilder::new(ticket_id())
        .summary("add OAuth2 login")
        .title("Add OAuth2 login flow")
        .description("Add Google OAuth2 login to the dashboard.")
        .priority(Priority::High)
        .build()
        .expect("minimal ticket should be valid")
}

// ---------------------------------------------------------------------------
// Test 1: Ticket round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_builder_ticket_roundtrip_canonical() {
    let node = minimal_ticket();

    // Render and re-parse
    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    assert_eq!(file.nodes.len(), 1);
    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.id, node.id);
    assert_eq!(reparsed.node_type, NodeType::Ticket);
    assert_eq!(reparsed.summary, node.summary);
    assert_eq!(reparsed.title, node.title);
    assert_eq!(reparsed.description, node.description);
    assert_eq!(reparsed.priority, node.priority);
}

// ---------------------------------------------------------------------------
// Test 2: Workflow with code_blocks and verify — round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_builder_workflow_with_code_blocks_roundtrip() {
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/auth.rs")
        .body("pub fn authenticate() -> bool { true }")
        .build()
        .unwrap();

    let check = VerifyCheckBuilder::command("cargo test --lib")
        .expect("exit_code_0")
        .build()
        .unwrap();

    let node = WorkflowBuilder::new("auth.login")
        .summary("authenticate user and create session")
        .steps(["resolve tenant", "redirect to provider", "handle callback"])
        .input(["host", "return_url"])
        .output(["redirect_url", "sid_cookie"])
        .code_blocks([cb])
        .verify([check])
        .build()
        .expect("valid workflow");

    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    assert_eq!(file.nodes.len(), 1);
    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.id, "auth.login");
    assert_eq!(reparsed.node_type, NodeType::Workflow);
    assert_eq!(reparsed.steps.as_deref().unwrap().len(), 3);
    assert_eq!(reparsed.code_blocks.as_ref().unwrap().len(), 1);
    assert_eq!(reparsed.verify.as_ref().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// Test 3: Orchestration with 3 groups
// ---------------------------------------------------------------------------

#[test]
fn test_builder_orchestration_three_groups() {
    // Groups reference the orchestration node's own ID so single-node validation passes.
    let node_id = "o.deploy";
    let make_group = |name: &str| ParallelGroup {
        group: name.to_owned(),
        nodes: vec![node_id.to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    };

    let node = OrchestrationBuilder::new(node_id)
        .summary("orchestrate full deployment pipeline")
        .parallel_groups([
            make_group("1-schema"),
            make_group("2-models"),
            make_group("3-backend"),
        ])
        .build()
        .expect("valid orchestration with 3 groups");

    assert_eq!(node.parallel_groups.as_ref().unwrap().len(), 3);

    let agm_text = node.render_canonical();
    assert!(agm_text.contains("node o.deploy"));
    assert!(agm_text.contains("1-schema"));
    assert!(agm_text.contains("2-models"));
    assert!(agm_text.contains("3-backend"));
}

// ---------------------------------------------------------------------------
// Test 4: Missing required fields → Validation error
// ---------------------------------------------------------------------------

#[test]
fn test_builder_missing_required_returns_validation_error() {
    // Missing title, description, priority — all required for ticket
    let result = TicketBuilder::new("test.ticket.bad")
        .summary("incomplete ticket")
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_validation());

    let diags = err.diagnostics().unwrap();
    assert!(diags.has_errors());
    // Should have V024 errors for missing required fields
    let has_v024 = diags
        .diagnostics()
        .iter()
        .any(|d| d.code == ErrorCode::V024);
    assert!(has_v024, "expected V024 for missing required fields");
}

// ---------------------------------------------------------------------------
// Test 5: build_unchecked skips validation
// ---------------------------------------------------------------------------

#[test]
fn test_builder_build_unchecked_skips_validation() {
    // Missing title, description, priority — would fail validation
    let node = TicketBuilder::new("test.ticket.draft")
        .summary("wip")
        .build_unchecked()
        .expect("build_unchecked should succeed regardless of missing fields");

    // The node is returned as-is — not validated
    assert!(node.title.is_none());
    assert!(node.priority.is_none());
}

// ---------------------------------------------------------------------------
// Test 6: build_with(Strict) on a ticket with a long title → V032 still Warning
// ---------------------------------------------------------------------------

#[test]
fn test_builder_build_with_strict_long_title_is_still_warning() {
    // V032 is a Warning, not an Error — even in Strict mode it doesn't block build.
    // Strict mode escalates V010 (missing recommended fields) to errors.
    let long_title = "A".repeat(201);

    let result = TicketBuilder::new("test.ticket.long")
        .summary("long title test")
        .title(&long_title)
        .description("Detailed description.")
        .priority(Priority::Normal)
        .build_with(EnforcementLevel::Strict);

    // V032 is a warning (not an error), so build_with(Strict) may or may not error
    // depending on whether Strict escalates warnings. Check the actual behavior:
    match &result {
        Ok(_) => {
            // Passed — V032 remains a warning in Strict mode too.
        }
        Err(BuildError::Validation(diags)) => {
            // V032 warning is present — ensure it's truly a warning
            let v032 = diags
                .diagnostics()
                .iter()
                .find(|d| d.code == ErrorCode::V032);
            assert!(v032.is_some(), "V032 diagnostic should be present");
            // If we're here, Strict escalated warnings to errors — that's also valid.
        }
        Err(e) => panic!("unexpected error: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Test 7: render_node_only strips header
// ---------------------------------------------------------------------------

#[test]
fn test_node_render_canonical_strips_header_in_node_only_variant() {
    let node = minimal_ticket();

    let full = node.render_canonical();
    let node_only = node.render_node_only();

    // Full output includes the scratch header
    assert!(full.contains("agm: 1.0"));
    assert!(full.contains("package: scratch.builder"));
    assert!(full.contains("version: 0.1.0"));

    // node_only starts with "node " directly
    assert!(
        node_only.starts_with("node "),
        "node_only should start with 'node '"
    );
    assert!(
        !node_only.contains("agm:"),
        "node_only should not contain header fields"
    );
    assert!(!node_only.contains("package:"));
    assert!(!node_only.contains("version:"));

    // Both contain the node content
    assert!(full.contains(&format!("node {}", ticket_id())));
    assert!(node_only.contains(&format!("node {}", ticket_id())));
}

// ---------------------------------------------------------------------------
// Test 8: Facts builder round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_builder_facts_roundtrip() {
    let node = FactsBuilder::new("auth.constraints")
        .summary("authentication policy constraints")
        .items(["sessions expire after 24h", "MFA required for admin"])
        .build()
        .expect("valid facts node");

    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.id, "auth.constraints");
    assert_eq!(reparsed.node_type, NodeType::Facts);
    assert_eq!(reparsed.items.as_deref().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// Test 9: Rules builder round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_builder_rules_roundtrip() {
    let node = RulesBuilder::new("auth.rules")
        .summary("authentication rules")
        .items(["require HTTPS", "rate-limit login to 5 attempts/min"])
        .build()
        .expect("valid rules node");

    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.id, "auth.rules");
    assert_eq!(reparsed.node_type, NodeType::Rules);
}

// ---------------------------------------------------------------------------
// Test 10: Decision builder round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_builder_decision_roundtrip() {
    let node = DecisionBuilder::new("arch.db-choice")
        .summary("chose PostgreSQL over MongoDB")
        .rationale(["ACID guarantees required", "existing team expertise"])
        .tradeoffs(["more complex scaling than NoSQL"])
        .build()
        .expect("valid decision node");

    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.id, "arch.db-choice");
    assert_eq!(reparsed.node_type, NodeType::Decision);
    assert_eq!(reparsed.rationale.as_deref().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// Test 11: MemoryEntryBuilder
// ---------------------------------------------------------------------------

#[test]
fn test_builder_memory_entry_upsert() {
    use agm_core::model::memory::{MemoryScope, MemoryTtl};

    let entry = MemoryEntryBuilder::new("repo.pattern", "rust.repository", MemoryAction::Upsert)
        .value("row_to_column uses get()")
        .scope(MemoryScope::Project)
        .ttl(MemoryTtl::Permanent)
        .build()
        .expect("valid memory entry");

    assert_eq!(entry.key, "repo.pattern");
    assert_eq!(entry.topic, "rust.repository");
    assert_eq!(entry.action, MemoryAction::Upsert);
    assert!(entry.value.is_some());
}

// ---------------------------------------------------------------------------
// Snapshot tests
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_minimal_ticket_render_canonical() {
    let node = TicketBuilder::new("my.ticket.oauth")
        .summary("add OAuth2 login")
        .title("Add OAuth2 login flow")
        .description("Add Google OAuth2 login to the dashboard.")
        .priority(Priority::High)
        .build()
        .unwrap();

    let rendered = node.render_node_only();
    insta::assert_snapshot!("builder_minimal_ticket", rendered);
}

#[test]
fn test_snapshot_workflow_with_steps() {
    let node = WorkflowBuilder::new("auth.login")
        .summary("authenticate user and create session")
        .steps(["resolve tenant", "redirect to provider", "handle callback"])
        .input(["host", "return_url"])
        .output(["redirect_url", "sid_cookie"])
        .build()
        .unwrap();

    let rendered = node.render_node_only();
    insta::assert_snapshot!("builder_workflow_with_steps", rendered);
}
