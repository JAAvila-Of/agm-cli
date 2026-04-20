//! Tests for diff detection of structured fields (memory, verify, code_blocks,
//! parallel_groups, agent_context) and rendering 200-node diffs in all formats.

use agm_core::diff::{self, render::DiffFormat, render::render_diff};
use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::context::AgentContext;
use agm_core::model::fields::{NodeType, Span};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::memory::{MemoryAction, MemoryEntry};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::verify::VerifyCheck;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn valid_header(package: &str) -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: package.to_owned(),
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

fn minimal_node(id: &str, node_type: NodeType, summary: &str, line: usize) -> Node {
    Node {
        id: id.to_owned(),
        node_type,
        summary: summary.to_owned(),
        span: Span::new(line, line + 3),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 6.1 Structured field changes
// ---------------------------------------------------------------------------

#[test]
fn test_diff_memory_field_change_detected() {
    let mut left_node = minimal_node("mem.node", NodeType::Facts, "memory node", 1);
    left_node.memory = Some(vec![MemoryEntry {
        key: "repo.pattern".to_owned(),
        topic: "rust.repository".to_owned(),
        action: MemoryAction::Upsert,
        value: Some("value A".to_owned()),
        scope: None,
        ttl: None,
        query: None,
        max_results: None,
        extra_fields: Default::default(),
    }]);

    let mut right_node = left_node.clone();
    right_node.memory = Some(vec![MemoryEntry {
        key: "repo.pattern".to_owned(),
        topic: "rust.repository".to_owned(),
        action: MemoryAction::Upsert,
        value: Some("value B".to_owned()), // changed
        scope: None,
        ttl: None,
        query: None,
        max_results: None,
        extra_fields: Default::default(),
    }]);

    let left = AgmFile {
        header: valid_header("test.mem"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.mem"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);

    assert_eq!(
        report.modified_nodes.len(),
        1,
        "memory node should be in modified_nodes"
    );
    assert_eq!(report.modified_nodes[0].node_id, "mem.node");

    let memory_change = report.modified_nodes[0]
        .field_changes
        .iter()
        .find(|fc| fc.field == "memory");
    assert!(
        memory_change.is_some(),
        "memory field change should be detected"
    );
}

#[test]
fn test_diff_verify_field_change_detected() {
    let mut left_node = minimal_node("verify.node", NodeType::Facts, "verify node", 1);
    left_node.verify = Some(vec![VerifyCheck::FileExists {
        file: "a.txt".to_owned(),
    }]);

    let mut right_node = left_node.clone();
    right_node.verify = Some(vec![VerifyCheck::FileExists {
        file: "b.txt".to_owned(), // changed
    }]);

    let left = AgmFile {
        header: valid_header("test.verify"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.verify"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);

    assert_eq!(
        report.modified_nodes.len(),
        1,
        "verify node should be in modified_nodes"
    );

    let verify_change = report.modified_nodes[0]
        .field_changes
        .iter()
        .find(|fc| fc.field == "verify");
    assert!(
        verify_change.is_some(),
        "verify field change should be detected"
    );
}

#[test]
fn test_diff_code_blocks_field_change_detected() {
    let mut left_node = minimal_node("code.node", NodeType::Facts, "code node", 1);
    left_node.code_blocks = Some(vec![CodeBlock {
        lang: Some("rust".to_owned()),
        target: None,
        action: CodeAction::Full,
        body: "fn main() {}".to_owned(),
        anchor: None,
        old: None,
    }]);

    let mut right_node = left_node.clone();
    right_node.code_blocks = Some(vec![
        CodeBlock {
            lang: Some("rust".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "fn main() {}".to_owned(),
            anchor: None,
            old: None,
        },
        CodeBlock {
            lang: Some("rust".to_owned()),
            target: Some("src/lib.rs".to_owned()),
            action: CodeAction::Append,
            body: "pub fn extra() {}".to_owned(),
            anchor: None,
            old: None,
        },
    ]);

    let left = AgmFile {
        header: valid_header("test.code_blocks"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.code_blocks"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);

    assert_eq!(
        report.modified_nodes.len(),
        1,
        "code node should be in modified_nodes"
    );

    let code_blocks_change = report.modified_nodes[0]
        .field_changes
        .iter()
        .find(|fc| fc.field == "code_blocks");
    assert!(
        code_blocks_change.is_some(),
        "code_blocks field change should be detected"
    );
}

#[test]
fn test_diff_parallel_groups_field_change_detected() {
    let mut left_node = minimal_node("orch.node", NodeType::Facts, "orchestration node", 1);
    left_node.parallel_groups = Some(vec![ParallelGroup {
        group: "1-setup".to_owned(),
        nodes: vec!["setup.a".to_owned()],
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    }]);

    let mut right_node = left_node.clone();
    right_node.parallel_groups = Some(vec![
        ParallelGroup {
            group: "1-setup".to_owned(),
            nodes: vec!["setup.a".to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        },
        ParallelGroup {
            group: "2-core".to_owned(),
            nodes: vec!["core.b".to_owned(), "core.c".to_owned()],
            strategy: Strategy::Parallel,
            requires: Some(vec!["1-setup".to_owned()]),
            max_concurrency: None,
        },
    ]);

    let left = AgmFile {
        header: valid_header("test.parallel"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.parallel"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);

    assert_eq!(
        report.modified_nodes.len(),
        1,
        "orchestration node should be in modified_nodes"
    );

    let pg_change = report.modified_nodes[0]
        .field_changes
        .iter()
        .find(|fc| fc.field == "parallel_groups");
    assert!(
        pg_change.is_some(),
        "parallel_groups field change should be detected"
    );
}

#[test]
fn test_diff_agent_context_field_change_detected() {
    let mut left_node = minimal_node("agent.node", NodeType::Facts, "agent node", 1);
    left_node.agent_context = Some(AgentContext {
        load_nodes: None,
        load_files: None,
        system_hint: Some("hint A".to_owned()),
        max_tokens: None,
        load_memory: None,
    });

    let mut right_node = left_node.clone();
    right_node.agent_context = Some(AgentContext {
        load_nodes: None,
        load_files: None,
        system_hint: Some("hint B".to_owned()), // changed
        max_tokens: None,
        load_memory: None,
    });

    let left = AgmFile {
        header: valid_header("test.agent"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.agent"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);

    assert_eq!(
        report.modified_nodes.len(),
        1,
        "agent node should be in modified_nodes"
    );

    let ctx_change = report.modified_nodes[0]
        .field_changes
        .iter()
        .find(|fc| fc.field == "agent_context");
    assert!(
        ctx_change.is_some(),
        "agent_context field change should be detected"
    );
}

// ---------------------------------------------------------------------------
// 6.2 All 3 output formats on 200-node diff
// ---------------------------------------------------------------------------

fn make_200_node_diff() -> (AgmFile, AgmFile) {
    let left_nodes: Vec<Node> = (0..200usize)
        .map(|i| {
            minimal_node(
                &format!("pkg.n{i:03}"),
                NodeType::Facts,
                &format!("original summary {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let right_nodes: Vec<Node> = (0..200usize)
        .map(|i| {
            minimal_node(
                &format!("pkg.n{i:03}"),
                NodeType::Facts,
                &format!("updated summary {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let left = AgmFile {
        header: valid_header("stress.render"),
        nodes: left_nodes,
    };
    let right = AgmFile {
        header: valid_header("stress.render"),
        nodes: right_nodes,
    };
    (left, right)
}

#[test]
fn test_diff_render_text_200_nodes_modified_not_empty() {
    let (left, right) = make_200_node_diff();
    let report = diff::diff(&left, &right);
    assert_eq!(report.summary.nodes_modified, 200);

    let output = render_diff(&report, DiffFormat::Text);

    assert!(
        !output.is_empty(),
        "text render of 200-node diff should not be empty"
    );
    assert!(
        output.contains("=== AGM Semantic Diff ==="),
        "text output must contain the header marker"
    );
    assert!(
        output.contains("--- Nodes Modified (200) ---"),
        "text output must mention 200 modified nodes"
    );
}

#[test]
fn test_diff_render_json_200_nodes_modified_valid_json() {
    let (left, right) = make_200_node_diff();
    let report = diff::diff(&left, &right);
    assert_eq!(report.summary.nodes_modified, 200);

    let output = render_diff(&report, DiffFormat::Json);

    let parsed: agm_core::diff::DiffReport =
        serde_json::from_str(&output).expect("JSON output of 200-node diff must be valid JSON");
    assert_eq!(
        parsed.summary.nodes_modified, 200,
        "deserialized JSON must report 200 modified nodes"
    );
    assert_eq!(parsed.modified_nodes.len(), 200);
}

#[test]
fn test_diff_render_markdown_200_nodes_modified_contains_headers() {
    let (left, right) = make_200_node_diff();
    let report = diff::diff(&left, &right);
    assert_eq!(report.summary.nodes_modified, 200);

    let output = render_diff(&report, DiffFormat::Markdown);

    assert!(
        output.contains("# AGM Semantic Diff"),
        "markdown must contain top-level header"
    );
    assert!(
        output.contains("## Summary"),
        "markdown must contain Summary section"
    );
    assert!(
        output.contains("## Modified Nodes"),
        "markdown must contain Modified Nodes section"
    );
}
