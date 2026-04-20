//! Exhaustive branch coverage for `renderer/canonical.rs`.
//!
//! Exercises every field emitter: imports, load_profiles (with both
//! TokenEstimate variants), all code/code_blocks branches, all verify
//! check variants, all load_nodes/load_files/load_memory inline-vs-block
//! branches, multi-line system_hint, memory entries (all optional
//! fields), parallel_groups with long requires/nodes lists, retry_count,
//! extra_fields (Scalar/List/Block), and roundtrip through the parser.

use std::collections::BTreeMap;

use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::context::{AgentContext, FileRange, LoadFile};
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::fields::{Confidence, FieldValue, NodeStatus, NodeType, Priority, Stability};
use agm_core::model::file::{AgmFile, Header, LoadProfile, TokenEstimate};
use agm_core::model::imports::ImportEntry;
use agm_core::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::verify::VerifyCheck;
use agm_core::parser::parse;
use agm_core::renderer::canonical::render_canonical;

fn blank_node(id: &str, t: NodeType) -> Node {
    Node {
        id: id.to_owned(),
        node_type: t,
        summary: format!("summary for {id}"),
        ..Default::default()
    }
}

/// Full header with imports + load_profiles (both TokenEstimate variants).
fn full_header() -> Header {
    let mut profiles = BTreeMap::new();
    profiles.insert(
        "summary".to_owned(),
        LoadProfile {
            filter: "type in [facts]".to_owned(),
            estimated_tokens: Some(TokenEstimate::Count(1200)),
        },
    );
    profiles.insert(
        "full".to_owned(),
        LoadProfile {
            filter: "*".to_owned(),
            estimated_tokens: Some(TokenEstimate::Variable),
        },
    );

    Header {
        agm: "1.0".to_owned(),
        package: "test.canonical".to_owned(),
        version: "1.2.3".to_owned(),
        title: Some("Canonical fixture".to_owned()),
        owner: Some("team-x".to_owned()),
        imports: Some(vec![
            ImportEntry::new("pkg.one".to_owned(), Some("^1.0.0".to_owned())),
            ImportEntry::new("pkg.two".to_owned(), None),
        ]),
        default_load: Some("operational".to_owned()),
        description: Some("Multi-line\ndescription\nblock".to_owned()),
        tags: Some(vec!["alpha".to_owned(), "beta".to_owned()]),
        status: Some("stable".to_owned()),
        load_profiles: Some(profiles),
        target_runtime: Some("rust-1.80".to_owned()),
    }
}

/// Node populating every field emitted by canonical — including the
/// rarely-used branches: code with anchor+old, code_blocks with lang/no-lang,
/// memory entry with all optional fields, parallel_groups with >3 nodes and
/// a requires list of >3, long load_nodes/load_memory lists, multi-line
/// system_hint, and extra_fields covering all three FieldValue variants.
fn kitchen_sink_node() -> Node {
    let mut n = blank_node("mod.kitchen", NodeType::Workflow);
    n.priority = Some(Priority::High);
    n.stability = Some(Stability::Medium);
    n.confidence = Some(Confidence::High);
    n.status = Some(NodeStatus::Draft);
    n.depends = Some(vec!["a".to_owned(), "b".to_owned()]);
    n.related_to = Some(vec!["c".to_owned()]);
    n.replaces = Some(vec!["old".to_owned()]);
    n.conflicts = Some(vec!["rival".to_owned()]);
    n.see_also = Some(vec!["ref".to_owned()]);
    n.input = Some(vec!["ia".to_owned()]);
    n.output = Some(vec!["oa".to_owned()]);
    n.items = Some(vec!["i1".to_owned(), "i2".to_owned()]);
    n.steps = Some(vec!["s1".to_owned()]);
    n.fields = Some(vec!["f1: int".to_owned()]);

    // code with ALL optional fields (lang, target, anchor, old)
    n.code = Some(CodeBlock {
        lang: Some("rust".to_owned()),
        target: Some("src/main.rs".to_owned()),
        action: CodeAction::Replace,
        body: "fn main() {}\nfn other() {}".to_owned(),
        anchor: Some("// ANCHOR".to_owned()),
        old: Some("old_line1\nold_line2".to_owned()),
    });

    // code_blocks: mix of lang/no-lang + anchor/old
    n.code_blocks = Some(vec![
        CodeBlock {
            lang: Some("bash".to_owned()),
            target: Some("run.sh".to_owned()),
            action: CodeAction::Full,
            body: "echo hi".to_owned(),
            anchor: Some("# anchor".to_owned()),
            old: Some("echo old".to_owned()),
        },
        CodeBlock {
            lang: None,
            target: None,
            action: CodeAction::Create,
            body: "raw body".to_owned(),
            anchor: None,
            old: None,
        },
    ]);

    // verify — all five variants
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
            pattern: "agm".to_owned(),
        },
        VerifyCheck::FileNotContains {
            file: "src/lib.rs".to_owned(),
            pattern: "todo".to_owned(),
        },
        VerifyCheck::NodeStatus {
            node: "other.node".to_owned(),
            status: "completed".to_owned(),
        },
    ]);

    // agent_context — long lists force block form, multi-line system_hint
    n.agent_context = Some(AgentContext {
        load_nodes: Some(vec![
            "ctx.a".to_owned(),
            "ctx.b".to_owned(),
            "ctx.c".to_owned(),
            "ctx.d".to_owned(),
            "ctx.e".to_owned(),
        ]),
        load_files: Some(vec![
            LoadFile {
                path: "src/main.rs".to_owned(),
                range: FileRange::Full,
            },
            LoadFile {
                path: "src/util.rs".to_owned(),
                range: FileRange::Lines(1, 50),
            },
            LoadFile {
                path: "src/work.rs".to_owned(),
                range: FileRange::Function("do_work".to_owned()),
            },
        ]),
        system_hint: Some("hint line 1\nhint line 2".to_owned()), // multi-line
        max_tokens: Some(8000),
        load_memory: Some(vec![
            "topic.a".to_owned(),
            "topic.b".to_owned(),
            "topic.c".to_owned(),
            "topic.d".to_owned(),
        ]),
    });

    n.target = Some("tg".to_owned());

    // memory with all optional fields
    n.memory = Some(vec![MemoryEntry {
        key: "k1".to_owned(),
        topic: "t1".to_owned(),
        action: MemoryAction::Search,
        value: Some("v1".to_owned()),
        scope: Some(MemoryScope::Global),
        ttl: Some(MemoryTtl::Duration("P2D".to_owned())),
        query: Some("q1".to_owned()),
        max_results: Some(10),
    }]);

    // execution state
    n.execution_status = Some(ExecutionStatus::Completed);
    n.executed_by = Some("agent-1".to_owned());
    n.executed_at = Some("2026-04-12T12:34:56Z".to_owned());
    n.execution_log = Some("log line 1\nlog line 2".to_owned());
    n.retry_count = Some(2);

    // parallel_groups with >3 nodes AND >3 requires (forces block form)
    n.parallel_groups = Some(vec![ParallelGroup {
        group: "g1".to_owned(),
        nodes: vec![
            "n1".to_owned(),
            "n2".to_owned(),
            "n3".to_owned(),
            "n4".to_owned(),
        ],
        strategy: Strategy::Parallel,
        requires: Some(vec![
            "r1".to_owned(),
            "r2".to_owned(),
            "r3".to_owned(),
            "r4".to_owned(),
        ]),
        max_concurrency: Some(3),
    }]);

    // explanatory
    n.detail = Some("Line A\nLine B".to_owned()); // multi-line → block form
    n.rationale = Some(vec!["r".to_owned()]);
    n.tradeoffs = Some(vec!["t".to_owned()]);
    n.resolution = Some(vec!["x".to_owned()]);
    n.examples = Some("ex1".to_owned());
    n.notes = Some("ex note".to_owned());

    // context
    n.scope = Some(vec!["s1".to_owned()]);
    n.applies_when = Some("always".to_owned());
    n.valid_from = Some("2026-01-01".to_owned());
    n.valid_until = Some("2027-01-01".to_owned());
    n.tags = Some(vec!["tg1".to_owned()]);
    n.aliases = Some(vec!["al".to_owned()]);
    n.keywords = Some(vec!["kw".to_owned()]);

    // extra_fields — all 3 variants
    n.extra_fields.insert(
        "x_scalar".to_owned(),
        FieldValue::Scalar("scalarval".to_owned()),
    );
    n.extra_fields.insert(
        "x_block".to_owned(),
        FieldValue::Block("block line one\nblock line two".to_owned()),
    );
    n.extra_fields.insert(
        "x_list".to_owned(),
        FieldValue::List(vec!["l1".to_owned(), "l2".to_owned()]),
    );

    n
}

fn kitchen_sink_file() -> AgmFile {
    AgmFile {
        header: full_header(),
        nodes: vec![kitchen_sink_node()],
    }
}

#[test]
fn test_canonical_kitchen_sink_emits_and_is_reparseable() {
    let file = kitchen_sink_file();
    let out = render_canonical(&file);

    // Header
    assert!(out.starts_with("agm: 1.0\n"));
    assert!(out.contains("package: test.canonical"));
    assert!(out.contains("title: Canonical fixture"));
    assert!(out.contains("imports:\n  - pkg.one@^1.0.0"));
    assert!(out.contains("  - pkg.two\n"));
    assert!(out.contains("load_profiles:"));
    assert!(out.contains("    filter: type in [facts]"));
    assert!(out.contains("    estimated_tokens: 1200"));
    assert!(out.contains("    estimated_tokens: variable"));

    // Node pieces
    assert!(out.contains("retry_count: 2"));
    assert!(out.contains("x_scalar: scalarval"));
    assert!(out.contains("x_block:\n"));
    assert!(out.contains("block line one"));
    assert!(out.contains("x_list: "));

    // verify (all five)
    for frag in [
        "type: command",
        "type: file_exists",
        "type: file_contains",
        "type: file_not_contains",
        "type: node_status",
    ] {
        assert!(out.contains(frag), "missing verify fragment: {frag}");
    }

    // code with anchor + old
    assert!(out.contains("  anchor: // ANCHOR"));
    assert!(out.contains("  old:\n    old_line1"));

    // parallel_groups block list forms (>3 triggers multi-line)
    assert!(out.contains("    nodes:\n      - n1"));
    assert!(out.contains("    requires:\n      - r1"));
    assert!(out.contains("    max_concurrency: 3"));

    // agent_context >3 load_nodes / load_memory → block form
    assert!(out.contains("  load_nodes:\n    - ctx.a"));
    assert!(out.contains("  load_memory:\n    - topic.a"));

    // system_hint multi-line → block form
    assert!(out.contains("  system_hint:\n"));
    assert!(out.contains("    hint line 1"));

    // load_files ranges — all 3 variants
    assert!(out.contains("      range: full"));
    assert!(out.contains("      range: 1-50"));
    assert!(out.contains("      range: function: do_work"));

    // memory entry with all optional fields
    assert!(out.contains("    value: v1"));
    assert!(out.contains("    scope: global"));
    assert!(out.contains("    ttl: duration:P2D"));
    assert!(out.contains("    query: q1"));
    assert!(out.contains("    max_results: 10"));

    // Re-parse succeeds
    let reparsed = parse(&out)
        .unwrap_or_else(|e| panic!("canonical output did not round-trip:\n{out}\nerrors: {e:?}"));
    assert_eq!(reparsed.nodes.len(), 1);
    assert_eq!(reparsed.nodes[0].id, "mod.kitchen");
}

#[test]
fn test_canonical_emits_empty_body_code_block() {
    // Code with empty body triggers the fallback `"    \n"` line.
    let mut n = blank_node("empty.code", NodeType::Facts);
    n.code = Some(CodeBlock {
        lang: None,
        target: None,
        action: CodeAction::Full,
        body: String::new(),
        anchor: None,
        old: None,
    });
    let file = AgmFile {
        header: Header {
            agm: "1.0".to_owned(),
            package: "t".to_owned(),
            version: "0".to_owned(),
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
        nodes: vec![n],
    };
    let out = render_canonical(&file);
    assert!(out.contains("code:\n"));
    assert!(out.contains("  action: full"));
    assert!(out.contains("  body: |2\n"));
}

#[test]
fn test_canonical_empty_vecs_are_skipped() {
    let mut n = blank_node("empty.vecs", NodeType::Facts);
    n.items = Some(vec![]);
    n.depends = Some(vec![]);
    n.code_blocks = Some(vec![]);
    n.verify = Some(vec![]);
    n.memory = Some(vec![]);
    n.parallel_groups = Some(vec![]);

    let mut header = full_header();
    header.imports = Some(vec![]); // also empty
    header.load_profiles = Some(BTreeMap::new());

    let file = AgmFile {
        header,
        nodes: vec![n],
    };
    let out = render_canonical(&file);

    // No empty sections should appear
    assert!(!out.contains("items:"));
    assert!(!out.contains("depends:"));
    assert!(!out.contains("code_blocks:"));
    assert!(!out.contains("verify:"));
    assert!(!out.contains("memory:"));
    assert!(!out.contains("parallel_groups:"));
    assert!(!out.contains("imports:"));
    assert!(!out.contains("load_profiles:"));
}

#[test]
fn test_canonical_trailing_newline_always_present() {
    let file = kitchen_sink_file();
    let out = render_canonical(&file);
    assert!(out.ends_with('\n'));
}

#[test]
fn test_canonical_scope_list_preserved() {
    let mut n = blank_node("c.scope", NodeType::Facts);
    n.scope = Some(vec!["global".to_owned(), "team".to_owned()]);
    let file = AgmFile {
        header: full_header(),
        nodes: vec![n],
    };
    let out = render_canonical(&file);
    assert!(out.contains("scope: [global, team]"));
}
