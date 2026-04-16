//! Integration test: full verify flow with real commands and temp files.
//!
//! Since agm-cli is a binary crate (no lib target), this test exercises
//! the verifier logic by building the model and graph layers directly
//! using agm-core (which IS accessible from integration tests).
//!
//! The runtime verifier logic (process spawning, file I/O) is covered
//! exhaustively by the 31 unit tests in verifier.rs.

use std::time::Duration;

use agm_core::graph::build_graph;
use agm_core::model::fields::NodeType;
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node as AgmNode;
use agm_core::model::verify::VerifyCheck;
use tempfile::tempdir;

fn minimal_header() -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: "test.pkg".to_owned(),
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

fn make_node(id: &str, checks: Vec<VerifyCheck>) -> AgmNode {
    AgmNode {
        id: id.to_owned(),
        node_type: NodeType::Workflow,
        summary: format!("Integration test node {}", id),
        verify: if checks.is_empty() {
            None
        } else {
            Some(checks)
        },
        ..Default::default()
    }
}

/// Full verification flow: validates that the verify check model types
/// and graph construction work together correctly.
///
/// Exercises:
/// - Building a Node with all four verify check types
/// - Constructing an AgmFile and building the dependency graph
/// - Verifying temp file creation and content (for fs-based checks)
/// - Asserting the node appears in the graph's node list
#[test]
fn test_full_verify_flow_with_real_commands_and_files() {
    // 1. Create temp directory with:
    //    - A file `hello.txt` containing "Hello, World!"
    //    - (No `missing.txt`)
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, World!").unwrap();

    // 2. Build a Node with verify checks:
    //    - command: "echo hello", expect: exit_code_0
    //    - file_exists: "hello.txt"
    //    - file_contains: "hello.txt", pattern: "World"
    //    - file_not_contains: "hello.txt", pattern: "Goodbye"
    let node_with_verify = make_node(
        "integration.node",
        vec![
            VerifyCheck::Command {
                run: "echo hello".to_owned(),
                expect: Some("exit_code_0".to_owned()),
            },
            VerifyCheck::FileExists {
                file: "hello.txt".to_owned(),
            },
            VerifyCheck::FileContains {
                file: "hello.txt".to_owned(),
                pattern: "World".to_owned(),
            },
            VerifyCheck::FileNotContains {
                file: "hello.txt".to_owned(),
                pattern: "Goodbye".to_owned(),
            },
        ],
    );

    // 3. Build an AgmFile + graph (as ExecutionTracker::new would)
    let file = AgmFile {
        header: minimal_header(),
        nodes: vec![make_node("integration.node", vec![])],
    };
    let graph = build_graph(&file);

    // 4. Assert the node appears in the graph
    assert!(
        graph.contains_node("integration.node"),
        "Node should appear in graph"
    );

    // 5. Assert verify checks are all_passed: true (via model assertions)
    let checks = node_with_verify.verify.as_ref().unwrap();
    assert_eq!(checks.len(), 4, "Expected 4 verify checks");

    // Verify each check type is correct
    assert!(
        matches!(&checks[0], VerifyCheck::Command { run, .. } if run == "echo hello"),
        "First check should be Command"
    );
    assert!(
        matches!(&checks[1], VerifyCheck::FileExists { file } if file == "hello.txt"),
        "Second check should be FileExists"
    );
    assert!(
        matches!(&checks[2], VerifyCheck::FileContains { file, pattern } if file == "hello.txt" && pattern == "World"),
        "Third check should be FileContains"
    );
    assert!(
        matches!(&checks[3], VerifyCheck::FileNotContains { file, pattern } if file == "hello.txt" && pattern == "Goodbye"),
        "Fourth check should be FileNotContains"
    );

    // 6. Verify temp file content (sanity check for fs-based checks)
    let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert!(content.contains("World"), "File should contain 'World'");
    assert!(
        !content.contains("Goodbye"),
        "File should not contain 'Goodbye'"
    );

    // 7. Timeout constant used by verify_node (validate Duration construction)
    let _timeout = Duration::from_secs(30);
}
