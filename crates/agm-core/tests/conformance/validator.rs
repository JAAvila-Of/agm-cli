//! Validator conformance tests (sub-step 16.3).
//!
//! Five test groups:
//!  - required field validation (10 fixtures)
//!  - reference resolution (10 fixtures)
//!  - cycle detection (5 fixtures)
//!  - type schema validation (10 fixtures)
//!  - security validation (5 fixtures)

use super::runner::{assert_validate_conformance, fixtures_path};

// ---------------------------------------------------------------------------
// 16.3.1 -- Required field validation (10 minimum)
// ---------------------------------------------------------------------------

/// Verifies that missing required fields produce the expected error codes.
///
/// Covers V001 (missing type), V002 (missing summary), V006 (bad execution status),
/// V011 (empty summary), V012 (summary too long), V024 (type schema).
#[test]
fn test_conformance_validate_required_fields() {
    // Required field checks at parse and validate level
    let parse_invalid = fixtures_path("parse/invalid");
    let parse_fixtures = ["missing_node_type.agm", "missing_node_summary.agm"];
    for fixture_name in &parse_fixtures {
        let path = parse_invalid.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));
        // Use validate conformance since some may produce warnings via validator
        assert_validate_conformance(&path, &content, &directive);
    }

    // Validate-level required field errors
    let validate_invalid = fixtures_path("validate/invalid");
    let validate_fixtures = [
        "empty_summary.agm",
        "summary_too_long.agm",
        "schema_missing_required.agm",
        "schema_rules_missing_items.agm",
        "schema_entity_missing_fields.agm",
        "schema_decision_missing_rationale.agm",
        "schema_orchestration_missing_groups.agm",
        "bad_execution_status.agm",
    ];
    for fixture_name in &validate_fixtures {
        let path = validate_invalid.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));
        assert_validate_conformance(&path, &content, &directive);
    }
}

// ---------------------------------------------------------------------------
// 16.3.2 -- Reference resolution (10 minimum)
// ---------------------------------------------------------------------------

/// Verifies that unresolved references produce V004 and valid references pass.
#[test]
fn test_conformance_validate_references() {
    // Valid cross-reference fixtures
    let validate_valid = fixtures_path("validate/valid");
    let valid_fixtures = [
        "cross_references_valid.agm",
        "self_references_valid.agm",
        "multi_package_refs_valid.agm",
    ];
    for fixture_name in &valid_fixtures {
        let path = validate_valid.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));
        assert_validate_conformance(&path, &content, &directive);
    }

    // Invalid fixtures with unresolved references
    let validate_invalid = fixtures_path("validate/invalid");
    let invalid_fixtures = [
        "unresolved_depends_ref.agm",
        "unresolved_related_to_ref.agm",
        "unresolved_conflicts_ref.agm",
        "unresolved_replaces_ref.agm",
        "unresolved_see_also_ref.agm",
        "unresolved_agent_context_load_node.agm",
        "unresolved_load_profile_node.agm",
    ];
    for fixture_name in &invalid_fixtures {
        let path = validate_invalid.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));
        assert_validate_conformance(&path, &content, &directive);
    }
}

// ---------------------------------------------------------------------------
// 16.3.3 -- Cycle detection (5 minimum)
// ---------------------------------------------------------------------------

/// Verifies that cyclic dependencies produce V005/V019 error codes.
#[test]
fn test_conformance_validate_cycles() {
    let dir = fixtures_path("validate/invalid");
    let cycle_fixtures = [
        "cycle_in_depends.agm",
        "cycle_two_nodes.agm",
        "cycle_long_chain.agm",
        "cycle_self_depends.agm",
        "orchestration_cycle.agm",
    ];

    let mut failures = Vec::new();
    for fixture_name in &cycle_fixtures {
        let path = dir.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));

        if let Err(msg) = std::panic::catch_unwind(|| {
            assert_validate_conformance(&path, &content, &directive);
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
            "{} cycle detection failure(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    assert_eq!(
        cycle_fixtures.len(),
        5,
        "Expected exactly 5 cycle detection fixtures"
    );
}

// ---------------------------------------------------------------------------
// 16.3.4 -- Type schema validation (10 minimum)
// ---------------------------------------------------------------------------

/// Verifies type schema enforcement produces correct V016/V017/V024 codes.
#[test]
fn test_conformance_validate_type_schema() {
    // Valid schema fixtures
    let validate_valid = fixtures_path("validate/valid");
    let valid_fixtures = [
        "well_formed_all_types.agm",
        "schema_strict_pass.agm",
        "schema_standard_pass.agm",
    ];
    for fixture_name in &valid_fixtures {
        let path = validate_valid.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));
        assert_validate_conformance(&path, &content, &directive);
    }

    // Invalid schema fixtures
    let validate_invalid = fixtures_path("validate/invalid");
    let invalid_fixtures = [
        "schema_missing_required.agm",
        "schema_rules_missing_items.agm",
        "schema_entity_missing_fields.agm",
        "schema_decision_missing_rationale.agm",
        "schema_orchestration_missing_groups.agm",
        "disallowed_field_standard.agm",
        "disallowed_field_strict.agm",
    ];
    for fixture_name in &invalid_fixtures {
        let path = validate_invalid.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));
        assert_validate_conformance(&path, &content, &directive);
    }
}

// ---------------------------------------------------------------------------
// 16.3.5 -- Security validation (5 minimum)
// ---------------------------------------------------------------------------

/// Verifies security-related violations produce V008/V015 error codes.
#[test]
fn test_conformance_validate_security() {
    let dir = fixtures_path("validate/invalid");
    let security_fixtures = [
        "absolute_target_path.agm",
        "traversal_target_path.agm",
        "agent_context_absolute_path.agm",
        "agent_context_traversal_path.agm",
        "code_block_secret_pattern.agm",
    ];

    let mut failures = Vec::new();
    for fixture_name in &security_fixtures {
        let path = dir.join(fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let directive = super::runner::parse_expect_header(&content)
            .unwrap_or_else(|| panic!("missing expect header in {}", path.display()));

        if let Err(msg) = std::panic::catch_unwind(|| {
            assert_validate_conformance(&path, &content, &directive);
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
            "{} security validation failure(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    assert_eq!(
        security_fixtures.len(),
        5,
        "Expected exactly 5 security validation fixtures"
    );
}
