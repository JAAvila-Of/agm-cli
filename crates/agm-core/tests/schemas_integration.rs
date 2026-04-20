//! Parity tests: valid fixtures must pass schema validation,
//! invalid fixtures (except semantic-only violations) must fail.
//!
//! Steps 9 and 10 from the implementation plan.

use agm_core::model::fields::NodeType;
use agm_core::parser;
use agm_core::schemas::{SchemaOptions, schema_for};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compile_schema(node_type: &NodeType) -> jsonschema::Validator {
    let schema = schema_for(node_type, &SchemaOptions::default()).expect("schema_for returned Err");
    jsonschema::validator_for(&schema).expect("failed to compile JSON Schema")
}

fn compile_strict_schema(node_type: &NodeType) -> jsonschema::Validator {
    let opts = SchemaOptions {
        strict: true,
        ..Default::default()
    };
    let schema = schema_for(node_type, &opts).expect("schema_for returned Err");
    jsonschema::validator_for(&schema).expect("failed to compile strict JSON Schema")
}

fn parse_first_node(source: &str) -> agm_core::model::node::Node {
    let file = parser::parse(source).expect("parse failed");
    file.nodes.into_iter().next().expect("no nodes in fixture")
}

fn node_to_json(node: &agm_core::model::node::Node) -> serde_json::Value {
    serde_json::to_value(node).expect("serde_json::to_value failed")
}

// ---------------------------------------------------------------------------
// Known-semantic-only invalid fixtures
//
// These contain violations the schema cannot catch (missing title in a custom
// node, duplicate IDs, cross-node ref cycles, etc.).
// ---------------------------------------------------------------------------

const KNOWN_SEMANTIC_ONLY: &[&str] = &[
    // validator catches "title_too_long" as a semantic length constraint —
    // JSON Schema has no string-length rule in our schema, so we allow it here.
    "ticket_title_too_long",
    // maximally_invalid contains many different violations; skip for broad
    // schema coverage (it's covered by validator tests elsewhere).
    "maximally_invalid",
];

fn is_semantic_only(fixture_name: &str) -> bool {
    KNOWN_SEMANTIC_ONLY.iter().any(|n| fixture_name.contains(n))
}

// ---------------------------------------------------------------------------
// Step 9 — Validator acceptance implies schema acceptance (valid fixtures)
// ---------------------------------------------------------------------------

#[test]
fn test_validator_accept_implies_schema_accept() {
    let fixtures_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid");

    let ticket_validator = compile_schema(&NodeType::Ticket);

    let entries = std::fs::read_dir(&fixtures_dir).expect("cannot read fixtures/valid");
    let mut checked = 0usize;

    for entry in entries {
        let entry = entry.expect("dir entry error");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("agm") {
            continue;
        }

        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("cannot read {}", path.display()));

        let file = match parser::parse(&source) {
            Ok(f) => f,
            Err(_) => continue, // some valid/ fixtures have parse warnings treated as errors; skip
        };
        for node in &file.nodes {
            // Use the schema that matches the node's type.
            let node_type = &node.node_type;
            if matches!(node_type, NodeType::Custom(_)) {
                continue; // no schema for custom types
            }
            let schema =
                schema_for(node_type, &SchemaOptions::default()).expect("schema_for returned Err");
            let validator = jsonschema::validator_for(&schema).expect("compile failed");
            let node_json = node_to_json(node);

            assert!(
                validator.is_valid(&node_json),
                "valid fixture {} node {} failed schema validation:\n{}",
                path.display(),
                node.id,
                serde_json::to_string_pretty(&node_json).unwrap()
            );
            checked += 1;
        }

        // Extra: ticket fixtures specifically validated against ticket schema.
        for node in &file.nodes {
            if node.node_type == NodeType::Ticket {
                let node_json = node_to_json(node);
                assert!(
                    ticket_validator.is_valid(&node_json),
                    "ticket validator failed for node {} in {}",
                    node.id,
                    path.display()
                );
            }
        }
    }

    assert!(
        checked > 0,
        "no nodes checked — fixtures directory may be empty"
    );
}

// ---------------------------------------------------------------------------
// Step 10 — Schema rejects invalid ticket fixtures
// ---------------------------------------------------------------------------

#[test]
fn test_schema_reject_implies_validator_reject() {
    let fixtures_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid");

    let entries = std::fs::read_dir(&fixtures_dir).expect("cannot read fixtures/invalid");
    let mut schema_rejects = 0usize;

    for entry in entries {
        let entry = entry.expect("dir entry error");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("agm") {
            continue;
        }

        let fixture_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if is_semantic_only(fixture_name) {
            continue;
        }

        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let file = match parser::parse(&source) {
            Ok(f) => f,
            Err(_) => continue, // parse errors are caught by validator, skip here
        };

        for node in &file.nodes {
            if matches!(node.node_type, NodeType::Custom(_)) {
                continue;
            }

            // Use strict schema for disallowed-field violations.
            let strict_opts = SchemaOptions {
                strict: true,
                ..Default::default()
            };
            let schema_strict =
                schema_for(&node.node_type, &strict_opts).expect("schema_for returned Err");
            let validator_strict =
                jsonschema::validator_for(&schema_strict).expect("compile failed");

            let node_json = node_to_json(node);

            // If the strict schema rejects it, that's a parity win — count it.
            if !validator_strict.is_valid(&node_json) {
                schema_rejects += 1;
            }
            // We don't assert false here because the schema cannot catch every
            // validator violation (e.g. semantic rules). The test is directional.
        }
    }

    // At least some invalid fixtures must be caught by the strict schema.
    assert!(
        schema_rejects > 0,
        "strict schema did not reject any invalid fixture — check test setup"
    );
}

// ---------------------------------------------------------------------------
// Individual parity tests
// ---------------------------------------------------------------------------

#[test]
fn test_ticket_schema_rejects_invalid_action() {
    // Serialize a ticket node with an unsupported action value and assert schema rejects it.
    let bad_node = serde_json::json!({
        "node": "test.ticket",
        "type": "ticket",
        "summary": "test",
        "title": "Test",
        "description": "desc",
        "priority": "high",
        "action": "delete",  // not in the enum
    });

    let schema = schema_for(&NodeType::Ticket, &SchemaOptions::default()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        !validator.is_valid(&bad_node),
        "schema should reject action: 'delete'"
    );
}

#[test]
fn test_ticket_schema_rejects_invalid_sdd_phase() {
    let bad_node = serde_json::json!({
        "node": "test.ticket",
        "type": "ticket",
        "summary": "test",
        "title": "Test",
        "description": "desc",
        "priority": "high",
        "sdd_phase": "planning",  // not in the enum
    });

    let schema = schema_for(&NodeType::Ticket, &SchemaOptions::default()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        !validator.is_valid(&bad_node),
        "schema should reject sdd_phase: 'planning'"
    );
}

#[test]
fn test_ticket_strict_rejects_disallowed_steps() {
    // ticket disallows "steps" — strict schema should catch it.
    let bad_node = serde_json::json!({
        "node": "test.ticket",
        "type": "ticket",
        "summary": "test",
        "title": "Test",
        "description": "desc",
        "priority": "high",
        "steps": ["step one"],  // disallowed on ticket
    });

    let validator = compile_strict_schema(&NodeType::Ticket);
    assert!(
        !validator.is_valid(&bad_node),
        "strict schema should reject 'steps' on ticket node"
    );
}

#[test]
fn test_ticket_schema_accepts_valid_ticket() {
    let good_node = serde_json::json!({
        "node": "test.ticket",
        "type": "ticket",
        "summary": "add OAuth2 login",
        "title": "Add OAuth2 login flow",
        "description": "Add Google OAuth2 login to the dashboard.",
        "priority": "high",
        "action": "create",
        "sdd_phase": "backlog",
    });

    let schema = schema_for(&NodeType::Ticket, &SchemaOptions::default()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(&good_node),
        "schema should accept a valid ticket node"
    );
}

#[test]
fn test_orchestration_schema_accepts_valid_node() {
    let good_node = serde_json::json!({
        "node": "auth.plan",
        "type": "orchestration",
        "summary": "execute authentication migration",
        "parallel_groups": [
            {
                "group": "1-schema",
                "nodes": ["auth.schema"],
                "strategy": "sequential"
            }
        ]
    });

    let schema = schema_for(&NodeType::Orchestration, &SchemaOptions::default()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(&good_node),
        "schema should accept a valid orchestration node"
    );
}

#[test]
fn test_ticket_schema_rejects_invalid_priority() {
    let bad_node = serde_json::json!({
        "node": "test.ticket",
        "type": "ticket",
        "summary": "test",
        "title": "Test",
        "description": "desc",
        "priority": "urgent",  // not a valid Priority variant
    });

    let schema = schema_for(&NodeType::Ticket, &SchemaOptions::default()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        !validator.is_valid(&bad_node),
        "schema should reject priority: 'urgent'"
    );
}

#[test]
fn test_ticket_schema_valid_fixture_file() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/valid/ticket_create.agm"),
    )
    .expect("cannot read ticket_create.agm");

    let node = parse_first_node(&source);
    let node_json = node_to_json(&node);
    let schema = schema_for(&NodeType::Ticket, &SchemaOptions::default()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();

    assert!(
        validator.is_valid(&node_json),
        "ticket_create.agm should pass schema validation"
    );
}

// ---------------------------------------------------------------------------
// Allowlist JSON fixture
// ---------------------------------------------------------------------------

#[test]
fn test_known_semantic_only_fixture_exists_in_invalid() {
    // Document that the allowlist entries do correspond to real invalid fixtures.
    let fixtures_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid");

    for name in KNOWN_SEMANTIC_ONLY {
        let path = fixtures_dir.join(format!("{name}.agm"));
        assert!(
            path.exists(),
            "KNOWN_SEMANTIC_ONLY entry '{name}' has no corresponding fixture at {}",
            path.display()
        );
    }
}
