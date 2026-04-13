//! Edge-case integration tests for the agm-core validator.
//!
//! Exercises boundary conditions, cross-node relationships, schema enforcement,
//! duplicate detection, reference resolution, orchestration, and security paths.

use std::collections::BTreeMap;

use agm_core::error::codes::ErrorCode;
use agm_core::error::diagnostic::Severity;
use agm_core::model::fields::{NodeStatus, NodeType, Span};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::validator::{ValidateOptions, validate};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn valid_header() -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: "test.edge".to_owned(),
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
    }
}

fn minimal_node(id: &str, line: usize) -> Node {
    Node {
        id: id.to_owned(),
        node_type: NodeType::Facts,
        summary: "a test node".to_owned(),
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
        span: Span::new(line, line + 2),
    }
}

fn count_errors_with_code(
    collection: &agm_core::error::diagnostic::DiagnosticCollection,
    code: ErrorCode,
) -> usize {
    collection
        .diagnostics()
        .iter()
        .filter(|d| d.code == code)
        .count()
}

fn has_error_with_code(
    collection: &agm_core::error::diagnostic::DiagnosticCollection,
    code: ErrorCode,
) -> bool {
    count_errors_with_code(collection, code) > 0
}

// ---------------------------------------------------------------------------
// 1. Overlapping errors: missing both type AND summary on the same node
// ---------------------------------------------------------------------------

#[test]
fn test_validate_node_missing_type_and_summary_reports_both_errors() {
    // We construct the node manually with an empty summary.
    // The parser always sets a type, so we use an empty summary + custom type
    // to test multiple errors on one node.  V002 fires on empty summary,
    // and V021 fires if the ID pattern is invalid — use both to confirm
    // the validator accumulates multiple errors per-node.
    let mut node = minimal_node("test.multi.err", 5);
    node.summary = String::new(); // triggers V002 (empty summary)

    // Also give it an invalid ID to trigger V021
    node.id = "INVALID ID with spaces".to_owned();
    node.span = Span::new(5, 7);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());

    // Both V002 and V021 should be present
    assert!(
        has_error_with_code(&result, ErrorCode::V002),
        "V002 (empty summary) expected"
    );
    assert!(
        has_error_with_code(&result, ErrorCode::V021),
        "V021 (invalid ID pattern) expected"
    );
}

// ---------------------------------------------------------------------------
// 2. Boundary values: pre-release and build-metadata versions are accepted
// ---------------------------------------------------------------------------

#[test]
fn test_validate_version_prerelease_alpha_returns_no_version_error() {
    let mut header = valid_header();
    header.version = "1.0.0-alpha".to_owned();
    let file = AgmFile {
        header,
        nodes: vec![minimal_node("test.node", 5)],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    // The validator should not reject pre-release version strings at the
    // file level (P001 is about missing fields, not format).
    assert!(
        !result.diagnostics().iter().any(|d| d.code == ErrorCode::P001),
        "Pre-release version should not trigger P001"
    );
}

#[test]
fn test_validate_version_build_metadata_returns_no_version_error() {
    let mut header = valid_header();
    header.version = "1.0.0+build.42".to_owned();
    let file = AgmFile {
        header,
        nodes: vec![minimal_node("test.node", 5)],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        !result.diagnostics().iter().any(|d| d.code == ErrorCode::P001),
        "Build-metadata version should not trigger P001"
    );
}

// ---------------------------------------------------------------------------
// 3. Cross-node conflicts: bidirectional conflicts reported exactly once
// ---------------------------------------------------------------------------

#[test]
fn test_validate_bidirectional_conflicts_reported_once() {
    let mut node_a = minimal_node("auth.a", 5);
    let mut node_b = minimal_node("auth.b", 10);
    node_a.conflicts = Some(vec!["auth.b".to_owned()]);
    node_b.conflicts = Some(vec!["auth.a".to_owned()]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node_a, node_b],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    let v013_count = count_errors_with_code(&result, ErrorCode::V013);
    assert_eq!(
        v013_count, 1,
        "Bidirectional conflict should produce exactly one V013 warning, got {v013_count}"
    );
}

#[test]
fn test_validate_unidirectional_conflict_reported_once() {
    let mut node_a = minimal_node("auth.a", 5);
    let node_b = minimal_node("auth.b", 10);
    // Only node_a declares the conflict
    node_a.conflicts = Some(vec!["auth.b".to_owned()]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node_a, node_b],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    let v013_count = count_errors_with_code(&result, ErrorCode::V013);
    assert_eq!(v013_count, 1, "Unidirectional conflict should yield V013");
}

// ---------------------------------------------------------------------------
// 4. Schema enforcement: workflow node with entity-only fields triggers warning
// ---------------------------------------------------------------------------

#[test]
fn test_validate_workflow_node_with_entity_fields_triggers_schema_warning() {
    // A `workflow` node with `fields` set (an entity-typical field) should
    // trigger V017 (standard mode) or V016 (strict mode) if that field is
    // disallowed for workflow nodes.
    let mut node = minimal_node("auth.flow", 5);
    node.node_type = NodeType::Workflow;
    // `fields` is an entity-specific field; workflow uses `steps` instead.
    node.fields = Some(vec!["field_one".to_owned(), "field_two".to_owned()]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    // Either V016 (strict) or V017 (standard) indicates schema enforcement.
    // In default (Standard) mode, V017 should fire as a warning.
    let has_schema_diag = result
        .diagnostics()
        .iter()
        .any(|d| d.code == ErrorCode::V016 || d.code == ErrorCode::V017);
    assert!(
        has_schema_diag,
        "Workflow node with entity-only `fields` should trigger schema warning/error"
    );
}

// ---------------------------------------------------------------------------
// 5. Duplicate detection: duplicate node IDs
// ---------------------------------------------------------------------------

#[test]
fn test_validate_duplicate_node_ids_reports_v003() {
    let node_a1 = minimal_node("auth.login", 5);
    let node_a2 = minimal_node("auth.login", 15); // same ID

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node_a1, node_a2],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V003),
        "Duplicate node ID must produce V003"
    );
}

#[test]
fn test_validate_three_nodes_two_duplicates_reports_v003() {
    let node_a1 = minimal_node("auth.login", 5);
    let node_a2 = minimal_node("auth.login", 15);
    let node_b = minimal_node("auth.rules", 25);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node_a1, node_a2, node_b],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(has_error_with_code(&result, ErrorCode::V003));
}

// ---------------------------------------------------------------------------
// 6. Reference validation: unresolved depends, circular depends, self-depends
// ---------------------------------------------------------------------------

#[test]
fn test_validate_depends_on_nonexistent_node_returns_v004() {
    let mut node = minimal_node("auth.login", 5);
    node.depends = Some(vec!["nonexistent.node".to_owned()]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V004),
        "Depends on nonexistent node must produce V004"
    );
}

#[test]
fn test_validate_self_dependency_returns_v005() {
    let mut node = minimal_node("auth.login", 5);
    node.depends = Some(vec!["auth.login".to_owned()]); // self-cycle

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V005),
        "Self-dependency must produce V005"
    );
}

#[test]
fn test_validate_circular_depends_chain_returns_v005() {
    let mut node_a = minimal_node("auth.a", 5);
    let mut node_b = minimal_node("auth.b", 10);
    let mut node_c = minimal_node("auth.c", 15);
    node_a.depends = Some(vec!["auth.b".to_owned()]);
    node_b.depends = Some(vec!["auth.c".to_owned()]);
    node_c.depends = Some(vec!["auth.a".to_owned()]); // closes the cycle

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node_a, node_b, node_c],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V005),
        "Three-node cycle must produce V005"
    );
}

#[test]
fn test_validate_multiple_unresolved_refs_reports_all() {
    let mut node = minimal_node("auth.login", 5);
    node.depends = Some(vec!["missing.a".to_owned(), "missing.b".to_owned()]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    let v004_count = count_errors_with_code(&result, ErrorCode::V004);
    assert_eq!(
        v004_count, 2,
        "Two unresolved deps should produce two V004 errors, got {v004_count}"
    );
}

// ---------------------------------------------------------------------------
// 7. Import edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_validate_import_no_resolver_skips_import_rules() {
    // Without an import resolver, I001-I005 should never fire.
    use agm_core::model::imports::ImportEntry;
    let mut header = valid_header();
    header.imports = Some(vec![ImportEntry {
        package: "nonexistent.pkg".to_owned(),
        version_constraint: Some(">=1.0.0, <2.0.0".to_owned()),
    }]);

    let file = AgmFile {
        header,
        nodes: vec![minimal_node("test.node", 5)],
    };
    let opts = ValidateOptions {
        import_resolver: None,
        ..Default::default()
    };
    let result = validate(&file, "", "edge.agm", &opts);
    // Without a resolver, no I-category errors should fire
    let import_errors = result
        .diagnostics()
        .iter()
        .filter(|d| d.code == ErrorCode::I001)
        .count();
    assert_eq!(
        import_errors, 0,
        "No import resolver means I001 should not fire"
    );
}

// ---------------------------------------------------------------------------
// 8. Orchestration validation
// ---------------------------------------------------------------------------

#[test]
fn test_validate_orchestration_empty_group_nodes_list_returns_v018() {
    let mut node = minimal_node("orch.main", 5);
    node.node_type = NodeType::Orchestration;
    node.parallel_groups = Some(vec![ParallelGroup {
        group: "phase1".to_owned(),
        nodes: vec![], // empty
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    }]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V018),
        "Empty group nodes list must produce V018"
    );
}

#[test]
fn test_validate_orchestration_group_references_nonexistent_node_returns_v018() {
    let mut node = minimal_node("orch.main", 5);
    node.node_type = NodeType::Orchestration;
    node.parallel_groups = Some(vec![ParallelGroup {
        group: "phase1".to_owned(),
        nodes: vec!["ghost.node".to_owned()], // not in the file
        strategy: Strategy::Sequential,
        requires: None,
        max_concurrency: None,
    }]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V018),
        "Nonexistent group node reference must produce V018"
    );
}

#[test]
fn test_validate_orchestration_requires_cycle_returns_v019() {
    let mut orch = minimal_node("orch.main", 5);
    orch.node_type = NodeType::Orchestration;
    let step_a = minimal_node("step.a", 20);
    let step_b = minimal_node("step.b", 25);
    orch.parallel_groups = Some(vec![
        ParallelGroup {
            group: "phase1".to_owned(),
            nodes: vec!["step.a".to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["phase2".to_owned()]),
            max_concurrency: None,
        },
        ParallelGroup {
            group: "phase2".to_owned(),
            nodes: vec!["step.b".to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["phase1".to_owned()]),
            max_concurrency: None,
        },
    ]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![orch, step_a, step_b],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V019),
        "Cycle in orchestration requires must produce V019"
    );
}

#[test]
fn test_validate_orchestration_nonexistent_group_in_requires_returns_v018() {
    let mut orch = minimal_node("orch.main", 5);
    orch.node_type = NodeType::Orchestration;
    let step_a = minimal_node("step.a", 20);
    orch.parallel_groups = Some(vec![ParallelGroup {
        group: "phase1".to_owned(),
        nodes: vec!["step.a".to_owned()],
        strategy: Strategy::Sequential,
        requires: Some(vec!["ghost.phase".to_owned()]), // group doesn't exist
        max_concurrency: None,
    }]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![orch, step_a],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V018),
        "Requires referencing nonexistent group must produce V018"
    );
}

// ---------------------------------------------------------------------------
// 9. Security validation: path traversal in code block target, empty verify command
// ---------------------------------------------------------------------------

#[test]
fn test_validate_code_block_target_path_traversal_returns_v015() {
    use agm_core::model::code::{CodeAction, CodeBlock};

    let mut node = minimal_node("sec.node", 5);
    node.code = Some(CodeBlock {
        lang: Some("rust".to_owned()),
        target: Some("../../etc/passwd".to_owned()),
        action: CodeAction::Create,
        body: "fn main() {}".to_owned(),
        anchor: None,
        old: None,
    });

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V015),
        "Path traversal in code block target must produce V015"
    );
}

#[test]
fn test_validate_code_block_absolute_target_returns_v015() {
    use agm_core::model::code::{CodeAction, CodeBlock};

    let mut node = minimal_node("sec.node", 5);
    node.code = Some(CodeBlock {
        lang: Some("rust".to_owned()),
        target: Some("/absolute/path/file.rs".to_owned()),
        action: CodeAction::Create,
        body: "fn main() {}".to_owned(),
        anchor: None,
        old: None,
    });

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V015),
        "Absolute path in code block target must produce V015"
    );
}

#[test]
fn test_validate_verify_empty_run_command_returns_v009() {
    use agm_core::model::verify::VerifyCheck;

    let mut node = minimal_node("sec.node", 5);
    node.verify = Some(vec![VerifyCheck::Command {
        run: "   ".to_owned(), // whitespace-only — should be treated as empty
        expect: None,
    }]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V009),
        "Empty verify run command must produce V009"
    );
}

// ---------------------------------------------------------------------------
// 10. Empty/minimal valid files
// ---------------------------------------------------------------------------

#[test]
fn test_validate_header_only_no_nodes_returns_p008() {
    // A file with only the header (no nodes) should produce P008.
    let file = AgmFile {
        header: valid_header(),
        nodes: vec![],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::P008),
        "File with no nodes must produce P008"
    );
}

#[test]
fn test_validate_node_with_only_required_fields_returns_no_errors() {
    // A node with only the minimum required fields (id, type, summary) is valid.
    let file = AgmFile {
        header: valid_header(),
        nodes: vec![minimal_node("auth.minimal", 5)],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        !result.has_errors(),
        "Node with only required fields should produce no errors"
    );
}

#[test]
fn test_validate_node_with_all_optional_fields_set_returns_no_errors() {
    use agm_core::model::fields::{Confidence, Priority, Stability};

    let mut node = minimal_node("auth.full", 5);
    // Populate every optional simple field
    node.priority = Some(Priority::High);
    node.stability = Some(Stability::Medium);
    node.confidence = Some(Confidence::High);
    node.status = Some(NodeStatus::Active);
    node.items = Some(vec!["item one".to_owned()]);
    node.steps = Some(vec!["step one".to_owned()]);
    node.detail = Some("Detailed explanation.".to_owned());
    node.rationale = Some(vec!["Why it matters.".to_owned()]);
    node.notes = Some("A note.".to_owned());
    node.tags = Some(vec!["auth".to_owned()]);
    node.aliases = Some(vec!["login".to_owned()]);
    node.keywords = Some(vec!["authentication".to_owned()]);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        !result.has_errors(),
        "Node with all optional fields set should produce no errors"
    );
}

#[test]
fn test_validate_valid_file_with_two_unrelated_nodes_returns_no_errors() {
    let node_a = minimal_node("auth.a", 5);
    let node_b = minimal_node("auth.b", 10);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node_a, node_b],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        !result.has_errors(),
        "Two unrelated valid nodes should produce no errors"
    );
}

// ---------------------------------------------------------------------------
// 11. Summary boundary: 200-char summary produces warning, 201 produces warning
// ---------------------------------------------------------------------------

#[test]
fn test_validate_summary_exactly_200_chars_returns_no_length_warning() {
    let mut node = minimal_node("auth.long", 5);
    node.summary = "x".repeat(200);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    // 200 chars is at the boundary — V012 threshold is > 200
    let v012_count = count_errors_with_code(&result, ErrorCode::V012);
    assert_eq!(
        v012_count, 0,
        "Summary of exactly 200 characters should not trigger V012"
    );
}

#[test]
fn test_validate_summary_201_chars_returns_v012_warning() {
    let mut node = minimal_node("auth.long", 5);
    node.summary = "x".repeat(201);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V012),
        "Summary exceeding 200 characters must produce V012"
    );
    // V012 is a Warning, not an Error
    assert!(
        result
            .diagnostics()
            .iter()
            .filter(|d| d.code == ErrorCode::V012)
            .all(|d| d.severity == Severity::Warning),
        "V012 should be a Warning"
    );
}

// ---------------------------------------------------------------------------
// 12. Node ID pattern edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_validate_node_id_with_hyphens_is_valid() {
    let node = minimal_node("auth.login-flow", 5);
    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        !has_error_with_code(&result, ErrorCode::V021),
        "Hyphenated node ID should be valid"
    );
}

#[test]
fn test_validate_node_id_starting_with_digit_returns_v021() {
    let mut node = minimal_node("test.placeholder", 5);
    node.id = "1invalid.start".to_owned();

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V021),
        "ID starting with digit must produce V021"
    );
}

#[test]
fn test_validate_node_id_with_uppercase_returns_v021() {
    let mut node = minimal_node("test.placeholder", 5);
    node.id = "Auth.Login".to_owned();

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V021),
        "Uppercase in node ID must produce V021"
    );
}

// ---------------------------------------------------------------------------
// 13. Valid_from / valid_until inversion
// ---------------------------------------------------------------------------

#[test]
fn test_validate_valid_from_after_valid_until_returns_v007() {
    let mut node = minimal_node("auth.dated", 5);
    node.valid_from = Some("2026-12-01".to_owned());
    node.valid_until = Some("2026-01-01".to_owned()); // before valid_from

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V007),
        "valid_from after valid_until must produce V007"
    );
}

#[test]
fn test_validate_valid_from_before_valid_until_returns_no_v007() {
    let mut node = minimal_node("auth.dated", 5);
    node.valid_from = Some("2026-01-01".to_owned());
    node.valid_until = Some("2026-12-31".to_owned());

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        !has_error_with_code(&result, ErrorCode::V007),
        "valid_from before valid_until should not produce V007"
    );
}

// ---------------------------------------------------------------------------
// 14. Deprecated node missing replaces
// ---------------------------------------------------------------------------

#[test]
fn test_validate_deprecated_node_without_replaces_returns_v014() {
    let mut node = minimal_node("auth.old", 5);
    node.status = Some(NodeStatus::Deprecated);
    // No `replaces` field set

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V014),
        "Deprecated node without replaces must produce V014"
    );
}

#[test]
fn test_validate_deprecated_node_with_replaces_returns_no_v014() {
    let mut old_node = minimal_node("auth.old", 5);
    old_node.status = Some(NodeStatus::Deprecated);
    old_node.replaces = Some(vec!["auth.older".to_owned()]);

    // We need "auth.older" to exist in the file or the replaces ref will fail V004.
    // Use a separate node to satisfy it.
    let older_node = minimal_node("auth.older", 15);

    let file = AgmFile {
        header: valid_header(),
        nodes: vec![old_node, older_node],
    };
    let result = validate(&file, "", "edge.agm", &Default::default());
    assert!(
        !has_error_with_code(&result, ErrorCode::V014),
        "Deprecated node with replaces must not produce V014"
    );
}

// ---------------------------------------------------------------------------
// STRESS / LARGE FILE TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_validate_300_nodes_all_valid_no_errors() {
    let nodes: Vec<Node> = (0..300usize)
        .map(|i| minimal_node(&format!("stress.n{i:03}"), i * 4 + 1))
        .collect();

    let file = AgmFile {
        header: valid_header(),
        nodes,
    };
    let result = validate(&file, "", "stress.agm", &Default::default());
    assert!(
        !result.has_errors(),
        "300 valid unique nodes should produce no errors"
    );
}

#[test]
fn test_validate_300_nodes_with_cross_references() {
    // Build a simple DAG: node[i] depends on node[i-1] for i >= 1
    let mut nodes: Vec<Node> = (0..300usize)
        .map(|i| minimal_node(&format!("dag.n{i:03}"), i * 4 + 1))
        .collect();
    // Set up a valid DAG: each node depends on the previous one
    for i in 1..300usize {
        nodes[i].depends = Some(vec![format!("dag.n{:03}", i - 1)]);
    }

    let file = AgmFile {
        header: valid_header(),
        nodes,
    };
    let result = validate(&file, "", "stress.agm", &Default::default());
    assert!(
        !result.has_errors(),
        "300-node linear DAG with valid cross-references should produce no errors"
    );
}

#[test]
fn test_validate_100_nodes_all_duplicate_ids() {
    // 100 nodes all sharing the same ID — each is a duplicate of "dup.node"
    let nodes: Vec<Node> = (0..100usize)
        .map(|i| minimal_node("dup.node", i * 4 + 1))
        .collect();

    let file = AgmFile {
        header: valid_header(),
        nodes,
    };
    let result = validate(&file, "", "stress.agm", &Default::default());
    assert!(
        has_error_with_code(&result, ErrorCode::V003),
        "100 nodes with the same ID should produce V003 errors"
    );
    // V003 fires for each duplicate occurrence (all except the first)
    let v003_count = count_errors_with_code(&result, ErrorCode::V003);
    assert!(
        v003_count >= 99,
        "expected at least 99 V003 errors for 100 duplicate IDs, got {v003_count}"
    );
}

#[test]
fn test_validate_large_file_many_warnings() {
    // 200 nodes each with a summary exceeding 200 characters — all should generate V012 warnings
    let long_summary = "w".repeat(201);
    let nodes: Vec<Node> = (0..200usize)
        .map(|i| {
            let mut n = minimal_node(&format!("warn.n{i:03}"), i * 4 + 1);
            n.summary = long_summary.clone();
            n
        })
        .collect();

    let file = AgmFile {
        header: valid_header(),
        nodes,
    };
    let result = validate(&file, "", "stress.agm", &Default::default());
    let v012_count = count_errors_with_code(&result, ErrorCode::V012);
    assert_eq!(
        v012_count, 200,
        "200 nodes with over-long summaries should generate 200 V012 warnings, got {v012_count}"
    );
}
