//! Exhaustive branch coverage for `renderer/json_canonical.rs`.
//!
//! Split into:
//!  1. Full forward + reverse round trip through `agm_to_json` / `json_to_agm`
//!     on a kitchen-sink file (covers the serde-backed structured
//!     sub-converters: CodeBlock, VerifyCheck, AgentContext, ParallelGroup,
//!     MemoryEntry, LoadProfile).
//!  2. `json_to_agm` error paths: every `InvalidType` / `MissingField` /
//!     `InvalidEnumValue` branch in the helper functions.
//!  3. `extra_fields` reverse conversion for all JSON Value variants
//!     (Bool, Number, String, Array, Null).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::context::{AgentContext, FileRange, LoadFile};
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::fields::{
    Confidence, FieldValue, NodeStatus, NodeType, Priority, Span, Stability,
};
use agm_core::model::file::{AgmFile, Header, LoadProfile, TokenEstimate};
use agm_core::model::imports::ImportEntry;
use agm_core::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::verify::VerifyCheck;
use agm_core::renderer::RenderError;
use agm_core::renderer::json_canonical::{agm_to_json, json_to_agm, render_json_canonical};

fn blank_node(id: &str, t: NodeType) -> Node {
    Node {
        id: id.to_owned(),
        node_type: t,
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

fn kitchen_sink_file() -> AgmFile {
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
    let header = Header {
        agm: "1.0".to_owned(),
        package: "test.json".to_owned(),
        version: "1.2.3".to_owned(),
        title: Some("JSON fixture".to_owned()),
        owner: Some("owner".to_owned()),
        imports: Some(vec![
            ImportEntry::new("a.b".to_owned(), Some("^1.0.0".to_owned())),
            ImportEntry::new("c.d".to_owned(), None),
        ]),
        default_load: Some("operational".to_owned()),
        description: Some("Desc.".to_owned()),
        tags: Some(vec!["t1".to_owned()]),
        status: Some("stable".to_owned()),
        load_profiles: Some(profiles),
        target_runtime: Some("rust-1.80".to_owned()),
    };

    let mut n = blank_node("mod.node", NodeType::Workflow);
    n.priority = Some(Priority::High);
    n.stability = Some(Stability::High);
    n.confidence = Some(Confidence::High);
    n.status = Some(NodeStatus::Active);
    n.depends = Some(vec!["d.a".to_owned()]);
    n.related_to = Some(vec!["r.a".to_owned()]);
    n.replaces = Some(vec!["old".to_owned()]);
    n.conflicts = Some(vec!["x".to_owned()]);
    n.see_also = Some(vec!["y".to_owned()]);
    n.input = Some(vec!["in".to_owned()]);
    n.output = Some(vec!["out".to_owned()]);
    n.items = Some(vec!["i".to_owned()]);
    n.steps = Some(vec!["s".to_owned()]);
    n.fields = Some(vec!["f: str".to_owned()]);
    n.detail = Some("detail".to_owned());
    n.rationale = Some(vec!["r".to_owned()]);
    n.tradeoffs = Some(vec!["t".to_owned()]);
    n.resolution = Some(vec!["res".to_owned()]);
    n.examples = Some("ex".to_owned());
    n.notes = Some("nt".to_owned());
    n.code = Some(CodeBlock {
        lang: Some("rust".to_owned()),
        target: Some("src/l.rs".to_owned()),
        action: CodeAction::Create,
        body: "fn a() {}".to_owned(),
        anchor: Some("// A".to_owned()),
        old: Some("old".to_owned()),
    });
    n.code_blocks = Some(vec![CodeBlock {
        lang: Some("sh".to_owned()),
        target: None,
        action: CodeAction::Full,
        body: "echo".to_owned(),
        anchor: None,
        old: None,
    }]);
    n.verify = Some(vec![
        VerifyCheck::Command {
            run: "cmd".to_owned(),
            expect: Some("e".to_owned()),
        },
        VerifyCheck::FileExists {
            file: "f".to_owned(),
        },
        VerifyCheck::FileContains {
            file: "f".to_owned(),
            pattern: "p".to_owned(),
        },
        VerifyCheck::FileNotContains {
            file: "f".to_owned(),
            pattern: "p".to_owned(),
        },
        VerifyCheck::NodeStatus {
            node: "n".to_owned(),
            status: "completed".to_owned(),
        },
    ]);
    n.agent_context = Some(AgentContext {
        load_nodes: Some(vec!["cn".to_owned()]),
        load_files: Some(vec![
            LoadFile {
                path: "p1".to_owned(),
                range: FileRange::Full,
            },
            LoadFile {
                path: "p2".to_owned(),
                range: FileRange::Lines(1, 5),
            },
            LoadFile {
                path: "p3".to_owned(),
                range: FileRange::Function("f".to_owned()),
            },
        ]),
        system_hint: Some("hint".to_owned()),
        max_tokens: Some(4096),
        load_memory: Some(vec!["m".to_owned()]),
    });
    n.target = Some("tgt".to_owned());
    n.execution_status = Some(ExecutionStatus::Completed);
    n.executed_by = Some("a".to_owned());
    n.executed_at = Some("2026-04-12T10:00:00Z".to_owned());
    n.execution_log = Some("log".to_owned());
    n.retry_count = Some(1);
    n.parallel_groups = Some(vec![ParallelGroup {
        group: "g".to_owned(),
        nodes: vec!["a".to_owned(), "b".to_owned()],
        strategy: Strategy::Parallel,
        requires: Some(vec!["r".to_owned()]),
        max_concurrency: Some(2),
    }]);
    n.memory = Some(vec![MemoryEntry {
        key: "k".to_owned(),
        topic: "t".to_owned(),
        action: MemoryAction::Upsert,
        value: Some("v".to_owned()),
        scope: Some(MemoryScope::Global),
        ttl: Some(MemoryTtl::Permanent),
        query: None,
        max_results: None,
    }]);
    n.scope = Some(vec!["global".to_owned()]);
    n.applies_when = Some("always".to_owned());
    n.valid_from = Some("2026".to_owned());
    n.valid_until = Some("2027".to_owned());
    n.tags = Some(vec!["nt1".to_owned()]);
    n.aliases = Some(vec!["al".to_owned()]);
    n.keywords = Some(vec!["kw".to_owned()]);

    AgmFile {
        header,
        nodes: vec![n],
    }
}

#[test]
fn test_json_canonical_kitchen_sink_roundtrip_preserves_data() {
    let file = kitchen_sink_file();

    // Forward
    let json = agm_to_json(&file);
    assert!(json.is_object());
    // Also exercise render_json_canonical (pretty-printed string form)
    let text = render_json_canonical(&file);
    assert!(text.starts_with('{'));
    // Reverse
    let restored = json_to_agm(&json).expect("reverse must succeed on well-formed data");

    // Span is reset on reverse; strip for equality.
    let strip_span = |f: &AgmFile| AgmFile {
        header: f.header.clone(),
        nodes: f
            .nodes
            .iter()
            .map(|n| Node {
                span: Span::default(),
                ..n.clone()
            })
            .collect(),
    };
    assert_eq!(strip_span(&file), strip_span(&restored));
}

// --- Error-path tests ------------------------------------------------------

#[test]
fn test_json_to_agm_root_not_object_returns_invalid_type() {
    let json = json!(42);
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "<root>"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_missing_agm_returns_missing_field() {
    let json = json!({"package": "p", "version": "1", "nodes": []});
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::MissingField { field } => assert_eq!(field, "agm"),
        other => panic!("expected MissingField, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_agm_wrong_type_returns_invalid_type() {
    let json = json!({"agm": true, "package": "p", "version": "1", "nodes": []});
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "agm"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_agm_as_string_is_accepted() {
    let json = json!({"agm": "1.0", "package": "p", "version": "1", "nodes": []});
    let file = json_to_agm(&json).expect("string agm should be accepted");
    assert_eq!(file.header.agm, "1.0");
}

#[test]
fn test_json_to_agm_package_wrong_type_returns_invalid_type() {
    let json = json!({"agm": 1, "package": 123, "version": "1", "nodes": []});
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "package"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_opt_str_wrong_type_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "title": 42,
        "nodes": []
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "title"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_opt_str_null_treated_as_none() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "title": null, "owner": null,
        "nodes": [{"node": "n", "type": "facts", "summary": "s"}]
    });
    let file = json_to_agm(&json).unwrap();
    assert!(file.header.title.is_none());
    assert!(file.header.owner.is_none());
}

#[test]
fn test_json_to_agm_tags_non_array_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "tags": "alpha",
        "nodes": []
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "tags"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_tags_item_wrong_type_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "tags": ["ok", 42],
        "nodes": []
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "tags[1]"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_imports_not_array_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "imports": "shared.security@^1.0.0",
        "nodes": []
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "imports"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_imports_item_wrong_type_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "imports": [123],
        "nodes": []
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "imports[0]"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_load_profiles_not_object_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "load_profiles": ["x"],
        "nodes": []
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "load_profiles"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_node_not_object_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [42]
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "nodes[0]"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_node_missing_required_returns_missing_field() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [{"node": "n", "type": "facts"}]
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::MissingField { field } => assert_eq!(field, "summary"),
        other => panic!("expected MissingField, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_node_invalid_enum_returns_invalid_enum_value() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [{
            "node": "n", "type": "facts", "summary": "s",
            "priority": "super-urgent"
        }]
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidEnumValue { field, .. } => assert_eq!(field, "priority"),
        other => panic!("expected InvalidEnumValue, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_node_enum_wrong_type_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [{
            "node": "n", "type": "facts", "summary": "s",
            "priority": 42
        }]
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "priority"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_retry_count_wrong_type_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [{
            "node": "n", "type": "facts", "summary": "s",
            "retry_count": "many"
        }]
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "retry_count"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_retry_count_out_of_u32_range_returns_invalid_type() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [{
            "node": "n", "type": "facts", "summary": "s",
            "retry_count": 10_000_000_000u64
        }]
    });
    let err = json_to_agm(&json).unwrap_err();
    match err {
        RenderError::InvalidType { field, .. } => assert_eq!(field, "retry_count"),
        other => panic!("expected InvalidType, got {other:?}"),
    }
}

#[test]
fn test_json_to_agm_retry_count_null_treated_as_none() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [{
            "node": "n", "type": "facts", "summary": "s",
            "retry_count": null
        }]
    });
    let file = json_to_agm(&json).unwrap();
    assert!(file.nodes[0].retry_count.is_none());
}

// --- extra_fields reverse-conversion variants -----------------------------

#[test]
fn test_json_to_agm_extra_fields_all_value_variants() {
    let json = json!({
        "agm": 1, "package": "p", "version": "1",
        "nodes": [{
            "node": "n", "type": "facts", "summary": "s",
            "x_bool": true,
            "x_num": 7,
            "x_str": "hello",
            "x_arr": ["a", "b", 3],
            "x_obj": {"nested": true}
        }]
    });
    let file = json_to_agm(&json).unwrap();
    let ef = &file.nodes[0].extra_fields;

    assert!(matches!(
        ef.get("x_bool"),
        Some(FieldValue::Scalar(s)) if s == "true"
    ));
    assert!(matches!(
        ef.get("x_num"),
        Some(FieldValue::Scalar(s)) if s == "7"
    ));
    assert!(matches!(
        ef.get("x_str"),
        Some(FieldValue::Scalar(s)) if s == "hello"
    ));
    assert!(matches!(ef.get("x_arr"), Some(FieldValue::List(_))));
    // Non-array non-primitive object is scalar-stringified.
    assert!(matches!(ef.get("x_obj"), Some(FieldValue::Scalar(_))));
}

// --- agm_to_json `coerce_scalar` branches via extra_fields ----------------

#[test]
fn test_agm_to_json_scalar_coercions_for_bool_int_string_in_extra_fields() {
    let mut file = kitchen_sink_file();
    let n = &mut file.nodes[0];
    n.extra_fields
        .insert("b_true".into(), FieldValue::Scalar("true".into()));
    n.extra_fields
        .insert("b_false".into(), FieldValue::Scalar("false".into()));
    n.extra_fields
        .insert("x_num".into(), FieldValue::Scalar("-17".into()));
    n.extra_fields
        .insert("x_str".into(), FieldValue::Scalar("hi".into()));
    n.extra_fields
        .insert("x_block".into(), FieldValue::Block("line1\nline2".into()));
    n.extra_fields.insert(
        "x_list".into(),
        FieldValue::List(vec!["a".into(), "b".into()]),
    );

    let json = agm_to_json(&file);
    let node = &json["nodes"][0];
    assert_eq!(node["b_true"], Value::Bool(true));
    assert_eq!(node["b_false"], Value::Bool(false));
    assert_eq!(node["x_num"], Value::Number((-17i64).into()));
    assert_eq!(node["x_str"], Value::String("hi".into()));
    assert!(node["x_block"].is_string());
    assert!(node["x_list"].is_array());
}
