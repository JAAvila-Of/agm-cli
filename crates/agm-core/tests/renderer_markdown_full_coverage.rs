//! Exhaustive branch coverage for `renderer/markdown.rs`.
//!
//! Exercises every optional field, every verify variant, every FileRange
//! variant, the custom node-type path, and execution-state rendering.

use std::collections::BTreeMap;

use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::context::{AgentContext, FileRange, LoadFile};
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::fields::{Confidence, NodeStatus, NodeType, Priority, Span, Stability};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::imports::ImportEntry;
use agm_core::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::verify::VerifyCheck;
use agm_core::renderer::markdown::render_markdown;

fn full_header() -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: "test.full".to_owned(),
        version: "1.2.3".to_owned(),
        title: Some("Full Coverage Fixture".to_owned()),
        owner: Some("team-x".to_owned()),
        imports: Some(vec![ImportEntry::new(
            "shared.lib".to_owned(),
            Some("^1.0.0".to_owned()),
        )]),
        default_load: Some("operational".to_owned()),
        description: Some("A rich description line.".to_owned()),
        tags: Some(vec!["alpha".to_owned(), "beta".to_owned()]),
        status: Some("stable".to_owned()),
        load_profiles: None,
        target_runtime: Some("rust-1.80".to_owned()),
    }
}

fn blank_node(id: &str, node_type: NodeType) -> Node {
    Node {
        id: id.to_owned(),
        node_type,
        summary: format!("summary for {id}"),
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

/// A single node populating EVERY optional field supported by the markdown
/// renderer. One test against this node exercises the majority of the
/// branch conditions in `render_node_section`.
fn kitchen_sink_node() -> Node {
    let mut n = blank_node("mod.kitchen", NodeType::Workflow);
    n.priority = Some(Priority::High);
    n.stability = Some(Stability::Medium);
    n.confidence = Some(Confidence::High);
    n.status = Some(NodeStatus::Draft);
    n.tags = Some(vec!["foo".to_owned(), "bar".to_owned()]);
    n.depends = Some(vec!["other.dep".to_owned()]);
    n.related_to = Some(vec!["other.rel".to_owned()]);
    n.replaces = Some(vec!["old.node".to_owned()]);
    n.conflicts = Some(vec!["rival.node".to_owned()]);
    n.see_also = Some(vec!["ref.node".to_owned()]);
    n.input = Some(vec!["input.a".to_owned(), "input.b".to_owned()]);
    n.output = Some(vec!["output.x".to_owned()]);
    n.detail = Some("Detailed explanation of the kitchen sink node.".to_owned());
    n.items = Some(vec!["item1".to_owned(), "item2".to_owned()]);
    n.steps = Some(vec!["step1".to_owned(), "step2".to_owned()]);
    n.fields = Some(vec!["id: string".to_owned(), "name: string".to_owned()]);
    n.rationale = Some(vec!["rationale1".to_owned()]);
    n.tradeoffs = Some(vec!["tradeoff1".to_owned()]);
    n.resolution = Some(vec!["resolution1".to_owned()]);
    n.examples = Some("Block of examples text.".to_owned());
    n.notes = Some("Block of notes text.".to_owned());

    // Code with target + anchor + body WITHOUT trailing newline
    n.code = Some(CodeBlock {
        lang: Some("rust".to_owned()),
        target: Some("src/lib.rs".to_owned()),
        action: CodeAction::Replace,
        body: "fn hello() {}".to_owned(), // no trailing newline
        anchor: Some("// ANCHOR".to_owned()),
        old: None,
    });

    // code_blocks
    n.code_blocks = Some(vec![
        CodeBlock {
            lang: Some("bash".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "echo hi\n".to_owned(),
            anchor: None,
            old: None,
        },
        CodeBlock {
            lang: None,
            target: None,
            action: CodeAction::Full,
            body: "plain text".to_owned(),
            anchor: None,
            old: None,
        },
    ]);

    // verify — all 5 variants
    n.verify = Some(vec![
        VerifyCheck::Command {
            run: "cargo test".to_owned(),
            expect: Some("exit_code_0".to_owned()),
        },
        VerifyCheck::Command {
            run: "echo noexpect".to_owned(),
            expect: None,
        },
        VerifyCheck::FileExists {
            file: "Cargo.toml".to_owned(),
        },
        VerifyCheck::FileContains {
            file: "README.md".to_owned(),
            pattern: "agm-cli".to_owned(),
        },
        VerifyCheck::FileNotContains {
            file: "src/lib.rs".to_owned(),
            pattern: "todo!".to_owned(),
        },
        VerifyCheck::NodeStatus {
            node: "other.node".to_owned(),
            status: "completed".to_owned(),
        },
    ]);

    // Agent context — all 3 optional fields
    n.agent_context = Some(AgentContext {
        load_nodes: Some(vec!["ctx.a".to_owned(), "ctx.b".to_owned()]),
        load_files: Some(vec![
            LoadFile {
                path: "src/main.rs".to_owned(),
                range: FileRange::Full,
            },
            LoadFile {
                path: "src/util.rs".to_owned(),
                range: FileRange::Lines(10, 20),
            },
            LoadFile {
                path: "src/other.rs".to_owned(),
                range: FileRange::Function("do_work".to_owned()),
            },
        ]),
        system_hint: Some("Be concise.".to_owned()),
        max_tokens: Some(8000),
        load_memory: None,
    });

    // Memory entries
    n.memory = Some(vec![
        MemoryEntry {
            key: "sess.greeting".to_owned(),
            topic: "demo".to_owned(),
            action: MemoryAction::Upsert,
            value: Some("hello".to_owned()),
            scope: Some(MemoryScope::Session),
            ttl: Some(MemoryTtl::Session),
            query: None,
            max_results: None,
        },
        MemoryEntry {
            key: "proj.config".to_owned(),
            topic: "demo".to_owned(),
            action: MemoryAction::Get,
            value: None,
            scope: Some(MemoryScope::Project),
            ttl: None,
            query: None,
            max_results: None,
        },
    ]);

    // Parallel groups
    n.parallel_groups = Some(vec![ParallelGroup {
        group: "phase1".to_owned(),
        nodes: vec!["a".to_owned(), "b".to_owned()],
        strategy: Strategy::Parallel,
        requires: None,
        max_concurrency: Some(2),
    }]);

    // Execution state — all 4 fields
    n.execution_status = Some(ExecutionStatus::Completed);
    n.executed_by = Some("agent-1".to_owned());
    n.executed_at = Some("2026-04-12T12:34:56Z".to_owned());
    n.retry_count = Some(3);

    n
}

/// File with full header + every built-in node type + one custom type.
fn kitchen_sink_file() -> AgmFile {
    let mut nodes = vec![kitchen_sink_node()];

    // One of every built-in node type so every type_display_name branch and
    // the custom-type branch are exercised.
    for t in [
        NodeType::Facts,
        NodeType::Rules,
        NodeType::Entity,
        NodeType::Decision,
        NodeType::Exception,
        NodeType::Example,
        NodeType::Glossary,
        NodeType::AntiPattern,
        NodeType::Orchestration,
    ] {
        nodes.push(blank_node(&format!("n.{}", t), t));
    }

    // Custom type (hits the `Custom(_)` branch + `capitalize_first` + custom
    // types section iteration)
    nodes.push(blank_node(
        "n.custom",
        NodeType::Custom("policy".to_owned()),
    ));

    AgmFile {
        header: full_header(),
        nodes,
    }
}

#[test]
fn test_markdown_renders_full_header_block() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);

    assert!(out.starts_with("# test.full v1.2.3\n"));
    assert!(out.contains("> Full Coverage Fixture"));
    assert!(out.contains("A rich description line."));

    // Properties table — all 5 rows must appear.
    assert!(out.contains("| Property | Value |"));
    assert!(out.contains("| Owner | team-x |"));
    assert!(out.contains("| Status | stable |"));
    assert!(out.contains("| Default load | operational |"));
    assert!(out.contains("| Tags | alpha, beta |"));
    assert!(out.contains("| Target runtime | rust-1.80 |"));
}

#[test]
fn test_markdown_renders_imports_section() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    assert!(out.contains("## Imports"));
    assert!(out.contains("- `shared.lib@^1.0.0`"));
}

#[test]
fn test_markdown_all_builtin_type_headers_present() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);

    for header in [
        "## Facts",
        "## Rules",
        "## Workflow",
        "## Entity",
        "## Decision",
        "## Exception",
        "## Example",
        "## Glossary",
        "## Anti-Pattern",
        "## Orchestration",
    ] {
        assert!(out.contains(header), "missing header: {header}");
    }
}

#[test]
fn test_markdown_custom_type_section_and_capitalization() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    // capitalize_first("policy") -> "Policy"
    assert!(out.contains("## Policy"), "custom type header missing");
    assert!(out.contains("n.custom"), "custom node id missing");
}

#[test]
fn test_markdown_control_table_has_all_four_fields() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);

    assert!(out.contains("| Control | Value |"));
    assert!(out.contains("| Priority | high |"));
    assert!(out.contains("| Stability | medium |"));
    assert!(out.contains("| Confidence | high |"));
    assert!(out.contains("| Status | draft |"));
}

#[test]
fn test_markdown_node_tags_line() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    assert!(out.contains("**Tags**: foo, bar"));
}

#[test]
fn test_markdown_all_relationships_rendered() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    assert!(out.contains("**Depends on**: `other.dep`"));
    assert!(out.contains("**Related to**: `other.rel`"));
    assert!(out.contains("**Replaces**: `old.node`"));
    assert!(out.contains("**Conflicts with**: `rival.node`"));
    assert!(out.contains("**See also**: `ref.node`"));
}

#[test]
fn test_markdown_input_output_lines() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    assert!(out.contains("**Input**: `input.a`, `input.b`"));
    assert!(out.contains("**Output**: `output.x`"));
}

#[test]
fn test_markdown_structured_list_sections_present() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);

    for heading in [
        "#### Detail",
        "#### Items",
        "#### Steps",
        "#### Fields",
        "#### Rationale",
        "#### Tradeoffs",
        "#### Resolution",
        "#### Examples",
        "#### Notes",
        "#### Code",
        "#### Code Blocks",
        "#### Verification",
        "#### Agent Context",
        "#### Memory",
        "#### Parallel Groups",
        "#### Execution State",
    ] {
        assert!(out.contains(heading), "missing heading: {heading}");
    }

    // Steps are rendered as numbered list
    assert!(out.contains("1. step1"));
    assert!(out.contains("2. step2"));
}

#[test]
fn test_markdown_code_block_with_target_and_anchor_and_no_trailing_newline() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);

    assert!(out.contains("**File**: `src/lib.rs`"));
    assert!(out.contains("**Anchor**: `// ANCHOR`"));
    assert!(out.contains("**Action**: replace"));
    // body "fn hello() {}" had no trailing newline — renderer must add one
    assert!(out.contains("fn hello() {}\n```"));
}

#[test]
fn test_markdown_all_verify_variants_rendered() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);

    assert!(out.contains("`cargo test`"));
    assert!(out.contains("(expect: exit_code_0)"));
    assert!(out.contains("`echo noexpect`"));
    assert!(out.contains("File exists: `Cargo.toml`"));
    assert!(out.contains("`README.md` contains `agm-cli`"));
    assert!(out.contains("`src/lib.rs` does NOT contain `todo!`"));
    assert!(out.contains("Node `other.node` has status `completed`"));
}

#[test]
fn test_markdown_agent_context_all_file_range_variants() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);

    assert!(out.contains("**Load nodes**: `ctx.a`, `ctx.b`"));
    assert!(out.contains("**Load files**"));
    assert!(out.contains("`src/main.rs` (range: full)"));
    assert!(out.contains("`src/util.rs` (range: 10-20)"));
    assert!(out.contains("`src/other.rs` (range: function: do_work)"));
    assert!(out.contains("**System hint**: Be concise."));
}

#[test]
fn test_markdown_memory_table_rendered() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    assert!(out.contains("| Key | Topic | Action | Value |"));
    assert!(out.contains("| sess.greeting | demo | upsert | hello |"));
    // null value renders as "-"
    assert!(out.contains("| proj.config | demo | get | - |"));
}

#[test]
fn test_markdown_parallel_groups_table_rendered() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    assert!(out.contains("| Group | Nodes | Strategy |"));
    assert!(out.contains("| phase1 | a, b | parallel |"));
}

#[test]
fn test_markdown_execution_state_all_four_fields_rendered() {
    let file = kitchen_sink_file();
    let out = render_markdown(&file);
    assert!(out.contains("- **Status**: completed"));
    assert!(out.contains("- **Executed by**: agent-1"));
    assert!(out.contains("- **Executed at**: 2026-04-12T12:34:56Z"));
    assert!(out.contains("- **Retry count**: 3"));
}

/// A file with NO optional fields anywhere — guards the empty-path branches.
#[test]
fn test_markdown_bare_minimum_file_omits_optional_sections() {
    let header = Header {
        agm: "1.0".to_owned(),
        package: "test.bare".to_owned(),
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
    let file = AgmFile {
        header,
        nodes: vec![blank_node("bare.node", NodeType::Facts)],
    };
    let out = render_markdown(&file);

    assert!(!out.contains("| Property | Value |"));
    assert!(!out.contains("## Imports"));
    assert!(!out.contains("| Control | Value |"));
    assert!(!out.contains("**Tags**"));
    assert!(!out.contains("**Depends on**"));
    assert!(!out.contains("#### Detail"));
    assert!(!out.contains("#### Agent Context"));
    assert!(!out.contains("#### Execution State"));
}

/// Empty optional Vecs must NOT produce their section headings.
#[test]
fn test_markdown_empty_vec_optionals_omit_their_sections() {
    let mut n = blank_node("empty.vecs", NodeType::Facts);
    n.tags = Some(vec![]);
    n.depends = Some(vec![]);
    n.input = Some(vec![]);
    n.output = Some(vec![]);
    n.items = Some(vec![]);
    n.steps = Some(vec![]);
    n.fields = Some(vec![]);
    n.rationale = Some(vec![]);
    n.tradeoffs = Some(vec![]);
    n.resolution = Some(vec![]);
    n.code_blocks = Some(vec![]);
    n.verify = Some(vec![]);
    n.parallel_groups = Some(vec![]);
    n.memory = Some(vec![]);

    let mut header = full_header();
    header.imports = Some(vec![]); // empty imports guarded separately

    let file = AgmFile {
        header,
        nodes: vec![n],
    };
    let out = render_markdown(&file);

    assert!(!out.contains("**Tags**"));
    assert!(!out.contains("**Depends on**"));
    assert!(!out.contains("**Input**"));
    assert!(!out.contains("**Output**"));
    assert!(!out.contains("#### Items"));
    assert!(!out.contains("#### Steps"));
    assert!(!out.contains("#### Fields"));
    assert!(!out.contains("#### Rationale"));
    assert!(!out.contains("#### Tradeoffs"));
    assert!(!out.contains("#### Resolution"));
    assert!(!out.contains("#### Code Blocks"));
    assert!(!out.contains("#### Verification"));
    assert!(!out.contains("#### Parallel Groups"));
    assert!(!out.contains("#### Memory"));
    assert!(!out.contains("## Imports"));
}
