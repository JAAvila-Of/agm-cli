//! Renderer at-scale integration tests — Gap 4.
//!
//! Covers:
//!  4.1 Markdown render with 20+ nodes of mixed types
//!  4.2 render_mem / render_state with large sidecar files
//!  4.3 Canonical renderer determinism and roundtrip

use std::collections::BTreeMap;

use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::fields::{NodeType, Span};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::mem_file::{MemFile, MemFileEntry};
use agm_core::model::memory::{MemoryScope, MemoryTtl};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::state::{NodeState, StateFile};
use agm_core::model::verify::VerifyCheck;
use agm_core::parser::parse;
use agm_core::renderer::canonical::render_canonical;
use agm_core::renderer::mem::{render_mem, render_mem_json};
use agm_core::renderer::state::{render_state, render_state_json};
use agm_core::renderer::{render, RenderFormat};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn base_header() -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: "test.scale".to_owned(),
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

/// Build a minimal node with the given id and type.
fn make_node(id: &str, node_type: NodeType) -> Node {
    Node {
        id: id.to_owned(),
        node_type,
        summary: format!("Summary for {id}"),
        priority: None,
        stability: None,
        confidence: None,
        status: None,
        depends: None,
        related_to: None,
        replaces: None,
        conflicts: None,
        see_also: None,
        items: None,
        steps: None,
        fields: None,
        input: None,
        output: None,
        detail: None,
        rationale: None,
        tradeoffs: None,
        resolution: None,
        examples: None,
        notes: None,
        code: None,
        code_blocks: None,
        verify: None,
        agent_context: None,
        target: None,
        execution_status: None,
        executed_by: None,
        executed_at: None,
        execution_log: None,
        retry_count: None,
        parallel_groups: None,
        memory: None,
        scope: None,
        applies_when: None,
        valid_from: None,
        valid_until: None,
        tags: None,
        aliases: None,
        keywords: None,
        extra_fields: BTreeMap::new(),
        span: Span::default(),
    }
}

/// Build a code block.
fn make_code_block(lang: &str, body: &str) -> CodeBlock {
    CodeBlock {
        lang: Some(lang.to_owned()),
        target: None,
        action: CodeAction::Full,
        body: body.to_owned(),
        anchor: None,
        old: None,
    }
}

/// Build a `MemFileEntry`.
fn make_mem_entry(topic: &str, scope: MemoryScope, ttl: MemoryTtl, value: &str) -> MemFileEntry {
    MemFileEntry {
        topic: topic.to_owned(),
        scope,
        ttl,
        value: value.to_owned(),
        created_at: "2026-04-12T10:00:00Z".to_owned(),
        updated_at: "2026-04-12T10:00:00Z".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// 4.1 Markdown render — 20 nodes of mixed types
// ---------------------------------------------------------------------------

/// Build a rich node with items, detail, code, and verify populated.
fn rich_node(id: &str, node_type: NodeType) -> Node {
    let mut n = make_node(id, node_type);
    n.items = Some(vec![
        format!("{id}.item_a"),
        format!("{id}.item_b"),
    ]);
    n.detail = Some(format!("Detail text for {id}."));
    n.code = Some(make_code_block("rust", &format!("fn {id}() {{}}")));
    n.verify = Some(vec![
        VerifyCheck::Command {
            run: format!("check {id}"),
            expect: Some("exit_code_0".to_owned()),
        },
        VerifyCheck::FileExists {
            file: format!("{id}.txt"),
        },
    ]);
    n
}

/// Builds an AgmFile with 20 nodes: 4 each of facts, rules, workflow, entity,
/// orchestration — each with summary, items, detail, code, and verify.
fn file_20_mixed_nodes() -> AgmFile {
    let types = [
        NodeType::Facts,
        NodeType::Rules,
        NodeType::Workflow,
        NodeType::Entity,
        NodeType::Orchestration,
    ];

    let mut nodes: Vec<Node> = Vec::with_capacity(20);
    for (ti, node_type) in types.iter().enumerate() {
        for i in 0..4 {
            let id = format!("{}.node_{i}", node_type);
            nodes.push(rich_node(&id, node_type.clone()));
            let _ = ti;
        }
    }

    AgmFile {
        header: base_header(),
        nodes,
    }
}

#[test]
fn test_render_markdown_20_nodes_mixed_types_groups_correctly() {
    let file = file_20_mixed_nodes();
    let output = render(&file, RenderFormat::Markdown);

    // All type group headers must be present.
    assert!(output.contains("## Facts"), "missing Facts section");
    assert!(output.contains("## Rules"), "missing Rules section");
    assert!(output.contains("## Workflow"), "missing Workflow section");
    assert!(output.contains("## Entity"), "missing Entity section");
    assert!(output.contains("## Orchestration"), "missing Orchestration section");

    // Facts section must come before Rules, Rules before Workflow, etc.
    let facts_pos = output.find("## Facts").unwrap();
    let rules_pos = output.find("## Rules").unwrap();
    let workflow_pos = output.find("## Workflow").unwrap();
    let entity_pos = output.find("## Entity").unwrap();
    let orch_pos = output.find("## Orchestration").unwrap();
    assert!(facts_pos < rules_pos, "Facts must precede Rules");
    assert!(rules_pos < workflow_pos, "Rules must precede Workflow");
    assert!(workflow_pos < entity_pos, "Workflow must precede Entity");
    assert!(entity_pos < orch_pos, "Entity must precede Orchestration");

    // Every node ID must appear in the output.
    for node in &file.nodes {
        assert!(
            output.contains(&node.id),
            "node id '{}' not found in markdown output",
            node.id
        );
    }

    // Code blocks must be fenced (```).
    assert!(output.contains("```rust"), "fenced code block missing");

    // Verify section must appear.
    assert!(output.contains("#### Verification"), "Verification heading missing");
}

#[test]
fn test_render_markdown_orchestration_node_with_parallel_groups() {
    let mut node = make_node("orch.plan", NodeType::Orchestration);
    node.parallel_groups = Some(vec![
        ParallelGroup {
            group: "g1".to_owned(),
            nodes: vec!["step.a".to_owned(), "step.b".to_owned()],
            strategy: Strategy::Parallel,
            requires: None,
            max_concurrency: None,
        },
        ParallelGroup {
            group: "g2".to_owned(),
            nodes: vec!["step.c".to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["g1".to_owned()]),
            max_concurrency: None,
        },
        ParallelGroup {
            group: "g3".to_owned(),
            nodes: vec!["step.d".to_owned(), "step.e".to_owned()],
            strategy: Strategy::Parallel,
            requires: Some(vec!["g2".to_owned()]),
            max_concurrency: Some(2),
        },
    ]);

    let file = AgmFile {
        header: base_header(),
        nodes: vec![node],
    };

    let output = render(&file, RenderFormat::Markdown);

    assert!(
        output.contains("#### Parallel Groups"),
        "Parallel Groups heading missing"
    );
    assert!(output.contains("g1"), "group g1 missing");
    assert!(output.contains("g2"), "group g2 missing");
    assert!(output.contains("g3"), "group g3 missing");
}

#[test]
fn test_render_markdown_node_with_code_blocks_and_verify() {
    let mut node = make_node("impl.service", NodeType::Workflow);
    node.code_blocks = Some(vec![
        make_code_block("rust", "fn alpha() {}"),
        make_code_block("bash", "echo beta"),
        make_code_block("python", "def gamma(): pass"),
    ]);
    node.verify = Some(vec![
        VerifyCheck::Command {
            run: "cargo test".to_owned(),
            expect: Some("exit_code_0".to_owned()),
        },
        VerifyCheck::FileExists {
            file: "target/debug/app".to_owned(),
        },
        VerifyCheck::FileContains {
            file: "Cargo.toml".to_owned(),
            pattern: "agm-core".to_owned(),
        },
        VerifyCheck::FileNotContains {
            file: "src/lib.rs".to_owned(),
            pattern: "todo!()".to_owned(),
        },
        VerifyCheck::NodeStatus {
            node: "impl.service".to_owned(),
            status: "completed".to_owned(),
        },
    ]);

    let file = AgmFile {
        header: base_header(),
        nodes: vec![node],
    };

    let output = render(&file, RenderFormat::Markdown);

    // All three code blocks must appear as fenced blocks.
    assert!(output.contains("```rust"), "rust code block missing");
    assert!(output.contains("```bash"), "bash code block missing");
    assert!(output.contains("```python"), "python code block missing");

    // Code Blocks heading.
    assert!(
        output.contains("#### Code Blocks"),
        "Code Blocks heading missing"
    );

    // All 5 verify checks must appear.
    assert!(output.contains("#### Verification"), "Verification heading missing");
    assert!(output.contains("`cargo test`"), "cargo test verify missing");
    assert!(
        output.contains("target/debug/app"),
        "file_exists verify missing"
    );
    assert!(output.contains("Cargo.toml"), "file_contains verify missing");
    assert!(
        output.contains("todo!()"),
        "file_not_contains verify missing"
    );
    assert!(
        output.contains("impl.service"),
        "node_status verify missing"
    );
}

// ---------------------------------------------------------------------------
// 4.2 render_mem / render_state with large sidecar files
// ---------------------------------------------------------------------------

fn scopes_cycle(i: usize) -> MemoryScope {
    match i % 4 {
        0 => MemoryScope::Project,
        1 => MemoryScope::Session,
        2 => MemoryScope::Global,
        _ => MemoryScope::Node,
    }
}

fn ttls_cycle(i: usize) -> MemoryTtl {
    match i % 3 {
        0 => MemoryTtl::Permanent,
        1 => MemoryTtl::Session,
        _ => MemoryTtl::Duration(format!("P{}D", i + 1)),
    }
}

/// Build a MemFile with `n` distinct entries.
fn build_mem_file(n: usize) -> MemFile {
    let mut entries = BTreeMap::new();
    for i in 0..n {
        let key = format!("entry.key_{i:03}");
        let topic = format!("topic_{}", i % 10);
        let scope = scopes_cycle(i);
        let ttl = ttls_cycle(i);
        let value = format!("value for entry {i}");
        entries.insert(key, make_mem_entry(&topic, scope, ttl, &value));
    }
    MemFile {
        format_version: "1.0".to_owned(),
        package: "test.scale".to_owned(),
        updated_at: "2026-04-12T10:00:00Z".to_owned(),
        entries,
    }
}

#[test]
fn test_render_mem_100_entries_all_present_in_output() {
    let mem = build_mem_file(100);
    let text = render_mem(&mem);

    // Count "entry " occurrences — one per entry block.
    let entry_count = text.matches("\nentry ").count();
    assert_eq!(
        entry_count, 100,
        "expected 100 entry blocks, got {entry_count}"
    );

    // Every key must appear in the text output.
    for key in mem.entries.keys() {
        assert!(
            text.contains(key.as_str()),
            "key '{key}' missing from render_mem output"
        );
    }

    // JSON output must parse and have 100 entries.
    let json = render_mem_json(&mem);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("render_mem_json not valid JSON");
    let entries_obj = parsed["entries"].as_object().expect("entries must be object");
    assert_eq!(
        entries_obj.len(),
        100,
        "expected 100 entries in JSON, got {}",
        entries_obj.len()
    );
}

fn statuses_cycle(i: usize) -> ExecutionStatus {
    match i % 5 {
        0 => ExecutionStatus::Completed,
        1 => ExecutionStatus::Failed,
        2 => ExecutionStatus::Pending,
        3 => ExecutionStatus::Blocked,
        _ => ExecutionStatus::InProgress,
    }
}

/// Build a StateFile with `n` node states in mixed statuses.
fn build_state_file(n: usize) -> StateFile {
    let mut nodes = BTreeMap::new();
    for i in 0..n {
        let node_id = format!("node.state_{i:03}");
        let status = statuses_cycle(i);
        nodes.insert(
            node_id,
            NodeState {
                execution_status: status,
                executed_by: Some(format!("agent-{}", i % 3)),
                executed_at: Some("2026-04-12T10:00:00Z".to_owned()),
                execution_log: None,
                retry_count: (i % 3) as u32,
            },
        );
    }
    StateFile {
        format_version: "1.0".to_owned(),
        package: "test.scale".to_owned(),
        version: "1.0.0".to_owned(),
        session_id: "run-scale-001".to_owned(),
        started_at: "2026-04-12T10:00:00Z".to_owned(),
        updated_at: "2026-04-12T10:00:00Z".to_owned(),
        nodes,
    }
}

#[test]
fn test_render_state_50_nodes_all_present_in_output() {
    let state = build_state_file(50);
    let text = render_state(&state);

    // Count "state " blocks — one per node.
    let state_count = text.matches("\nstate ").count();
    assert_eq!(
        state_count, 50,
        "expected 50 state blocks in text, got {state_count}"
    );

    // Every node id must appear in the text.
    for node_id in state.nodes.keys() {
        assert!(
            text.contains(node_id.as_str()),
            "node_id '{node_id}' missing from render_state output"
        );
    }

    // JSON output must parse and have 50 nodes.
    let json = render_state_json(&state);
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("render_state_json not valid JSON");
    let nodes_obj = parsed["nodes"].as_object().expect("nodes must be object");
    assert_eq!(
        nodes_obj.len(),
        50,
        "expected 50 nodes in JSON, got {}",
        nodes_obj.len()
    );
}

// ---------------------------------------------------------------------------
// 4.3 Canonical renderer determinism
// ---------------------------------------------------------------------------

/// Strip span information so two ASTs with different source positions compare
/// equal. Mirrors the helper used in renderer_roundtrip.rs.
fn strip_spans(file: &AgmFile) -> AgmFile {
    use agm_core::model::fields::Span;
    AgmFile {
        header: file.header.clone(),
        nodes: file
            .nodes
            .iter()
            .map(|n| agm_core::model::node::Node {
                span: Span::default(),
                ..n.clone()
            })
            .collect(),
    }
}

#[test]
fn test_render_canonical_determinism_same_input_same_output() {
    let file = file_20_mixed_nodes();
    let first = render_canonical(&file);

    for i in 1..10 {
        let output = render_canonical(&file);
        assert_eq!(
            first, output,
            "canonical render is not deterministic on iteration {i}"
        );
    }
}

#[test]
fn test_render_canonical_roundtrip_20_nodes() {
    let file = file_20_mixed_nodes();
    let canonical1 = render_canonical(&file);

    let reparsed = parse(&canonical1).unwrap_or_else(|errs| {
        panic!(
            "re-parse of canonical output failed: {errs:?}\n\nOutput:\n{canonical1}"
        )
    });

    let canonical2 = render_canonical(&reparsed);

    // Compare stripped ASTs for semantic equality.
    let stripped_orig = strip_spans(&file);
    let stripped_reparsed = strip_spans(&reparsed);
    assert_eq!(
        stripped_orig, stripped_reparsed,
        "canonical roundtrip produced different AST"
    );

    // Both canonical outputs must be byte-identical.
    assert_eq!(
        canonical1, canonical2,
        "canonical output differs after roundtrip"
    );
}
