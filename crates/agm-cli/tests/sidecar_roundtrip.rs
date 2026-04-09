//! Sidecar round-trip tests: parse -> render -> re-parse, assert equality.

use agm_core::parser::mem::parse_mem;
use agm_core::parser::state::parse_state;
use agm_core::renderer::mem::render_mem;
use agm_core::renderer::state::render_state;

fn fixture_path(relative: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

fn read_fixture(relative: &str) -> String {
    std::fs::read_to_string(fixture_path(relative))
        .unwrap_or_else(|e| panic!("Failed to read fixture {relative}: {e}"))
}

#[test]
fn test_state_roundtrip_completed_matches_original() {
    let input = read_fixture("state/completed.agm.state");
    let parsed1 = parse_state(&input).expect("first parse failed");
    let rendered = render_state(&parsed1);
    let parsed2 = parse_state(&rendered).expect("second parse failed");
    assert_eq!(parsed1, parsed2, "round-trip mismatch for completed state");
}

#[test]
fn test_state_roundtrip_partial_matches_original() {
    let input = read_fixture("state/partial.agm.state");
    let parsed1 = parse_state(&input).expect("first parse failed");
    let rendered = render_state(&parsed1);
    let parsed2 = parse_state(&rendered).expect("second parse failed");
    assert_eq!(parsed1, parsed2, "round-trip mismatch for partial state");
}

#[test]
fn test_state_roundtrip_failed_matches_original() {
    let input = read_fixture("state/failed.agm.state");
    let parsed1 = parse_state(&input).expect("first parse failed");
    let rendered = render_state(&parsed1);
    let parsed2 = parse_state(&rendered).expect("second parse failed");
    assert_eq!(parsed1, parsed2, "round-trip mismatch for failed state");
}

#[test]
fn test_mem_roundtrip_project_matches_original() {
    let input = read_fixture("memory/project.agm.mem");
    let parsed1 = parse_mem(&input).expect("first parse failed");
    let rendered = render_mem(&parsed1);
    let parsed2 = parse_mem(&rendered).expect("second parse failed");
    assert_eq!(parsed1, parsed2, "round-trip mismatch for project mem");
}

#[test]
fn test_mem_roundtrip_expired_matches_original() {
    let input = read_fixture("memory/expired.agm.mem");
    let parsed1 = parse_mem(&input).expect("first parse failed");
    let rendered = render_mem(&parsed1);
    let parsed2 = parse_mem(&rendered).expect("second parse failed");
    assert_eq!(parsed1, parsed2, "round-trip mismatch for expired mem");
}
