//! Integration tests for memory validation using fixture `.agm` files.

use agm_core::error::codes::ErrorCode;
use agm_core::model::context::AgentContext;
use agm_core::model::memory::{MemoryAction, MemoryEntry};
use agm_core::parser;
use agm_core::validator::{self, ValidateOptions};

fn fixture(relative: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

fn parse_and_validate(
    src: &str,
    file_name: &str,
) -> agm_core::error::diagnostic::DiagnosticCollection {
    let file = parser::parse(src).expect("parse should succeed");
    validator::validate(&file, src, file_name, &ValidateOptions::default())
}

// ---------------------------------------------------------------------------
// Valid fixture tests
// ---------------------------------------------------------------------------

#[test]
fn test_memory_operations_valid_file_produces_no_errors() {
    let src = fixture("valid/memory_operations.agm");
    let result = parse_and_validate(&src, "memory_operations.agm");
    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.diagnostics()
    );
}

#[test]
fn test_memory_load_context_valid_file_produces_no_errors() {
    let src = fixture("valid/memory_load_context.agm");
    let result = parse_and_validate(&src, "memory_load_context.agm");
    assert!(
        !result.has_errors(),
        "Expected no errors for valid load_memory referencing declared topic, got: {:?}",
        result.diagnostics()
    );
}

// ---------------------------------------------------------------------------
// Invalid fixture tests
// ---------------------------------------------------------------------------

#[test]
fn test_memory_bad_key_produces_v022() {
    let src = fixture("invalid/memory_bad_key.agm");
    let result = parse_and_validate(&src, "memory_bad_key.agm");
    assert!(
        result
            .diagnostics()
            .iter()
            .any(|d| d.code == ErrorCode::V022),
        "Expected V022 for invalid memory key, got: {:?}",
        result.diagnostics()
    );
}

#[test]
fn test_memory_upsert_no_value_produces_v023() {
    let src = fixture("invalid/memory_upsert_no_value.agm");
    let result = parse_and_validate(&src, "memory_upsert_no_value.agm");
    assert!(
        result
            .diagnostics()
            .iter()
            .any(|d| d.code == ErrorCode::V023),
        "Expected V023 for upsert without value, got: {:?}",
        result.diagnostics()
    );
}

// ---------------------------------------------------------------------------
// Programmatic test for V026 (unresolved load_memory topic)
// ---------------------------------------------------------------------------

#[test]
fn test_memory_load_memory_unresolved_topic_produces_v026() {
    use agm_core::model::fields::{NodeType, Span};
    use agm_core::model::file::{AgmFile, Header};
    use agm_core::model::node::Node;

    let header = Header {
        agm: "1.0".to_owned(),
        package: "test.memory.unresolved".to_owned(),
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
    };

    // Node has a memory entry with topic "infrastructure"
    let memory_node = Node {
        id: "data.store".to_owned(),
        node_type: NodeType::Facts,
        summary: "data storage patterns".to_owned(),
        memory: Some(vec![MemoryEntry {
            key: "data.conn".to_owned(),
            topic: "infrastructure".to_owned(),
            action: MemoryAction::Get,
            value: None,
            scope: None,
            ttl: None,
            query: None,
            max_results: None,
            extra_fields: Default::default(),
        }]),
        span: Span::new(5, 7),
        ..Default::default()
    };

    // Node that references "rust.repository" in load_memory, which is NOT declared
    let context_node = Node {
        id: "data.migrate".to_owned(),
        node_type: NodeType::Workflow,
        summary: "migrate database schema".to_owned(),
        agent_context: Some(AgentContext {
            load_nodes: None,
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: Some(vec!["rust.repository".to_owned()]),
        }),
        span: Span::new(10, 15),
        ..Default::default()
    };

    let file = AgmFile {
        header,
        nodes: vec![memory_node, context_node],
    };

    let result = validator::validate(
        &file,
        "",
        "test_unresolved.agm",
        &ValidateOptions::default(),
    );

    assert!(
        result
            .diagnostics()
            .iter()
            .any(|d| d.code == ErrorCode::V026),
        "Expected V026 warning for unresolved load_memory topic, got: {:?}",
        result.diagnostics()
    );
}
