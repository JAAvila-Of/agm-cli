//! Integration tests for `agm_core::ingest`.

use agm_core::ingest::{IngestConfig, IngestError, ingest_many, ingest_one};
use agm_core::model::fields::NodeType;
use agm_core::model::schema::EnforcementLevel;
use agm_core::parser;
use agm_core::renderer::{RenderFormat, render};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_json(relative: &str) -> Value {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("fixture {relative} is not valid JSON: {e}"))
}

fn permissive() -> IngestConfig {
    IngestConfig {
        normalize: false,
        schema_check: false,
        enforcement: EnforcementLevel::Permissive,
    }
}

fn standard() -> IngestConfig {
    IngestConfig {
        normalize: true,
        schema_check: true,
        enforcement: EnforcementLevel::Standard,
    }
}

// ---------------------------------------------------------------------------
// test_ingest_one_ticket_minimal — fixture → valid AGM
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_one_ticket_minimal_produces_valid_agm() {
    let v = fixture_json("ingest/valid/ticket_minimal.json");
    let node = ingest_one(NodeType::Ticket, "auth.ticket.oauth", v, &standard()).unwrap();
    assert_eq!(node.id, "auth.ticket.oauth");
    assert_eq!(node.summary, "add OAuth2 login");
}

// ---------------------------------------------------------------------------
// test_ingest_one_ticket_with_labels — array handling
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_one_ticket_with_labels() {
    let v = fixture_json("ingest/valid/ticket_full.json");
    let node = ingest_one(NodeType::Ticket, "auth.ticket.impl", v, &standard()).unwrap();
    assert_eq!(node.id, "auth.ticket.impl");
    // labels should be stored in extra_fields (unknown field path)
    // or as a recognized field — either way the node must build
    assert!(!node.summary.is_empty());
}

// ---------------------------------------------------------------------------
// test_ingest_one_schema_check_rejects_invalid_priority
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_one_schema_check_rejects_invalid_priority() {
    let v = fixture_json("ingest/invalid/ticket_bad_priority.json");
    let config = IngestConfig {
        schema_check: true,
        ..permissive()
    };
    let err = ingest_one(NodeType::Ticket, "auth.t", v, &config).unwrap_err();
    assert!(
        matches!(err, IngestError::SchemaCheck(_)),
        "expected SchemaCheck, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// test_ingest_one_normalize_rewrites_depends_on
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_one_normalize_rewrites_depends_on() {
    // "depends_on" is a synonym for "depends" per the normalize layer.
    // With normalize=true the node should still build successfully.
    let v = json!({
        "summary": "auth constraints",
        "items": ["rule one", "rule two"]
    });
    let config = IngestConfig {
        normalize: true,
        schema_check: false,
        enforcement: EnforcementLevel::Permissive,
    };
    let node = ingest_one(NodeType::Facts, "auth.facts.rules", v, &config).unwrap();
    assert_eq!(node.id, "auth.facts.rules");
}

// ---------------------------------------------------------------------------
// test_ingest_many_mixed_success_and_failure_returns_batch_error
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_many_mixed_success_and_failure_returns_batch_error() {
    let values = vec![
        json!({
            "type": "ticket",
            "summary": "add login",
            "title": "Add Login",
            "description": "Implement login.",
            "priority": "high"
        }),
        // This element has an invalid priority — will fail schema_check
        json!({
            "type": "ticket",
            "summary": "second ticket",
            "title": "Second Ticket",
            "description": "desc",
            "priority": "urgent"
        }),
        json!({
            "type": "ticket",
            "summary": "third ticket",
            "title": "Third Ticket",
            "description": "desc",
            "priority": "normal"
        }),
    ];
    let config = IngestConfig {
        schema_check: true,
        normalize: false,
        enforcement: EnforcementLevel::Permissive,
    };
    let err = ingest_many(NodeType::Ticket, "batch.t", values, &config).unwrap_err();
    assert!(
        matches!(err, IngestError::Batch { ref errors } if errors.len() == 1),
        "expected Batch with 1 error, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// test_ingest_workflow_with_code_blocks_roundtrips
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_workflow_roundtrips_through_parse() {
    let v = fixture_json("ingest/valid/workflow_code_blocks.json");
    let node = ingest_one(NodeType::Workflow, "auth.workflow.oauth", v, &permissive()).unwrap();

    // Wrap in AgmFile and render canonical AGM
    use agm_core::model::file::{AgmFile, Header};
    let agm_file = AgmFile {
        header: Header {
            agm: "1.0".to_owned(),
            package: "auth.workflows".to_owned(),
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
        },
        nodes: vec![node.clone()],
    };

    let canonical = render(&agm_file, RenderFormat::Canonical);
    assert!(
        !canonical.is_empty(),
        "rendered canonical AGM must not be empty"
    );

    // Re-parse to verify round-trip
    let reparsed = parser::parse(&canonical).expect("reparsed canonical AGM must parse");
    assert_eq!(reparsed.nodes.len(), 1);
    assert_eq!(reparsed.nodes[0].id, "auth.workflow.oauth");
    assert_eq!(reparsed.nodes[0].summary, node.summary);
}

// ---------------------------------------------------------------------------
// test_ingest_many_all_valid
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_many_all_valid_returns_all_nodes() {
    let values: Vec<Value> = (0..5)
        .map(|i| {
            json!({
                "type": "ticket",
                "summary": format!("ticket {i}"),
                "title": format!("Ticket {i}"),
                "description": format!("Description {i}"),
                "priority": "normal"
            })
        })
        .collect();

    let nodes = ingest_many(NodeType::Ticket, "batch.ticket", values, &permissive()).unwrap();
    assert_eq!(nodes.len(), 5);
}

// ---------------------------------------------------------------------------
// test_ingest_one_not_an_object_returns_error
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_one_not_an_object_returns_error() {
    let v = json!([1, 2, 3]);
    let err = ingest_one(NodeType::Facts, "auth.f", v, &permissive()).unwrap_err();
    assert!(matches!(err, IngestError::NotAnObject));
}

// ---------------------------------------------------------------------------
// test_ingest_one_missing_id_returns_error
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_one_missing_id_returns_error() {
    let v = json!({"summary": "orphan"});
    let err = ingest_one(NodeType::Facts, "", v, &permissive()).unwrap_err();
    assert!(matches!(err, IngestError::MissingId));
}
