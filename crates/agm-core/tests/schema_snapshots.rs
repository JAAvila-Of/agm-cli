//! Snapshot tests for JSON Schema generation.
//!
//! Covers all 11 built-in node types × vanilla dialect, plus anthropic-tool-use
//! and openai-tool for ticket and orchestration, and strict vs default for ticket.
//!
//! Step 11 from the implementation plan.

use agm_core::model::fields::NodeType;
use agm_core::schemas::{SchemaDialect, SchemaOptions, schema_for};
use insta::assert_json_snapshot;

fn vanilla_opts() -> SchemaOptions {
    SchemaOptions::default()
}

fn strict_opts() -> SchemaOptions {
    SchemaOptions {
        strict: true,
        ..Default::default()
    }
}

fn anthropic_opts() -> SchemaOptions {
    SchemaOptions {
        dialect: SchemaDialect::AnthropicToolUse,
        ..Default::default()
    }
}

fn openai_opts() -> SchemaOptions {
    SchemaOptions {
        dialect: SchemaDialect::OpenAiTool,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// All 11 types — vanilla dialect
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_facts_vanilla() {
    let schema = schema_for(&NodeType::Facts, &vanilla_opts()).unwrap();
    assert_json_snapshot!("facts_vanilla", schema);
}

#[test]
fn test_snapshot_rules_vanilla() {
    let schema = schema_for(&NodeType::Rules, &vanilla_opts()).unwrap();
    assert_json_snapshot!("rules_vanilla", schema);
}

#[test]
fn test_snapshot_workflow_vanilla() {
    let schema = schema_for(&NodeType::Workflow, &vanilla_opts()).unwrap();
    assert_json_snapshot!("workflow_vanilla", schema);
}

#[test]
fn test_snapshot_entity_vanilla() {
    let schema = schema_for(&NodeType::Entity, &vanilla_opts()).unwrap();
    assert_json_snapshot!("entity_vanilla", schema);
}

#[test]
fn test_snapshot_decision_vanilla() {
    let schema = schema_for(&NodeType::Decision, &vanilla_opts()).unwrap();
    assert_json_snapshot!("decision_vanilla", schema);
}

#[test]
fn test_snapshot_exception_vanilla() {
    let schema = schema_for(&NodeType::Exception, &vanilla_opts()).unwrap();
    assert_json_snapshot!("exception_vanilla", schema);
}

#[test]
fn test_snapshot_example_vanilla() {
    let schema = schema_for(&NodeType::Example, &vanilla_opts()).unwrap();
    assert_json_snapshot!("example_vanilla", schema);
}

#[test]
fn test_snapshot_glossary_vanilla() {
    let schema = schema_for(&NodeType::Glossary, &vanilla_opts()).unwrap();
    assert_json_snapshot!("glossary_vanilla", schema);
}

#[test]
fn test_snapshot_anti_pattern_vanilla() {
    let schema = schema_for(&NodeType::AntiPattern, &vanilla_opts()).unwrap();
    assert_json_snapshot!("anti_pattern_vanilla", schema);
}

#[test]
fn test_snapshot_orchestration_vanilla() {
    let schema = schema_for(&NodeType::Orchestration, &vanilla_opts()).unwrap();
    assert_json_snapshot!("orchestration_vanilla", schema);
}

#[test]
fn test_snapshot_ticket_vanilla() {
    let schema = schema_for(&NodeType::Ticket, &vanilla_opts()).unwrap();
    assert_json_snapshot!("ticket_vanilla", schema);
}

// ---------------------------------------------------------------------------
// Ticket — dialect variants
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_ticket_anthropic_tool_use() {
    let schema = schema_for(&NodeType::Ticket, &anthropic_opts()).unwrap();
    assert_json_snapshot!("ticket_anthropic_tool_use", schema);
}

#[test]
fn test_snapshot_ticket_openai_tool() {
    let schema = schema_for(&NodeType::Ticket, &openai_opts()).unwrap();
    assert_json_snapshot!("ticket_openai_tool", schema);
}

// ---------------------------------------------------------------------------
// Ticket and Orchestration — strict mode
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_ticket_strict() {
    let schema = schema_for(&NodeType::Ticket, &strict_opts()).unwrap();
    assert_json_snapshot!("ticket_strict", schema);
}

#[test]
fn test_snapshot_orchestration_strict() {
    let schema = schema_for(&NodeType::Orchestration, &strict_opts()).unwrap();
    assert_json_snapshot!("orchestration_strict", schema);
}

// ---------------------------------------------------------------------------
// Orchestration — dialect variants
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_orchestration_anthropic_tool_use() {
    let schema = schema_for(&NodeType::Orchestration, &anthropic_opts()).unwrap();
    assert_json_snapshot!("orchestration_anthropic_tool_use", schema);
}

#[test]
fn test_snapshot_orchestration_openai_tool() {
    let schema = schema_for(&NodeType::Orchestration, &openai_opts()).unwrap();
    assert_json_snapshot!("orchestration_openai_tool", schema);
}
