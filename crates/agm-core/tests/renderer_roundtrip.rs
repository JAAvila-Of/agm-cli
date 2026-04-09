//! Round-trip integration tests (sub-step 13.11).
//!
//! JSON round-trip: json_to_agm -> agm_to_json must produce the original JSON.
//! AGM text round-trip: parse -> render_canonical -> parse must produce equivalent AST.

use agm_core::model::fields::Span;
use agm_core::model::file::AgmFile;
use agm_core::model::node::Node;
use agm_core::parser::parse;
use agm_core::renderer::canonical::render_canonical;
use agm_core::renderer::json_canonical::{agm_to_json, json_to_agm};

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

fn parse_agm(relative: &str) -> AgmFile {
    let path = fixtures_root().join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
    parse(&text).unwrap_or_else(|errs| panic!("parse errors in {relative}: {errs:?}"))
}

fn load_json(relative: &str) -> serde_json::Value {
    let path = fixtures_root().join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid JSON in {relative}: {e}"))
}

// ---------------------------------------------------------------------------
// JSON round-trip tests
// ---------------------------------------------------------------------------

macro_rules! json_roundtrip_test {
    ($test_name:ident, $json_rel:literal) => {
        #[test]
        fn $test_name() {
            let json1 = load_json($json_rel);
            let file = json_to_agm(&json1)
                .unwrap_or_else(|e| panic!("json_to_agm failed for {}: {e}", $json_rel));
            let json2 = agm_to_json(&file);
            assert_eq!(
                json1, json2,
                "JSON round-trip failed for {}: json1 != json2",
                $json_rel
            );
        }
    };
}

json_roundtrip_test!(test_json_roundtrip_minimal, "json/roundtrip/minimal.json");
json_roundtrip_test!(
    test_json_roundtrip_all_node_types,
    "json/roundtrip/all_node_types.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_agent_context,
    "json/roundtrip/with_agent_context.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_code_block,
    "json/roundtrip/with_code_block.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_execution_state,
    "json/roundtrip/with_execution_state.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_imports,
    "json/roundtrip/with_imports.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_load_profiles,
    "json/roundtrip/with_load_profiles.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_memory,
    "json/roundtrip/with_memory.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_orchestration,
    "json/roundtrip/with_orchestration.json"
);
json_roundtrip_test!(
    test_json_roundtrip_with_verify,
    "json/roundtrip/with_verify.json"
);

// ---------------------------------------------------------------------------
// AGM text round-trip tests
// ---------------------------------------------------------------------------

/// Strip all spans from an AgmFile for semantic comparison.
fn strip_spans(file: &AgmFile) -> AgmFile {
    AgmFile {
        header: file.header.clone(),
        nodes: file.nodes.iter().map(strip_node_span).collect(),
    }
}

fn strip_node_span(node: &Node) -> Node {
    Node {
        span: Span::default(),
        ..node.clone()
    }
}

macro_rules! agm_roundtrip_test {
    ($test_name:ident, $agm_rel:literal) => {
        #[test]
        fn $test_name() {
            let file1 = parse_agm($agm_rel);
            let canonical = render_canonical(&file1);
            let file2 = parse(&canonical).unwrap_or_else(|errs| {
                panic!(
                    "re-parse of canonical output failed for {}: {errs:?}\n\nCanonical output:\n{canonical}",
                    $agm_rel
                );
            });
            let stripped1 = strip_spans(&file1);
            let stripped2 = strip_spans(&file2);
            assert_eq!(
                stripped1, stripped2,
                "AGM text round-trip failed for {}: ASTs differ after canonical re-parse",
                $agm_rel
            );
        }
    };
}

// All round-trip fixtures use the json/forward directory (agm: 1.0 format)
agm_roundtrip_test!(test_canonical_roundtrip_minimal, "json/forward/minimal.agm");
agm_roundtrip_test!(
    test_canonical_roundtrip_with_verify,
    "json/forward/with_verify.agm"
);
agm_roundtrip_test!(
    test_canonical_roundtrip_with_code_block,
    "json/forward/with_code_block.agm"
);
agm_roundtrip_test!(
    test_canonical_roundtrip_with_memory,
    "json/forward/with_memory.agm"
);
agm_roundtrip_test!(
    test_canonical_roundtrip_auth_platform,
    "json/forward/auth_platform.agm"
);
