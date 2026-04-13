//! Integration tests for large, complex real-world fixture files.
//!
//! Tests parse, validate, render roundtrip, JSON conversion, diff, and compiler
//! on realistic multi-node AGM files covering all node types and structured fields.

use agm_core::model::fields::Span;
use agm_core::model::file::AgmFile;
use agm_core::model::node::Node;
use agm_core::parser::parse;
use agm_core::renderer::canonical::render_canonical;
use agm_core::renderer::json_canonical::{agm_to_json, json_to_agm};
use agm_core::validator::{ValidateOptions, validate};

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

fn read_fixture(relative: &str) -> String {
    let path = fixtures_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

fn parse_fixture(relative: &str) -> AgmFile {
    let src = read_fixture(relative);
    parse(&src).unwrap_or_else(|errs| panic!("parse errors in {relative}: {errs:?}"))
}

fn strip_spans(file: &AgmFile) -> AgmFile {
    AgmFile {
        header: file.header.clone(),
        nodes: file
            .nodes
            .iter()
            .map(|n| Node {
                span: Span::default(),
                ..n.clone()
            })
            .collect(),
    }
}

// ===========================================================================
// E-Commerce Platform AGM
// ===========================================================================

#[test]
fn test_ecommerce_platform_parses_successfully() {
    let file = parse_fixture("parse/valid/ecommerce_platform.agm");
    assert!(
        file.nodes.len() >= 20,
        "Expected 20+ nodes, got {}",
        file.nodes.len()
    );
}

#[test]
fn test_ecommerce_platform_validates_clean() {
    let src = read_fixture("parse/valid/ecommerce_platform.agm");
    let file = parse(&src).expect("parse should succeed");
    let collection = validate(
        &file,
        &src,
        "ecommerce_platform.agm",
        &ValidateOptions::default(),
    );
    assert!(
        !collection.has_errors(),
        "Validation errors in ecommerce_platform.agm: {:?}",
        collection
            .diagnostics()
            .iter()
            .filter(|d| d.severity == agm_core::error::Severity::Error)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_ecommerce_platform_has_all_node_types() {
    let file = parse_fixture("parse/valid/ecommerce_platform.agm");

    let type_strings: std::collections::HashSet<String> =
        file.nodes.iter().map(|n| n.node_type.to_string()).collect();

    assert!(type_strings.contains("facts"), "missing facts node");
    assert!(type_strings.contains("rules"), "missing rules node");
    assert!(type_strings.contains("workflow"), "missing workflow node");
    assert!(type_strings.contains("entity"), "missing entity node");
    assert!(type_strings.contains("decision"), "missing decision node");
    assert!(type_strings.contains("exception"), "missing exception node");
    assert!(type_strings.contains("example"), "missing example node");
    assert!(type_strings.contains("glossary"), "missing glossary node");
    assert!(type_strings.contains("anti_pattern"), "missing anti_pattern node");
    assert!(type_strings.contains("orchestration"), "missing orchestration node");
}

#[test]
fn test_ecommerce_platform_header_fields() {
    let file = parse_fixture("parse/valid/ecommerce_platform.agm");

    assert_eq!(file.header.package, "ecommerce.platform");
    assert_eq!(file.header.version, "2.3.1");
    assert_eq!(
        file.header.title.as_deref(),
        Some("E-Commerce Platform Core Architecture")
    );
    assert_eq!(file.header.owner.as_deref(), Some("platform-engineering"));
    assert!(file.header.tags.is_some());
    assert!(file.header.load_profiles.is_some());
    assert_eq!(
        file.header.target_runtime.as_deref(),
        Some("octopus")
    );
}

#[test]
fn test_ecommerce_platform_relationships() {
    let file = parse_fixture("parse/valid/ecommerce_platform.agm");

    // Order lifecycle should depend on platform.constraints and catalog.product
    let order = file
        .nodes
        .iter()
        .find(|n| n.id == "order.lifecycle")
        .expect("order.lifecycle node should exist");
    let deps = order.depends.as_ref().expect("should have depends");
    assert!(deps.contains(&"platform.constraints".to_string()));
    assert!(deps.contains(&"catalog.product".to_string()));

    // Payment browser storage should conflict
    let antipattern = file
        .nodes
        .iter()
        .find(|n| n.id == "payment.browser.storage")
        .expect("payment.browser.storage node should exist");
    assert!(antipattern.conflicts.is_some());
}

#[test]
fn test_ecommerce_platform_structured_fields() {
    let file = parse_fixture("parse/valid/ecommerce_platform.agm");

    // inventory.reserve should have code, verify, and agent_context
    let inv = file
        .nodes
        .iter()
        .find(|n| n.id == "inventory.reserve")
        .expect("inventory.reserve node should exist");
    assert!(inv.code.is_some(), "should have code block");
    assert!(inv.verify.is_some(), "should have verify checks");
    assert!(inv.agent_context.is_some(), "should have agent_context");
    assert_eq!(inv.execution_status.as_ref().map(|s| s.to_string()).as_deref(), Some("pending"));

    // deploy.canary should have code_blocks (multiple)
    let deploy = file
        .nodes
        .iter()
        .find(|n| n.id == "deploy.canary.workflow")
        .expect("deploy.canary.workflow node should exist");
    let blocks = deploy.code_blocks.as_ref().expect("should have code_blocks");
    assert!(blocks.len() >= 2, "should have at least 2 code blocks");

    // shipping should have memory
    let shipping = file
        .nodes
        .iter()
        .find(|n| n.id == "shipping.estimation")
        .expect("shipping.estimation node should exist");
    assert!(shipping.memory.is_some(), "should have memory entries");

    // orchestration should have parallel_groups
    let orch = file
        .nodes
        .iter()
        .find(|n| n.id == "rollout.orchestration")
        .expect("rollout.orchestration node should exist");
    let groups = orch
        .parallel_groups
        .as_ref()
        .expect("should have parallel_groups");
    assert!(groups.len() >= 3, "should have at least 3 groups");
}

#[test]
fn test_ecommerce_platform_temporal_fields() {
    let file = parse_fixture("parse/valid/ecommerce_platform.agm");

    let session = file
        .nodes
        .iter()
        .find(|n| n.id == "customer.session")
        .expect("customer.session node should exist");
    assert!(session.valid_from.is_some(), "should have valid_from");
    assert!(session.valid_until.is_some(), "should have valid_until");
    assert!(session.scope.is_some(), "should have scope");
}

#[test]
fn test_ecommerce_platform_canonical_roundtrip() {
    let file1 = parse_fixture("parse/valid/ecommerce_platform.agm");
    let canonical = render_canonical(&file1);
    let file2 = parse(&canonical).unwrap_or_else(|errs| {
        panic!("re-parse of canonical output failed: {errs:?}\n\nCanonical:\n{canonical}");
    });
    assert_eq!(strip_spans(&file1), strip_spans(&file2));
}

#[test]
fn test_ecommerce_platform_json_roundtrip() {
    let file1 = parse_fixture("parse/valid/ecommerce_platform.agm");
    let json1 = agm_to_json(&file1);
    let file2 = json_to_agm(&json1).expect("json_to_agm should succeed");
    let json2 = agm_to_json(&file2);
    assert_eq!(json1, json2, "JSON round-trip should be lossless");
}

// ===========================================================================
// CI/CD Pipeline AGM
// ===========================================================================

#[test]
fn test_cicd_pipeline_parses_successfully() {
    let file = parse_fixture("parse/valid/cicd_pipeline.agm");
    assert!(
        file.nodes.len() >= 14,
        "Expected 14+ nodes, got {}",
        file.nodes.len()
    );
}

#[test]
fn test_cicd_pipeline_validates_clean() {
    let src = read_fixture("parse/valid/cicd_pipeline.agm");
    let file = parse(&src).expect("parse should succeed");
    let collection = validate(
        &file,
        &src,
        "cicd_pipeline.agm",
        &ValidateOptions::default(),
    );
    assert!(
        !collection.has_errors(),
        "Validation errors in cicd_pipeline.agm: {:?}",
        collection
            .diagnostics()
            .iter()
            .filter(|d| d.severity == agm_core::error::Severity::Error)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_cicd_pipeline_orchestration_groups() {
    let file = parse_fixture("parse/valid/cicd_pipeline.agm");

    let orch = file
        .nodes
        .iter()
        .find(|n| n.id == "ci.pipeline.orchestration")
        .expect("ci.pipeline.orchestration node should exist");

    let groups = orch
        .parallel_groups
        .as_ref()
        .expect("should have parallel_groups");

    assert_eq!(groups.len(), 5, "should have 5 pipeline phases");

    // Verify group dependencies form a chain
    let group_names: Vec<_> = groups.iter().map(|g| g.group.as_str()).collect();
    assert_eq!(
        group_names,
        vec!["1-build", "2-quality-gates", "3-integration", "4-staging", "5-production"]
    );

    // Quality gates should run in parallel
    let qg = groups.iter().find(|g| g.group == "2-quality-gates").unwrap();
    assert_eq!(qg.strategy.to_string(), "parallel");
    assert_eq!(qg.nodes.len(), 2);
}

#[test]
fn test_cicd_pipeline_code_blocks_variety() {
    let file = parse_fixture("parse/valid/cicd_pipeline.agm");

    // Security scan should have code_blocks with yaml and toml
    let security = file
        .nodes
        .iter()
        .find(|n| n.id == "ci.security.scan")
        .expect("ci.security.scan node should exist");
    let blocks = security
        .code_blocks
        .as_ref()
        .expect("should have code_blocks");
    assert_eq!(blocks.len(), 2);

    let langs: Vec<_> = blocks
        .iter()
        .filter_map(|b| b.lang.as_deref())
        .collect();
    assert!(langs.contains(&"yaml"));
    assert!(langs.contains(&"toml"));

    // Container build should have dockerfile
    let container = file
        .nodes
        .iter()
        .find(|n| n.id == "ci.container.build")
        .expect("ci.container.build node should exist");
    let code = container.code.as_ref().expect("should have code block");
    assert_eq!(code.lang.as_deref(), Some("dockerfile"));
}

#[test]
fn test_cicd_pipeline_verify_checks() {
    let file = parse_fixture("parse/valid/cicd_pipeline.agm");

    // Build step should have verify with file_exists, file_contains, and command
    let build = file
        .nodes
        .iter()
        .find(|n| n.id == "ci.build.rust")
        .expect("ci.build.rust node should exist");
    let checks = build.verify.as_ref().expect("should have verify checks");
    assert!(checks.len() >= 2);
}

#[test]
fn test_cicd_pipeline_memory_operations() {
    let file = parse_fixture("parse/valid/cicd_pipeline.agm");

    let staging = file
        .nodes
        .iter()
        .find(|n| n.id == "ci.deploy.staging")
        .expect("ci.deploy.staging node should exist");
    let memory = staging.memory.as_ref().expect("should have memory");
    assert!(memory.len() >= 2);
}

#[test]
fn test_cicd_pipeline_canonical_roundtrip() {
    let file1 = parse_fixture("parse/valid/cicd_pipeline.agm");
    let canonical = render_canonical(&file1);
    let file2 = parse(&canonical).unwrap_or_else(|errs| {
        panic!("re-parse of canonical output failed: {errs:?}");
    });
    assert_eq!(strip_spans(&file1), strip_spans(&file2));
}

#[test]
fn test_cicd_pipeline_json_roundtrip() {
    let file1 = parse_fixture("parse/valid/cicd_pipeline.agm");
    let json1 = agm_to_json(&file1);
    let file2 = json_to_agm(&json1).expect("json_to_agm should succeed");
    let json2 = agm_to_json(&file2);
    assert_eq!(json1, json2);
}

// ===========================================================================
// JSON Forward: full_platform
// ===========================================================================

#[test]
fn test_json_forward_full_platform() {
    let file = parse_fixture("json/forward/full_platform.agm");
    let actual = agm_to_json(&file);
    let path = fixtures_root().join("json/forward/full_platform.json");
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("cannot read golden JSON"),
    )
    .expect("invalid JSON in golden file");
    assert_eq!(
        actual, expected,
        "forward conversion mismatch for full_platform"
    );
}

// ===========================================================================
// JSON Roundtrip: full_platform
// ===========================================================================

#[test]
fn test_json_roundtrip_full_platform() {
    let path = fixtures_root().join("json/roundtrip/full_platform.json");
    let json1: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("cannot read roundtrip JSON"),
    )
    .expect("invalid JSON");
    let file = json_to_agm(&json1).expect("json_to_agm should succeed");
    let json2 = agm_to_json(&file);
    assert_eq!(json1, json2, "JSON round-trip failed for full_platform");
}

// ===========================================================================
// Diff: full_platform_evolution
// ===========================================================================

#[test]
fn test_diff_full_platform_evolution_summary() {
    let left = parse_fixture("diff/full_platform_evolution/left.agm");
    let right = parse_fixture("diff/full_platform_evolution/right.agm");
    let report = agm_core::diff::diff(&left, &right);

    // Should detect added nodes
    assert!(
        !report.added_nodes.is_empty(),
        "should detect added nodes (passkey.registration, session.decision, orchestration)"
    );

    // auth.mfa was removed
    assert!(
        report.removed_nodes.contains(&"auth.mfa".to_string()),
        "should detect auth.mfa removal"
    );

    // Modified nodes should include auth.stack, auth.constraints, auth.session, auth.login, auth.logout
    let modified_ids: Vec<_> = report.modified_nodes.iter().map(|n| n.node_id.as_str()).collect();
    assert!(
        modified_ids.contains(&"auth.stack"),
        "auth.stack should be modified"
    );
    assert!(
        modified_ids.contains(&"auth.login"),
        "auth.login should be modified"
    );

    // Header changes: version bump 1.2.0 -> 2.0.0
    let version_change = report
        .header_changes
        .iter()
        .find(|hc| hc.field == "version");
    assert!(version_change.is_some(), "version should have changed");
}

#[test]
fn test_diff_full_platform_evolution_breaking_changes() {
    let left = parse_fixture("diff/full_platform_evolution/left.agm");
    let right = parse_fixture("diff/full_platform_evolution/right.agm");
    let report = agm_core::diff::diff(&left, &right);

    // Removing auth.mfa is a breaking change
    assert!(
        report.has_breaking_changes(),
        "removing a node should be a breaking change"
    );
}

#[test]
fn test_diff_full_platform_evolution_render_all_formats() {
    use agm_core::diff::render::{DiffFormat, render_diff};

    let left = parse_fixture("diff/full_platform_evolution/left.agm");
    let right = parse_fixture("diff/full_platform_evolution/right.agm");
    let report = agm_core::diff::diff(&left, &right);

    let text = render_diff(&report, DiffFormat::Text);
    assert!(!text.is_empty(), "text diff should not be empty");

    let json = render_diff(&report, DiffFormat::Json);
    let _: serde_json::Value =
        serde_json::from_str(&json).expect("JSON diff output should be valid JSON");

    let md = render_diff(&report, DiffFormat::Markdown);
    assert!(!md.is_empty(), "markdown diff should not be empty");
}

// ===========================================================================
// Compiler: complex markdown files
// ===========================================================================

#[cfg(feature = "compiler")]
mod compiler_tests {
    use agm_core::compiler::{CompileOptions, compile};
    use agm_core::parser;
    use agm_core::renderer::canonical::render_canonical;

    fn default_opts(package: &str) -> CompileOptions {
        CompileOptions {
            package: package.to_owned(),
            version: "0.1.0".to_owned(),
            min_confidence: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_compile_ecommerce_checkout_produces_multiple_nodes() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/ecommerce_checkout.md");
        let result = compile(md, &default_opts("ecommerce.checkout"));
        assert!(
            result.file.nodes.len() >= 5,
            "Expected 5+ nodes from ecommerce checkout markdown, got {}",
            result.file.nodes.len()
        );
    }

    #[test]
    fn test_compile_ecommerce_checkout_roundtrip_parses() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/ecommerce_checkout.md");
        let result = compile(md, &default_opts("ecommerce.checkout"));
        let agm_text = render_canonical(&result.file);
        let parsed = parser::parse(&agm_text);
        assert!(parsed.is_ok(), "Compiled AGM failed to parse: {parsed:?}");
    }

    #[test]
    fn test_compile_ecommerce_checkout_has_code_blocks() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/ecommerce_checkout.md");
        let result = compile(md, &default_opts("ecommerce.checkout"));
        let nodes_with_code: Vec<_> = result
            .file
            .nodes
            .iter()
            .filter(|n| n.code.is_some() || n.code_blocks.is_some())
            .collect();
        assert!(
            !nodes_with_code.is_empty(),
            "Should extract code blocks from markdown"
        );
    }

    #[test]
    fn test_compile_microservices_migration_produces_nodes() {
        let md =
            include_str!("../../../tests/fixtures/compiler/valid/microservices_migration.md");
        let result = compile(md, &default_opts("migration.plan"));
        assert!(
            result.file.nodes.len() >= 7,
            "Expected 7+ nodes from microservices migration markdown, got {}",
            result.file.nodes.len()
        );
    }

    #[test]
    fn test_compile_microservices_migration_roundtrip_parses() {
        let md =
            include_str!("../../../tests/fixtures/compiler/valid/microservices_migration.md");
        let result = compile(md, &default_opts("migration.plan"));
        let agm_text = render_canonical(&result.file);
        let parsed = parser::parse(&agm_text);
        assert!(
            parsed.is_ok(),
            "Compiled AGM failed to parse: {parsed:?}"
        );
    }

    #[test]
    fn test_compile_microservices_migration_has_code_and_entity() {
        let md =
            include_str!("../../../tests/fixtures/compiler/valid/microservices_migration.md");
        let result = compile(md, &default_opts("migration.plan"));

        // Should have extracted code blocks from SQL and YAML examples
        let has_code = result
            .file
            .nodes
            .iter()
            .any(|n| n.code.is_some() || n.code_blocks.is_some());
        assert!(has_code, "Should extract code blocks from SQL/YAML");

        // Should have entity nodes from the entity sections
        let has_entity = result.file.nodes.iter().any(|n| {
            n.node_type.to_string() == "entity"
        });
        assert!(has_entity, "Should detect entity node from user service section");
    }

    #[test]
    fn test_compile_ecommerce_checkout_snapshot() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/ecommerce_checkout.md");
        let result = compile(md, &default_opts("ecommerce.checkout"));
        let agm_text = render_canonical(&result.file);
        insta::assert_snapshot!("compiler__ecommerce_checkout", agm_text);
    }

    #[test]
    fn test_compile_microservices_migration_snapshot() {
        let md =
            include_str!("../../../tests/fixtures/compiler/valid/microservices_migration.md");
        let result = compile(md, &default_opts("migration.plan"));
        let agm_text = render_canonical(&result.file);
        insta::assert_snapshot!("compiler__microservices_migration", agm_text);
    }
}
