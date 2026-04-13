//! Coverage gaps in `validator/verify.rs` and `validator/imports.rs`.
//!
//! Exercises every "missing required field" and "unresolved reference"
//! branch in `validate_verify`, plus the cross-package ref resolution
//! path in `validate_imports`.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use agm_core::error::ErrorCode;
use agm_core::import::{ImportResolver, ResolvedPackage, ValidatedImport};
use agm_core::model::fields::{NodeType, Span};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::imports::ImportEntry;
use agm_core::model::node::Node;
use agm_core::model::verify::VerifyCheck;
use agm_core::validator::imports::validate_imports;
use agm_core::validator::verify::validate_verify;

fn blank_node(id: &str) -> Node {
    Node {
        id: id.to_owned(),
        node_type: NodeType::Facts,
        summary: "s".to_owned(),
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
        span: Span::new(1, 5),
    }
}

// ===========================================================================
// validate_verify — exhaustive branch coverage
// ===========================================================================

#[test]
fn test_validate_verify_none_returns_empty() {
    let node = blank_node("n");
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.is_empty());
}

#[test]
fn test_validate_verify_command_empty_run_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::Command {
        run: "  ".to_owned(), // whitespace only
        expect: None,
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
}

#[test]
fn test_validate_verify_file_exists_empty_file_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::FileExists {
        file: "".to_owned(),
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
}

#[test]
fn test_validate_verify_file_contains_missing_file_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::FileContains {
        file: "".to_owned(),
        pattern: "ok".to_owned(),
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
}

#[test]
fn test_validate_verify_file_contains_missing_pattern_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::FileContains {
        file: "src/lib.rs".to_owned(),
        pattern: "".to_owned(),
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
}

#[test]
fn test_validate_verify_file_not_contains_missing_file_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::FileNotContains {
        file: "".to_owned(),
        pattern: "x".to_owned(),
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
}

#[test]
fn test_validate_verify_file_not_contains_missing_pattern_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::FileNotContains {
        file: "src/lib.rs".to_owned(),
        pattern: "".to_owned(),
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
}

#[test]
fn test_validate_verify_node_status_empty_node_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::NodeStatus {
        node: "".to_owned(),
        status: "completed".to_owned(),
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
}

#[test]
fn test_validate_verify_node_status_unresolved_ref_emits_v009() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::NodeStatus {
        node: "does.not.exist".to_owned(),
        status: "completed".to_owned(),
    }]);
    let errors = validate_verify(&node, &HashSet::new(), "t.agm");
    assert!(
        errors
            .iter()
            .any(|e| e.code == ErrorCode::V009 && e.message.contains("non-existent")),
        "got: {errors:?}"
    );
}

#[test]
fn test_validate_verify_node_status_resolved_ref_ok() {
    let mut node = blank_node("n");
    node.verify = Some(vec![VerifyCheck::NodeStatus {
        node: "other.node".to_owned(),
        status: "completed".to_owned(),
    }]);
    let mut ids = HashSet::new();
    ids.insert("other.node".to_owned());
    let errors = validate_verify(&node, &ids, "t.agm");
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

// ===========================================================================
// validate_imports — cross-package reference path (I004)
// ===========================================================================

/// Resolver that succeeds for known packages, exposing their node IDs so
/// cross-package ref resolution can check against them.
struct KnownPackageResolver {
    /// Map of package -> node IDs owned by that package.
    known: BTreeMap<String, Vec<String>>,
}

impl ImportResolver for KnownPackageResolver {
    fn resolve(
        &self,
        import: &ValidatedImport,
    ) -> Result<ResolvedPackage, agm_core::error::diagnostic::AgmError> {
        let pkg = import.package();
        let node_ids = self
            .known
            .get(pkg)
            .cloned()
            .ok_or_else(|| {
                agm_core::error::diagnostic::AgmError::new(
                    ErrorCode::I001,
                    format!("unknown: {pkg}"),
                    agm_core::error::diagnostic::ErrorLocation::default(),
                )
            })?;

        // Build a fake AgmFile exposing those nodes.
        let nodes: Vec<Node> = node_ids
            .iter()
            .map(|id| {
                let mut n = blank_node(id);
                n.span = Span::new(1, 1);
                n
            })
            .collect();

        let header = Header {
            agm: "1.0".to_owned(),
            package: pkg.to_owned(),
            version: "1.0.0".to_owned(),
            title: None,
            owner: None,
            imports: None,
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        };
        Ok(ResolvedPackage {
            package: pkg.to_owned(),
            version: semver::Version::new(1, 0, 0),
            path: PathBuf::from("fake/path"),
            file: AgmFile { header, nodes },
        })
    }
}

fn file_with_import_and_cross_ref(
    import_pkg: &str,
    cross_ref: &str,
    agent_ctx_ref: Option<&str>,
) -> AgmFile {
    let mut node = blank_node("local.node");
    node.depends = Some(vec![cross_ref.to_owned()]);
    node.related_to = Some(vec![cross_ref.to_owned()]); // exercise multiple fields
    node.replaces = Some(vec![cross_ref.to_owned()]);
    node.conflicts = Some(vec![cross_ref.to_owned()]);
    node.see_also = Some(vec![cross_ref.to_owned()]);
    if let Some(aref) = agent_ctx_ref {
        node.agent_context = Some(agm_core::model::context::AgentContext {
            load_nodes: Some(vec![aref.to_owned()]),
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: None,
        });
    }

    AgmFile {
        header: Header {
            agm: "1.0".to_owned(),
            package: "myapp".to_owned(),
            version: "0.1.0".to_owned(),
            title: None,
            owner: None,
            imports: Some(vec![ImportEntry::new(import_pkg.to_owned(), None)]),
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        },
        nodes: vec![node],
    }
}

#[test]
fn test_validate_imports_cross_package_ref_resolves_when_node_exists() {
    // Import `shared.security` which exposes `shared.security.login`.
    // Our local file depends on `shared.security.login` — must resolve.
    // Node IDs inside the package are LOCAL (relative). Cross-package refs
    // `shared.security.login` strip the pkg prefix → look up `login`.
    let mut known = BTreeMap::new();
    known.insert("shared.security".to_owned(), vec!["login".to_owned()]);
    let resolver = KnownPackageResolver { known };

    let file = file_with_import_and_cross_ref(
        "shared.security",
        "shared.security.login",
        Some("shared.security.login"),
    );
    let errors = validate_imports(&file, &resolver, "t.agm");
    // No I004 — the cross-package ref resolves.
    assert!(
        !errors.iter().any(|e| e.code == ErrorCode::I004),
        "unexpected I004: {errors:?}"
    );
}

#[test]
fn test_validate_imports_cross_package_ref_missing_node_emits_i004() {
    // Import exists, but the ref points to a node the imported package
    // does NOT expose → I004.
    let mut known = BTreeMap::new();
    known.insert("shared.security".to_owned(), vec!["login".to_owned()]);
    let resolver = KnownPackageResolver { known };

    let file = file_with_import_and_cross_ref(
        "shared.security",
        "shared.security.missing_node",
        None,
    );
    let errors = validate_imports(&file, &resolver, "t.agm");
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::I004),
        "expected I004, got: {errors:?}"
    );
}

#[test]
fn test_validate_imports_non_cross_package_local_ref_not_flagged() {
    // A ref that doesn't start with any imported package prefix is skipped
    // by validate_imports (handled elsewhere).
    let mut known = BTreeMap::new();
    known.insert("shared.security".to_owned(), vec![]);
    let resolver = KnownPackageResolver { known };

    let file = file_with_import_and_cross_ref(
        "shared.security",
        "unrelated.node.ref", // doesn't start with "shared.security."
        None,
    );
    let errors = validate_imports(&file, &resolver, "t.agm");
    // The unrelated ref must NOT be flagged here.
    assert!(
        !errors.iter().any(|e| e.code == ErrorCode::I004),
        "got I004 for non-cross ref: {errors:?}"
    );
}

#[test]
fn test_validate_imports_local_ref_skipped() {
    // A ref that matches a local node id is skipped — no error.
    let mut known = BTreeMap::new();
    known.insert("shared.lib".to_owned(), vec![]);
    let resolver = KnownPackageResolver { known };

    // Build a file where the local node refs itself.
    let mut node = blank_node("local.node");
    node.depends = Some(vec!["local.node".to_owned()]); // self-ref (local)
    let file = AgmFile {
        header: Header {
            agm: "1.0".to_owned(),
            package: "myapp".to_owned(),
            version: "0.1.0".to_owned(),
            title: None,
            owner: None,
            imports: Some(vec![ImportEntry::new("shared.lib".to_owned(), None)]),
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        },
        nodes: vec![node],
    };

    let errors = validate_imports(&file, &resolver, "t.agm");
    assert!(!errors.iter().any(|e| e.code == ErrorCode::I004));
}
