//! Fixture generator — run once to populate JSON fixture files.
//!
//! Run with:
//!   GENERATE_FIXTURES=1 cargo test -p agm-core --test generate_fixtures -- --nocapture
//!
//! This test is gated behind `GENERATE_FIXTURES=1` so it does not run in normal CI.

use agm_core::parser::parse;
use agm_core::renderer::json_canonical::agm_to_json;

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

fn parse_agm_fixture(relative: &str) -> agm_core::model::file::AgmFile {
    let path = fixtures_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parse(&text).unwrap_or_else(|errs| {
        panic!("parse errors in {relative}: {errs:?}");
    })
}

fn write_json_fixture(relative: &str, value: &serde_json::Value) {
    let path = fixtures_root().join(relative);
    let json_str = serde_json::to_string_pretty(value).unwrap();
    std::fs::write(&path, &json_str)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
    println!("Wrote {}", path.display());
}

macro_rules! gen_forward {
    ($test_name:ident, $src_agm:literal, $dst_json:literal) => {
        #[test]
        fn $test_name() {
            if std::env::var("GENERATE_FIXTURES").is_err() {
                return;
            }
            let file = parse_agm_fixture($src_agm);
            let json_val = agm_to_json(&file);
            write_json_fixture($dst_json, &json_val);
        }
    };
}

// Forward: parse a fixture .agm and emit canonical JSON
gen_forward!(
    gen_forward_minimal,
    "json/forward/minimal.agm",
    "json/forward/minimal.json"
);
gen_forward!(
    gen_forward_auth_platform,
    "json/forward/auth_platform.agm",
    "json/forward/auth_platform.json"
);
gen_forward!(
    gen_forward_billing_invoice,
    "json/forward/billing_invoice.agm",
    "json/forward/billing_invoice.json"
);
gen_forward!(
    gen_forward_caching_migration,
    "json/forward/caching_migration.agm",
    "json/forward/caching_migration.json"
);
gen_forward!(
    gen_forward_pricing_temporal,
    "json/forward/pricing_temporal.agm",
    "json/forward/pricing_temporal.json"
);
gen_forward!(
    gen_forward_with_agent_context,
    "json/forward/with_agent_context.agm",
    "json/forward/with_agent_context.json"
);
gen_forward!(
    gen_forward_with_code_block,
    "json/forward/with_code_block.agm",
    "json/forward/with_code_block.json"
);
gen_forward!(
    gen_forward_with_memory,
    "json/forward/with_memory.agm",
    "json/forward/with_memory.json"
);
gen_forward!(
    gen_forward_with_orchestration,
    "json/forward/with_orchestration.agm",
    "json/forward/with_orchestration.json"
);
gen_forward!(
    gen_forward_with_verify,
    "json/forward/with_verify.agm",
    "json/forward/with_verify.json"
);

// Roundtrip: also parse the same sources, emit as S37 JSON into the roundtrip dir.
// The roundtrip fixtures are JSON files that survive json_to_agm -> agm_to_json unchanged.
gen_forward!(
    gen_roundtrip_minimal,
    "json/forward/minimal.agm",
    "json/roundtrip/minimal.json"
);
gen_forward!(
    gen_roundtrip_all_node_types,
    "json/forward/auth_platform.agm",
    "json/roundtrip/all_node_types.json"
);
gen_forward!(
    gen_roundtrip_with_agent_context,
    "json/forward/with_agent_context.agm",
    "json/roundtrip/with_agent_context.json"
);
gen_forward!(
    gen_roundtrip_with_code_block,
    "json/forward/with_code_block.agm",
    "json/roundtrip/with_code_block.json"
);
gen_forward!(
    gen_roundtrip_with_execution_state,
    "json/forward/with_orchestration.agm",
    "json/roundtrip/with_execution_state.json"
);
gen_forward!(
    gen_roundtrip_with_imports,
    "json/forward/billing_invoice.agm",
    "json/roundtrip/with_imports.json"
);
gen_forward!(
    gen_roundtrip_with_load_profiles,
    "json/forward/pricing_temporal.agm",
    "json/roundtrip/with_load_profiles.json"
);
gen_forward!(
    gen_roundtrip_with_memory,
    "json/forward/with_memory.agm",
    "json/roundtrip/with_memory.json"
);
gen_forward!(
    gen_roundtrip_with_orchestration,
    "json/forward/with_orchestration.agm",
    "json/roundtrip/with_orchestration.json"
);
gen_forward!(
    gen_roundtrip_with_verify,
    "json/forward/with_verify.agm",
    "json/roundtrip/with_verify.json"
);
