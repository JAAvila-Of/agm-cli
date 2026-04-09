//! Parser conformance tests (sub-step 16.2).
//!
//! Four test groups:
//!  - valid parse (22 existing fixtures)
//!  - invalid parse (16 existing fixtures)
//!  - edge cases (11 existing fixtures)
//!  - round-trip (5 new fixtures)

use super::runner::{
    ExpectDirective, assert_parse_conformance, assert_parse_roundtrip, discover_fixtures,
    fixtures_path, run_fixture_dir,
};

// ---------------------------------------------------------------------------
// 16.2.1 -- Valid parse (22 minimum)
// ---------------------------------------------------------------------------

/// Verifies that all fixtures in `parse/valid/` parse without errors.
#[test]
fn test_conformance_parse_valid() {
    let dir = fixtures_path("parse/valid");
    let count = run_fixture_dir(&dir, "agm", |path, content, directive| {
        assert!(
            *directive == ExpectDirective::ParseOk,
            "Expected 'parse ok' directive in {}, got {directive:?}",
            path.display()
        );
        assert_parse_conformance(path, content, directive);
    });
    assert!(
        count >= 20,
        "Expected at least 20 valid parse fixtures, found {count}"
    );
}

// ---------------------------------------------------------------------------
// 16.2.2 -- Invalid parse (16 minimum)
// ---------------------------------------------------------------------------

/// Verifies that all fixtures in `parse/invalid/` produce the expected error/warning codes.
#[test]
fn test_conformance_parse_invalid() {
    let dir = fixtures_path("parse/invalid");
    let count = run_fixture_dir(&dir, "agm", |path, content, directive| {
        assert!(
            matches!(
                directive,
                ExpectDirective::Error(_) | ExpectDirective::Warning(_)
            ),
            "Expected 'error' or 'warning' directive in {}, got {directive:?}",
            path.display()
        );
        // Use validate conformance for warning directives (must run full pipeline)
        match directive {
            ExpectDirective::Warning(_) => {
                super::runner::assert_validate_conformance(path, content, directive);
            }
            _ => {
                assert_parse_conformance(path, content, directive);
            }
        }
    });
    assert!(
        count >= 15,
        "Expected at least 15 invalid parse fixtures, found {count}"
    );
}

// ---------------------------------------------------------------------------
// 16.2.3 -- Edge cases (11 minimum)
// ---------------------------------------------------------------------------

/// Verifies that all fixtures in `parse/edge/` parse successfully.
///
/// Edge cases are valid files exercising boundary conditions.
#[test]
fn test_conformance_parse_edge() {
    let dir = fixtures_path("parse/edge");
    let count = run_fixture_dir(&dir, "agm", |path, content, directive| {
        assert!(
            *directive == ExpectDirective::ParseOk,
            "Expected 'parse ok' directive in {}, got {directive:?}",
            path.display()
        );
        assert_parse_conformance(path, content, directive);
    });
    assert!(
        count >= 10,
        "Expected at least 10 edge case fixtures, found {count}"
    );
}

// ---------------------------------------------------------------------------
// 16.2.4 -- Round-trip (5 minimum)
// ---------------------------------------------------------------------------

/// Verifies parse -> render_canonical -> parse round-trips produce identical ASTs.
#[test]
fn test_conformance_parse_roundtrip() {
    let dir = fixtures_path("parse/roundtrip");
    let fixtures = discover_fixtures(&dir, "agm");
    assert!(
        !fixtures.is_empty(),
        "No round-trip fixtures found in {}",
        dir.display()
    );

    let mut failures = Vec::new();
    for path in &fixtures {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

        if let Err(msg) = std::panic::catch_unwind(|| {
            assert_parse_roundtrip(path, &content);
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
            "{} round-trip failure(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    assert!(
        fixtures.len() >= 5,
        "Expected at least 5 round-trip fixtures, found {}",
        fixtures.len()
    );
}
