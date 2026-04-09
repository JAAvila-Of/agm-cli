//! JSON conformance tests (sub-step 16.4).
//!
//! Two test groups:
//!  - forward conversion (AGM -> JSON, 10 fixtures)
//!  - round-trip conversion (JSON -> AGM -> JSON, 10 fixtures)

use super::runner::{assert_json_forward, assert_json_roundtrip, discover_fixtures, fixtures_path};

// ---------------------------------------------------------------------------
// 16.4.1 -- Forward conversion (10 minimum)
// ---------------------------------------------------------------------------

/// Verifies AGM -> JSON conversion matches companion `.json` files.
#[test]
fn test_conformance_json_forward() {
    let dir = fixtures_path("json/forward");
    let agm_fixtures = discover_fixtures(&dir, "agm");
    assert!(
        !agm_fixtures.is_empty(),
        "No JSON forward fixtures found in {}",
        dir.display()
    );

    let mut failures = Vec::new();
    for path in &agm_fixtures {
        if let Err(msg) = std::panic::catch_unwind(|| {
            assert_json_forward(path);
        }) {
            let msg_str = if let Some(s) = msg.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = msg.downcast_ref::<&str>() {
                s.to_string()
            } else {
                format!("panic in {}", path.display())
            };
            failures.push(msg_str);
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} JSON forward failure(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    assert!(
        agm_fixtures.len() >= 10,
        "Expected at least 10 JSON forward fixtures, found {}",
        agm_fixtures.len()
    );
}

// ---------------------------------------------------------------------------
// 16.4.2 -- Round-trip conversion (10 minimum)
// ---------------------------------------------------------------------------

/// Verifies JSON -> AGM -> canonical text -> parse -> JSON round-trips.
#[test]
fn test_conformance_json_roundtrip() {
    let dir = fixtures_path("json/roundtrip");
    let json_fixtures = discover_fixtures(&dir, "json");
    assert!(
        !json_fixtures.is_empty(),
        "No JSON roundtrip fixtures found in {}",
        dir.display()
    );

    let mut failures = Vec::new();
    for path in &json_fixtures {
        if let Err(msg) = std::panic::catch_unwind(|| {
            assert_json_roundtrip(path);
        }) {
            let msg_str = if let Some(s) = msg.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = msg.downcast_ref::<&str>() {
                s.to_string()
            } else {
                format!("panic in {}", path.display())
            };
            failures.push(msg_str);
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} JSON roundtrip failure(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    assert!(
        json_fixtures.len() >= 10,
        "Expected at least 10 JSON roundtrip fixtures, found {}",
        json_fixtures.len()
    );
}
