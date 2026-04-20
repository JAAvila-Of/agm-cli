//! Group I — JSON render round-trip.
//!
//! Tests that `renderer::json::render_json` produces valid JSON for each node
//! type, and that the key set matches canonical model fields.

use agm_core::builder::{
    CodeBlockBuilder, DecisionBuilder, FactsBuilder, OrchestrationBuilder, RulesBuilder,
    TicketBuilder, VerifyCheckBuilder, WorkflowBuilder,
};
use agm_core::model::fields::Priority;
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::renderer::json::render_json;

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

fn wrap_single_node(node: agm_core::model::node::Node, package: &str) -> AgmFile {
    AgmFile {
        header: test_header(package),
        nodes: vec![node],
    }
}

fn assert_valid_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s)
        .unwrap_or_else(|e| panic!("render_json produced invalid JSON: {e}\nOutput: {s}"))
}

fn get_first_node(value: &serde_json::Value) -> &serde_json::Value {
    value
        .get("nodes")
        .and_then(|n| n.as_array())
        .and_then(|a| a.first())
        .expect("JSON must contain at least one node")
}

// ============================================================================
// I1 — render_json produces valid JSON for each node type
// ============================================================================

#[test]
fn test_json_render_facts_node_is_valid_json() {
    let node = FactsBuilder::new("json.facts.a")
        .summary("facts for JSON test")
        .items(["item one", "item two"])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.facts.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    let node_json = get_first_node(&value);
    assert_eq!(node_json["node"], "json.facts.a");
    assert_eq!(node_json["type"], "facts");
    assert_eq!(node_json["summary"], "facts for JSON test");
    assert!(node_json["items"].is_array(), "items must be JSON array");
    assert_eq!(node_json["items"].as_array().unwrap().len(), 2);
}

#[test]
fn test_json_render_rules_node_is_valid_json() {
    let node = RulesBuilder::new("json.rules.a")
        .summary("rules for JSON test")
        .items(["require HTTPS", "rate-limit"])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.rules.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    let node_json = get_first_node(&value);
    assert_eq!(node_json["type"], "rules");
    assert!(node_json["items"].is_array());
}

#[test]
fn test_json_render_workflow_node_is_valid_json() {
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/lib.rs")
        .body("fn auth() {}")
        .build()
        .unwrap();

    let check = VerifyCheckBuilder::command("cargo test")
        .expect("exit_code_0")
        .build()
        .unwrap();

    let node = WorkflowBuilder::new("json.workflow.a")
        .summary("workflow for JSON test")
        .steps(["step one", "step two"])
        .input(["host"])
        .output(["result"])
        .code_blocks([cb])
        .verify([check])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.workflow.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    let node_json = get_first_node(&value);
    assert_eq!(node_json["type"], "workflow");
    assert!(node_json["steps"].is_array(), "steps must be array");
    assert!(
        node_json["code_blocks"].is_array(),
        "code_blocks must be array"
    );
    assert!(node_json["verify"].is_array(), "verify must be array");
}

#[test]
fn test_json_render_decision_node_is_valid_json() {
    let node = DecisionBuilder::new("json.decision.a")
        .summary("decision for JSON test")
        .rationale(["ACID required", "expertise"])
        .tradeoffs(["complex scaling"])
        .resolution(["use PostgreSQL"])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.decision.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    let node_json = get_first_node(&value);
    assert_eq!(node_json["type"], "decision");
    assert!(node_json["rationale"].is_array());
    assert!(node_json["tradeoffs"].is_array());
    assert!(node_json["resolution"].is_array());
}

#[test]
fn test_json_render_ticket_node_is_valid_json() {
    let node = TicketBuilder::new("json.ticket.a")
        .summary("ticket for JSON test")
        .title("JSON Ticket")
        .description("Description for JSON test.")
        .priority(Priority::High)
        .labels(["auth", "backend"])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.ticket.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    let node_json = get_first_node(&value);
    assert_eq!(node_json["type"], "ticket");
    assert_eq!(node_json["title"], "JSON Ticket");
    assert_eq!(node_json["priority"], "high");
    assert!(node_json["labels"].is_array());
    assert_eq!(node_json["labels"].as_array().unwrap().len(), 2);
}

#[test]
fn test_json_render_orchestration_node_is_valid_json() {
    let group = ParallelGroup {
        group: "g1".to_owned(),
        nodes: vec!["json.orch.a".to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    };

    let node = OrchestrationBuilder::new("json.orch.a")
        .summary("orchestration for JSON test")
        .parallel_groups([group])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.orch.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    let node_json = get_first_node(&value);
    assert_eq!(node_json["type"], "orchestration");
    assert!(
        node_json["parallel_groups"].is_array(),
        "parallel_groups must be array"
    );
    assert_eq!(node_json["parallel_groups"].as_array().unwrap().len(), 1);
}

// ============================================================================
// I2 — JSON key set matches canonical model fields
// ============================================================================

#[test]
fn test_json_render_key_set_matches_canonical_fields_for_ticket() {
    let node = TicketBuilder::new("json.keys.ticket")
        .summary("key set test")
        .title("Key Set Test")
        .description("desc")
        .priority(Priority::Normal)
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.keys.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);
    let node_json = get_first_node(&value);

    let obj = node_json.as_object().expect("node must be JSON object");

    // Required keys that must be present for a ticket
    assert!(obj.contains_key("node"), "must have 'node' key (the ID)");
    assert!(obj.contains_key("type"), "must have 'type' key");
    assert!(obj.contains_key("summary"), "must have 'summary' key");
    assert!(
        obj.contains_key("title"),
        "must have 'title' key for ticket"
    );
    assert!(
        obj.contains_key("description"),
        "must have 'description' key for ticket"
    );
    assert!(
        obj.contains_key("priority"),
        "must have 'priority' key for ticket"
    );

    // Optional keys NOT set should be absent (serde skip_serializing_if = None)
    assert!(
        !obj.contains_key("steps"),
        "workflow-only 'steps' must not appear on ticket"
    );
    assert!(
        !obj.contains_key("rationale"),
        "decision-only 'rationale' must not appear on ticket"
    );
    assert!(
        !obj.contains_key("parallel_groups"),
        "orch-only field must not appear on ticket"
    );
}

#[test]
fn test_json_render_optional_fields_absent_when_not_set() {
    // A minimal facts node should not have optional fields in JSON output
    let node = FactsBuilder::new("json.optional.facts")
        .summary("minimal facts for JSON")
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.optional.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);
    let node_json = get_first_node(&value);
    let obj = node_json.as_object().unwrap();

    // Optional fields not set must be absent
    assert!(
        !obj.contains_key("items"),
        "items must be absent when not set"
    );
    assert!(
        !obj.contains_key("detail"),
        "detail must be absent when not set"
    );
    assert!(
        !obj.contains_key("notes"),
        "notes must be absent when not set"
    );
    assert!(
        !obj.contains_key("tags"),
        "tags must be absent when not set"
    );
    assert!(
        !obj.contains_key("depends"),
        "depends must be absent when not set"
    );
}

// ============================================================================
// I3 — JSON header fields present
// ============================================================================

#[test]
fn test_json_render_file_contains_header_and_nodes() {
    let node = FactsBuilder::new("json.header.test")
        .summary("header in JSON test")
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.header.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    // AgmFile uses #[serde(flatten)] for Header, so header fields appear at the
    // top level rather than nested under a "header" key.
    let obj = value.as_object().expect("top level must be object");
    assert!(obj.contains_key("nodes"), "JSON must contain 'nodes' field");
    assert!(obj.contains_key("agm"), "JSON must contain flattened 'agm' field");
    assert!(obj.contains_key("package"), "JSON must contain flattened 'package' field");
    assert!(obj.contains_key("version"), "JSON must contain flattened 'version' field");

    assert_eq!(value["agm"], "1.0");
    assert_eq!(value["package"], "json.header.pkg");
    assert_eq!(value["version"], "1.0.0");

    let nodes = value["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1, "must have 1 node");
}

// ============================================================================
// I4 — Multi-node JSON output is valid and has correct node count
// ============================================================================

#[test]
fn test_json_render_multi_node_file_is_valid_json_with_correct_count() {
    let facts = FactsBuilder::new("json.multi.facts")
        .summary("facts")
        .items(["i"])
        .build_unchecked()
        .unwrap();
    let rules = RulesBuilder::new("json.multi.rules")
        .summary("rules")
        .items(["r"])
        .build_unchecked()
        .unwrap();
    let workflow = WorkflowBuilder::new("json.multi.wf")
        .summary("workflow")
        .build_unchecked()
        .unwrap();

    let file = AgmFile {
        header: test_header("json.multi.pkg"),
        nodes: vec![facts, rules, workflow],
    };

    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);
    let nodes = value["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3, "must have 3 nodes in JSON output");

    // Types are correct
    assert_eq!(nodes[0]["type"], "facts");
    assert_eq!(nodes[1]["type"], "rules");
    assert_eq!(nodes[2]["type"], "workflow");
}

// ============================================================================
// I5 — JSON strategy field renders as string
// ============================================================================

#[test]
fn test_json_render_parallel_group_strategy_as_string() {
    use agm_core::builder::OrchestrationBuilder;

    let g1 = ParallelGroup {
        group: "seq-group".to_owned(),
        nodes: vec!["json.strat.test".to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    };
    let g2 = ParallelGroup {
        group: "par-group".to_owned(),
        nodes: vec!["json.strat.test".to_owned()],
        strategy: Strategy::Parallel,
        requires: Some(vec!["seq-group".to_owned()]),
        max_concurrency: Some(3),
    };

    let node = OrchestrationBuilder::new("json.strat.test")
        .summary("strategy JSON test")
        .parallel_groups([g1, g2])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "json.strat.pkg");
    let json_str = render_json(&file);
    let value = assert_valid_json(&json_str);

    let node_json = get_first_node(&value);
    let groups = node_json["parallel_groups"].as_array().unwrap();

    assert_eq!(groups[0]["strategy"], "sequential");
    assert_eq!(groups[1]["strategy"], "parallel");
    assert_eq!(groups[1]["max_concurrency"], 3);
    assert!(groups[1]["requires"].is_array());
}

// ============================================================================
// I6 — JSON shape snapshot (flat header — stable public contract lock)
// ============================================================================

/// Locks the JSON output shape for a minimal file.
///
/// The snapshot captures the flat shape (`agm`, `package`, `version` at the
/// top level alongside `nodes`). If `AgmFile::header` is ever un-flattened,
/// this snapshot diff will catch the breaking change.
#[test]
fn test_json_shape_flat_header_snapshot() {
    use agm_core::builder::FactsBuilder;
    use insta::assert_snapshot;

    let node = FactsBuilder::new("snap.shape.node")
        .summary("shape lock node")
        .items(["item one"])
        .build()
        .unwrap();

    let file = wrap_single_node(node, "snap.shape.pkg");
    let json_str = render_json(&file);

    // Parse and re-serialize to get deterministic key ordering.
    let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    assert_snapshot!("json_flat_shape", pretty);
}
