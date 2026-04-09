//! Forward conversion integration tests (sub-step 13.10).
//!
//! Each test parses a fixture .agm file, applies `agm_to_json`, and
//! compares the result to the corresponding golden JSON fixture.

use agm_core::parser::parse;
use agm_core::renderer::json_canonical::agm_to_json;

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

fn parse_agm(relative: &str) -> agm_core::model::file::AgmFile {
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

macro_rules! forward_test {
    ($test_name:ident, $agm_rel:literal, $json_rel:literal) => {
        #[test]
        fn $test_name() {
            let file = parse_agm($agm_rel);
            let actual = agm_to_json(&file);
            let expected = load_json($json_rel);
            assert_eq!(
                actual, expected,
                "forward conversion mismatch for {}: actual != expected",
                $agm_rel
            );
        }
    };
}

forward_test!(
    test_json_forward_minimal,
    "json/forward/minimal.agm",
    "json/forward/minimal.json"
);

forward_test!(
    test_json_forward_auth_platform,
    "json/forward/auth_platform.agm",
    "json/forward/auth_platform.json"
);

forward_test!(
    test_json_forward_billing_invoice,
    "json/forward/billing_invoice.agm",
    "json/forward/billing_invoice.json"
);

forward_test!(
    test_json_forward_caching_migration,
    "json/forward/caching_migration.agm",
    "json/forward/caching_migration.json"
);

forward_test!(
    test_json_forward_pricing_temporal,
    "json/forward/pricing_temporal.agm",
    "json/forward/pricing_temporal.json"
);

forward_test!(
    test_json_forward_with_agent_context,
    "json/forward/with_agent_context.agm",
    "json/forward/with_agent_context.json"
);

forward_test!(
    test_json_forward_with_code_block,
    "json/forward/with_code_block.agm",
    "json/forward/with_code_block.json"
);

forward_test!(
    test_json_forward_with_memory,
    "json/forward/with_memory.agm",
    "json/forward/with_memory.json"
);

forward_test!(
    test_json_forward_with_orchestration,
    "json/forward/with_orchestration.agm",
    "json/forward/with_orchestration.json"
);

forward_test!(
    test_json_forward_with_verify,
    "json/forward/with_verify.agm",
    "json/forward/with_verify.json"
);
