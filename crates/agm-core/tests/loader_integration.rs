//! Integration tests: loader field filtering and load profile resolution.
//!
//! Tests cover:
//!  1.1  Each LoadMode strips/includes the correct fields on a parsed fixture.
//!  1.2  Loader at scale: 200+ nodes.
//!  1.3  Load profiles at scale.

use std::collections::BTreeMap;

use agm_core::loader::{self, LoadMode};
use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::fields::{NodeType, Priority, Span};
use agm_core::model::file::{AgmFile, Header, LoadProfile};
use agm_core::model::node::Node;
use agm_core::parser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture(relative: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("tests/fixtures")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

fn parse_fixture(relative: &str) -> AgmFile {
    let src = fixture(relative);
    parser::parse(&src).unwrap_or_else(|errs| {
        panic!(
            "parse failed for {relative}: {:?}",
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>()
        )
    })
}

fn minimal_header() -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: "test.scale".to_owned(),
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
    }
}

fn make_node(id: &str) -> Node {
    Node {
        id: id.to_owned(),
        node_type: NodeType::Facts,
        summary: format!("summary for {id}"),
        priority: Some(Priority::High),
        items: Some(vec!["item1".to_owned()]),
        detail: Some("full detail text".to_owned()),
        code: Some(CodeBlock {
            lang: Some("bash".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "echo hi".to_owned(),
            anchor: None,
            old: None,
        }),
        execution_status: Some(ExecutionStatus::Pending),
        tags: Some(vec!["tag1".to_owned()]),
        span: Span::new(1, 5),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 1.1  Field filtering per mode on the fully_populated_node fixture
// ---------------------------------------------------------------------------

#[test]
fn test_load_summary_mode_strips_operational_executable_full_fields() {
    let file = parse_fixture("valid/fully_populated_node.agm");
    let result = loader::load(&file, LoadMode::Summary);

    // auth.login is the first node.
    let node = result.nodes.iter().find(|n| n.id == "auth.login").unwrap();

    // --- Summary fields present ---
    assert_eq!(node.id, "auth.login");
    assert_eq!(node.node_type, NodeType::Workflow);
    assert!(!node.summary.is_empty());
    assert!(
        node.priority.is_some(),
        "priority should be present in Summary"
    );
    assert!(
        node.stability.is_some(),
        "stability should be present in Summary"
    );
    assert!(
        node.depends.is_some(),
        "depends should be present in Summary"
    );
    assert!(node.tags.is_some(), "tags should be present in Summary");

    // --- Operational fields absent ---
    assert!(node.items.is_none(), "items must be absent in Summary");
    assert!(node.steps.is_none(), "steps must be absent in Summary");
    assert!(node.fields.is_none(), "fields must be absent in Summary");
    assert!(node.input.is_none(), "input must be absent in Summary");
    assert!(node.output.is_none(), "output must be absent in Summary");

    // --- Executable fields absent ---
    assert!(node.code.is_none(), "code must be absent in Summary");
    assert!(node.verify.is_none(), "verify must be absent in Summary");
    assert!(
        node.agent_context.is_none(),
        "agent_context must be absent in Summary"
    );
    assert!(
        node.execution_status.is_none(),
        "execution_status must be absent in Summary"
    );
    assert!(node.memory.is_none(), "memory must be absent in Summary");

    // --- Full fields absent ---
    assert!(
        node.confidence.is_none(),
        "confidence must be absent in Summary"
    );
    assert!(node.detail.is_none(), "detail must be absent in Summary");
    assert!(
        node.rationale.is_none(),
        "rationale must be absent in Summary"
    );
    assert!(
        node.related_to.is_none(),
        "related_to must be absent in Summary"
    );
    assert!(node.notes.is_none(), "notes must be absent in Summary");
}

#[test]
fn test_load_operational_mode_includes_operational_strips_executable_full() {
    let file = parse_fixture("valid/fully_populated_node.agm");
    let result = loader::load(&file, LoadMode::Operational);

    let node = result.nodes.iter().find(|n| n.id == "auth.login").unwrap();

    // --- Operational fields present ---
    assert!(
        node.items.is_some(),
        "items should be present in Operational"
    );
    assert!(
        node.steps.is_some(),
        "steps should be present in Operational"
    );
    assert!(
        node.fields.is_some(),
        "fields should be present in Operational"
    );
    assert!(
        node.input.is_some(),
        "input should be present in Operational"
    );
    assert!(
        node.output.is_some(),
        "output should be present in Operational"
    );

    // --- Executable fields absent ---
    assert!(node.code.is_none(), "code must be absent in Operational");
    assert!(
        node.verify.is_none(),
        "verify must be absent in Operational"
    );
    assert!(
        node.execution_status.is_none(),
        "execution_status must be absent in Operational"
    );

    // --- Full fields absent ---
    assert!(
        node.confidence.is_none(),
        "confidence must be absent in Operational"
    );
    assert!(
        node.detail.is_none(),
        "detail must be absent in Operational"
    );
    assert!(
        node.related_to.is_none(),
        "related_to must be absent in Operational"
    );
}

#[test]
fn test_load_executable_mode_includes_executable_strips_full() {
    let file = parse_fixture("valid/fully_populated_node.agm");
    let result = loader::load(&file, LoadMode::Executable);

    let node = result.nodes.iter().find(|n| n.id == "auth.login").unwrap();

    // --- Executable fields present ---
    assert!(node.code.is_some(), "code should be present in Executable");
    assert!(
        node.verify.is_some(),
        "verify should be present in Executable"
    );
    assert!(
        node.agent_context.is_some(),
        "agent_context should be present in Executable"
    );
    assert!(
        node.execution_status.is_some(),
        "execution_status should be present in Executable"
    );
    assert!(
        node.memory.is_some(),
        "memory should be present in Executable"
    );

    // --- Full fields absent ---
    assert!(
        node.confidence.is_none(),
        "confidence must be absent in Executable"
    );
    assert!(node.detail.is_none(), "detail must be absent in Executable");
    assert!(
        node.related_to.is_none(),
        "related_to must be absent in Executable"
    );
    assert!(node.notes.is_none(), "notes must be absent in Executable");
    assert!(
        node.parallel_groups.is_none(),
        "parallel_groups must be absent in Executable"
    );
}

#[test]
fn test_load_full_mode_includes_all_fields() {
    let file = parse_fixture("valid/fully_populated_node.agm");
    let result = loader::load(&file, LoadMode::Full);

    let node = result.nodes.iter().find(|n| n.id == "auth.login").unwrap();

    // Summary
    assert!(node.priority.is_some());
    assert!(node.stability.is_some());
    assert!(node.depends.is_some());
    assert!(node.tags.is_some());

    // Operational
    assert!(node.items.is_some());
    assert!(node.steps.is_some());
    assert!(node.fields.is_some());
    assert!(node.input.is_some());
    assert!(node.output.is_some());

    // Executable
    assert!(node.code.is_some());
    assert!(node.verify.is_some());
    assert!(node.agent_context.is_some());
    assert!(node.execution_status.is_some());
    assert!(node.memory.is_some());

    // Full-only
    assert!(node.confidence.is_some());
    assert!(node.detail.is_some());
    assert!(node.rationale.is_some());
    assert!(node.related_to.is_some());
    assert!(node.notes.is_some());
    assert!(node.parallel_groups.is_some());
    assert!(node.aliases.is_some());
    assert!(node.keywords.is_some());
    assert!(node.scope.is_some());
    assert!(node.applies_when.is_some());
    assert!(node.valid_from.is_some());
    assert!(node.valid_until.is_some());
}

// ---------------------------------------------------------------------------
// 1.2  Loader at scale: 200+ nodes
// ---------------------------------------------------------------------------

fn make_file_with_n_nodes(n: usize) -> AgmFile {
    let nodes = (0..n).map(|i| make_node(&format!("node.{i:04}"))).collect();
    AgmFile {
        header: minimal_header(),
        nodes,
    }
}

#[test]
fn test_load_200_nodes_summary_mode_all_nodes_present_fields_correct() {
    let file = make_file_with_n_nodes(200);
    let result = loader::load(&file, LoadMode::Summary);

    assert_eq!(result.nodes.len(), 200, "all 200 nodes must be present");

    for node in &result.nodes {
        assert!(!node.id.is_empty(), "node id must be populated");
        assert!(!node.summary.is_empty(), "summary must be populated");
        assert!(node.items.is_none(), "items must be absent in Summary");
        assert!(node.detail.is_none(), "detail must be absent in Summary");
        assert!(node.code.is_none(), "code must be absent in Summary");
    }
}

#[test]
fn test_load_200_nodes_full_mode_all_fields_preserved() {
    let file = make_file_with_n_nodes(200);
    let result = loader::load(&file, LoadMode::Full);

    assert_eq!(result.nodes.len(), 200, "all 200 nodes must be present");

    for node in &result.nodes {
        assert!(node.items.is_some(), "items must be present in Full");
        assert!(node.detail.is_some(), "detail must be present in Full");
        assert!(node.code.is_some(), "code must be present in Full");
    }
}

// ---------------------------------------------------------------------------
// 1.3  Load profiles at scale
// ---------------------------------------------------------------------------

/// Builds a file with `n` nodes; nodes are assigned types and priorities
/// in a round-robin pattern so that profiles can select distinct subsets.
fn make_profiled_file(n: usize) -> AgmFile {
    let types = [
        NodeType::Workflow,
        NodeType::Facts,
        NodeType::Rules,
        NodeType::Decision,
    ];
    let priorities = [
        Priority::Critical,
        Priority::High,
        Priority::Normal,
        Priority::Low,
    ];

    let nodes: Vec<Node> = (0..n)
        .map(|i| {
            let mut node = make_node(&format!("node.{i:04}"));
            node.node_type = types[i % types.len()].clone();
            node.priority = Some(priorities[i % priorities.len()].clone());
            node.tags = Some(vec![format!("tag{}", i % 5)]);
            node
        })
        .collect();

    AgmFile {
        header: minimal_header(),
        nodes,
    }
}

/// Creates a `BTreeMap<String, LoadProfile>` with `n` named profiles.
///
/// Each profile filters by a different type (cycling through 4 types).
fn make_10_profiles() -> BTreeMap<String, LoadProfile> {
    let filter_types = [
        "workflow", "facts", "rules", "decision", "workflow", "facts", "rules", "decision",
        "workflow", "facts",
    ];

    (0..10)
        .map(|i| {
            let name = format!("profile_{i:02}");
            let filter = format!("type in [{}]", filter_types[i]);
            (
                name,
                LoadProfile {
                    filter,
                    estimated_tokens: None,
                },
            )
        })
        .collect()
}

#[test]
fn test_load_profile_10_named_profiles_each_filters_correctly() {
    let mut file = make_profiled_file(20);
    file.header.load_profiles = Some(make_10_profiles());

    // Precompute expected counts for each profile.
    // Nodes 0..20, node_type = types[i % 4]:
    // workflow = i in {0,4,8,12,16}     = 5 nodes
    // facts    = i in {1,5,9,13,17}     = 5 nodes
    // rules    = i in {2,6,10,14,18}    = 5 nodes
    // decision = i in {3,7,11,15,19}    = 5 nodes
    let expected_type_counts = [
        ("workflow", 5usize),
        ("facts", 5),
        ("rules", 5),
        ("decision", 5),
        ("workflow", 5),
        ("facts", 5),
        ("rules", 5),
        ("decision", 5),
        ("workflow", 5),
        ("facts", 5),
    ];

    for (i, (expected_type_str, expected_count)) in expected_type_counts.iter().enumerate() {
        let profile_name = format!("profile_{i:02}");
        let result = loader::load_profile(&file, Some(&profile_name))
            .unwrap_or_else(|e| panic!("profile {profile_name} failed: {e}"));

        assert_eq!(
            result.nodes.len(),
            *expected_count,
            "profile {profile_name} should return {expected_count} nodes of type {expected_type_str}"
        );

        let expected_node_type: NodeType = expected_type_str.parse().unwrap();
        for node in &result.nodes {
            assert_eq!(
                node.node_type, expected_node_type,
                "profile {profile_name}: unexpected node type {:?} for node {}",
                node.node_type, node.id
            );
        }
    }
}

#[test]
fn test_load_profile_default_load_with_10_profiles_resolves_correctly() {
    let mut file = make_profiled_file(20);
    let profiles = make_10_profiles();
    // Set default_load to profile_02 which filters by type = rules.
    file.header.default_load = Some("profile_02".to_owned());
    file.header.load_profiles = Some(profiles);

    // Calling load_profile(file, None) should resolve to profile_02.
    let result = loader::load_profile(&file, None)
        .expect("load_profile with None should resolve default_load");

    // profile_02 = "type in [rules]" → 5 nodes of type Rules.
    assert_eq!(
        result.nodes.len(),
        5,
        "default profile should return 5 rules nodes"
    );

    for node in &result.nodes {
        assert_eq!(
            node.node_type,
            NodeType::Rules,
            "all returned nodes should be type rules, got {:?} for {}",
            node.node_type,
            node.id
        );
    }
}
