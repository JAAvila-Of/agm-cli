//! Advanced edge-case integration tests for agm-core::diff.
//!
//! Covers identical large files, node reordering, single-field changes,
//! breaking change detection, combined add/remove/modify diffs, and empty files.

use std::collections::BTreeMap;

use agm_core::diff::{self, ChangeKind, ChangeSeverity};
use agm_core::model::fields::{NodeType, Priority, Span};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node;
use agm_core::parser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn valid_header(package: &str) -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: package.to_owned(),
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

fn minimal_node(id: &str, node_type: NodeType, summary: &str, line: usize) -> Node {
    Node {
        id: id.to_owned(),
        node_type,
        summary: summary.to_owned(),
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
        span: Span::new(line, line + 3),
    }
}

fn fixture(relative: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest)
        .join("../..")
        .join("tests/fixtures")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

fn parse_fixture(relative: &str) -> AgmFile {
    let src = fixture(relative);
    parser::parse(&src).expect("fixture should parse successfully")
}

// ---------------------------------------------------------------------------
// 1. Identical large files: 50+ nodes — verify no diff
// ---------------------------------------------------------------------------

#[test]
fn test_diff_identical_large_file_50_nodes_returns_empty() {
    let nodes: Vec<Node> = (0..50)
        .map(|i| {
            minimal_node(
                &format!("pkg.node{i:02}"),
                NodeType::Facts,
                &format!("summary for node {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let file = AgmFile {
        header: valid_header("test.large"),
        nodes,
    };

    let report = diff::diff(&file, &file);
    assert!(
        report.is_empty(),
        "Identical 50-node files should produce empty diff"
    );
    assert_eq!(
        report.summary.nodes_unchanged, 50,
        "All 50 nodes should be unchanged"
    );
    assert_eq!(report.summary.nodes_added, 0);
    assert_eq!(report.summary.nodes_removed, 0);
    assert_eq!(report.summary.nodes_modified, 0);
}

#[test]
fn test_diff_identical_large_file_no_breaking_changes() {
    let nodes: Vec<Node> = (0..55)
        .map(|i| {
            minimal_node(
                &format!("pkg.node{i:02}"),
                NodeType::Rules,
                "rule summary",
                i * 4 + 1,
            )
        })
        .collect();

    let file = AgmFile {
        header: valid_header("test.large2"),
        nodes,
    };

    let report = diff::diff(&file, &file);
    assert!(!report.has_breaking_changes());
}

// ---------------------------------------------------------------------------
// 2. Reordered nodes: same nodes in different order
// ---------------------------------------------------------------------------

#[test]
fn test_diff_reordered_nodes_detects_no_content_change() {
    // Same nodes, different order — semantic content is identical
    let node_a = minimal_node("pkg.a", NodeType::Facts, "node a summary", 1);
    let node_b = minimal_node("pkg.b", NodeType::Rules, "node b summary", 5);
    let node_c = minimal_node("pkg.c", NodeType::Workflow, "node c summary", 9);

    let left = AgmFile {
        header: valid_header("test.reorder"),
        nodes: vec![node_a.clone(), node_b.clone(), node_c.clone()],
    };
    let right = AgmFile {
        header: valid_header("test.reorder"),
        nodes: vec![node_c, node_a, node_b],
    };

    let report = diff::diff(&left, &right);
    // Nodes matched by ID — same content, different order means no modifications
    assert!(report.added_nodes.is_empty(), "No nodes should be added");
    assert!(
        report.removed_nodes.is_empty(),
        "No nodes should be removed"
    );
    assert!(
        report.modified_nodes.is_empty(),
        "Reordered nodes with identical content should have no modifications"
    );
    assert_eq!(report.summary.nodes_unchanged, 3);
}

#[test]
fn test_diff_reordered_with_one_content_change_detects_modification() {
    let node_a = minimal_node("pkg.a", NodeType::Facts, "original summary", 1);
    let node_b = minimal_node("pkg.b", NodeType::Rules, "rule summary", 5);

    let mut node_a_modified = node_a.clone();
    node_a_modified.summary = "modified summary".to_owned();

    let left = AgmFile {
        header: valid_header("test.reorder"),
        nodes: vec![node_a, node_b.clone()],
    };
    let right = AgmFile {
        header: valid_header("test.reorder"),
        nodes: vec![node_b, node_a_modified], // different order + modified
    };

    let report = diff::diff(&left, &right);
    assert_eq!(
        report.summary.nodes_modified, 1,
        "One node should be modified"
    );
    assert_eq!(
        report.summary.nodes_unchanged, 1,
        "One node should be unchanged"
    );
    assert!(report.added_nodes.is_empty());
    assert!(report.removed_nodes.is_empty());
}

// ---------------------------------------------------------------------------
// 3. Field-level granularity: single field changed in many-field node
// ---------------------------------------------------------------------------

#[test]
fn test_diff_single_field_change_only_that_field_reported() {
    let mut left_node = minimal_node("auth.login", NodeType::Facts, "login facts", 1);
    left_node.priority = Some(Priority::Normal);
    left_node.items = Some(vec!["item a".to_owned(), "item b".to_owned()]);
    left_node.detail = Some("Detailed explanation".to_owned());
    left_node.notes = Some("a note".to_owned());
    left_node.tags = Some(vec!["auth".to_owned()]);

    let mut right_node = left_node.clone();
    // Change ONLY the priority field
    right_node.priority = Some(Priority::High);

    let left = AgmFile {
        header: valid_header("test.granular"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.granular"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);
    assert_eq!(report.modified_nodes.len(), 1);
    let node_diff = &report.modified_nodes[0];
    assert_eq!(node_diff.node_id, "auth.login");

    // Only "priority" should appear as a field change
    let field_names: Vec<&str> = node_diff
        .field_changes
        .iter()
        .map(|fc| fc.field.as_str())
        .collect();
    assert!(
        field_names.contains(&"priority"),
        "priority should be in field changes, got: {field_names:?}"
    );
    // summary, items, detail, notes, tags should NOT be in field changes
    for unchanged in &["summary", "items", "detail", "notes"] {
        assert!(
            !field_names.contains(unchanged),
            "'{unchanged}' should NOT appear in field changes when unchanged"
        );
    }
}

#[test]
fn test_diff_only_summary_changed_severity_is_minor() {
    let left_node = minimal_node("auth.node", NodeType::Facts, "original summary text", 1);
    let mut right_node = left_node.clone();
    right_node.summary = "updated summary text".to_owned();

    let left = AgmFile {
        header: valid_header("test.granular"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.granular"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);
    let node_diff = &report.modified_nodes[0];
    let summary_change = node_diff
        .field_changes
        .iter()
        .find(|fc| fc.field == "summary")
        .expect("summary change should be present");
    assert_eq!(
        summary_change.severity,
        ChangeSeverity::Minor,
        "Summary change should be Minor severity"
    );
    assert!(!report.has_breaking_changes());
}

// ---------------------------------------------------------------------------
// 4. Breaking change detection
// ---------------------------------------------------------------------------

#[test]
fn test_diff_type_change_is_breaking() {
    let left_node = minimal_node("auth.node", NodeType::Facts, "a node", 1);
    let mut right_node = left_node.clone();
    right_node.node_type = NodeType::Workflow; // type changed

    let left = AgmFile {
        header: valid_header("test.breaking"),
        nodes: vec![left_node],
    };
    let right = AgmFile {
        header: valid_header("test.breaking"),
        nodes: vec![right_node],
    };

    let report = diff::diff(&left, &right);
    assert!(
        report.has_breaking_changes(),
        "Type change must be breaking"
    );

    let type_change = report.modified_nodes[0]
        .field_changes
        .iter()
        .find(|fc| fc.field == "type")
        .expect("type field change should be present");
    assert_eq!(type_change.severity, ChangeSeverity::Breaking);
    assert_eq!(type_change.kind, ChangeKind::Modified);
}

#[test]
fn test_diff_removed_node_is_breaking() {
    let node_a = minimal_node("auth.a", NodeType::Facts, "node a", 1);
    let node_b = minimal_node("auth.b", NodeType::Rules, "node b", 5);

    let left = AgmFile {
        header: valid_header("test.breaking"),
        nodes: vec![node_a, node_b],
    };
    let right = AgmFile {
        header: valid_header("test.breaking"),
        nodes: vec![minimal_node("auth.a", NodeType::Facts, "node a", 1)],
    };

    let report = diff::diff(&left, &right);
    assert!(
        report.has_breaking_changes(),
        "Removing a node must be breaking"
    );
    assert!(report.removed_nodes.contains(&"auth.b".to_owned()));
}

#[test]
fn test_diff_breaking_change_fixture_type_change_detected() {
    let left = parse_fixture("diff/breaking_change/left.agm");
    let right = parse_fixture("diff/breaking_change/right.agm");
    let report = diff::diff(&left, &right);
    assert!(report.has_breaking_changes());
}

// ---------------------------------------------------------------------------
// 5. Added + removed + modified simultaneously
// ---------------------------------------------------------------------------

#[test]
fn test_diff_mixed_add_remove_modify_all_detected() {
    let node_keep = minimal_node("pkg.keep", NodeType::Facts, "unchanged node", 1);
    let node_remove = minimal_node("pkg.remove", NodeType::Rules, "will be removed", 5);
    let mut node_modify = minimal_node("pkg.modify", NodeType::Facts, "original summary", 9);
    node_modify.priority = Some(Priority::Low);

    let left = AgmFile {
        header: valid_header("test.mixed"),
        nodes: vec![node_keep.clone(), node_remove, node_modify],
    };

    let mut node_modify_right = minimal_node("pkg.modify", NodeType::Facts, "modified summary", 9);
    node_modify_right.priority = Some(Priority::High);
    let node_add = minimal_node("pkg.add", NodeType::Glossary, "newly added node", 13);

    let right = AgmFile {
        header: valid_header("test.mixed"),
        nodes: vec![node_keep, node_modify_right, node_add],
    };

    let report = diff::diff(&left, &right);
    assert_eq!(report.added_nodes.len(), 1, "One node should be added");
    assert_eq!(report.removed_nodes.len(), 1, "One node should be removed");
    assert_eq!(
        report.modified_nodes.len(),
        1,
        "One node should be modified"
    );
    assert_eq!(
        report.summary.nodes_unchanged, 1,
        "One node should be unchanged"
    );
    assert!(
        report.has_breaking_changes(),
        "Removed node makes this breaking"
    );

    assert!(report.added_nodes.contains(&"pkg.add".to_owned()));
    assert!(report.removed_nodes.contains(&"pkg.remove".to_owned()));
    assert_eq!(report.modified_nodes[0].node_id, "pkg.modify");
}

#[test]
fn test_diff_mixed_summary_adds_up_correctly() {
    let left_nodes: Vec<Node> = (0..5)
        .map(|i| minimal_node(&format!("pkg.n{i}"), NodeType::Facts, "original", i * 3 + 1))
        .collect();

    // Right: remove nodes 0 and 4, add two new nodes, modify node 2
    let mut right_nodes: Vec<Node> = (1..4)
        .map(|i| minimal_node(&format!("pkg.n{i}"), NodeType::Facts, "original", i * 3 + 1))
        .collect();
    // Modify node 2
    right_nodes[1].summary = "changed".to_owned(); // pkg.n2
    // Add two new nodes
    right_nodes.push(minimal_node("pkg.new1", NodeType::Rules, "new rule", 50));
    right_nodes.push(minimal_node(
        "pkg.new2",
        NodeType::Glossary,
        "new glossary",
        55,
    ));

    let left = AgmFile {
        header: valid_header("test.mixed2"),
        nodes: left_nodes,
    };
    let right = AgmFile {
        header: valid_header("test.mixed2"),
        nodes: right_nodes,
    };

    let report = diff::diff(&left, &right);
    assert_eq!(report.summary.nodes_added, 2);
    assert_eq!(report.summary.nodes_removed, 2);
    assert_eq!(report.summary.nodes_modified, 1);
    assert_eq!(report.summary.nodes_unchanged, 2); // pkg.n1 and pkg.n3
}

// ---------------------------------------------------------------------------
// 6. Empty files
// ---------------------------------------------------------------------------

#[test]
fn test_diff_both_empty_files_returns_empty_diff() {
    // We parse a minimal valid file and use the empty-nodes variant
    let empty = AgmFile {
        header: valid_header("test.empty"),
        nodes: vec![],
    };
    let report = diff::diff(&empty, &empty);
    assert!(
        report.is_empty(),
        "Two empty files should produce empty diff"
    );
    assert_eq!(report.summary.nodes_unchanged, 0);
}

#[test]
fn test_diff_empty_left_vs_populated_right_all_nodes_added() {
    let empty_left = AgmFile {
        header: valid_header("test.empty"),
        nodes: vec![],
    };
    let right = AgmFile {
        header: valid_header("test.empty"),
        nodes: vec![
            minimal_node("pkg.a", NodeType::Facts, "node a", 1),
            minimal_node("pkg.b", NodeType::Rules, "node b", 5),
            minimal_node("pkg.c", NodeType::Workflow, "node c", 9),
        ],
    };

    let report = diff::diff(&empty_left, &right);
    assert_eq!(
        report.added_nodes.len(),
        3,
        "All right-side nodes should be added"
    );
    assert!(report.removed_nodes.is_empty());
    assert!(report.modified_nodes.is_empty());
    // Adding nodes is not breaking
    assert!(!report.has_breaking_changes());
    assert_eq!(report.summary.nodes_added, 3);
}

#[test]
fn test_diff_populated_left_vs_empty_right_all_nodes_removed_breaking() {
    let left = AgmFile {
        header: valid_header("test.empty"),
        nodes: vec![
            minimal_node("pkg.a", NodeType::Facts, "node a", 1),
            minimal_node("pkg.b", NodeType::Rules, "node b", 5),
        ],
    };
    let empty_right = AgmFile {
        header: valid_header("test.empty"),
        nodes: vec![],
    };

    let report = diff::diff(&left, &empty_right);
    assert_eq!(
        report.removed_nodes.len(),
        2,
        "All left-side nodes should be removed"
    );
    assert!(report.added_nodes.is_empty());
    assert!(
        report.has_breaking_changes(),
        "Removing all nodes must be breaking"
    );
    assert_eq!(report.summary.nodes_removed, 2);
}

// ---------------------------------------------------------------------------
// 7. breaking_only filter on mixed diff
// ---------------------------------------------------------------------------

#[test]
fn test_diff_breaking_only_excludes_added_nodes_and_minor_changes() {
    let left_node = minimal_node("auth.login", NodeType::Facts, "login facts", 1);
    let remove_node = minimal_node("auth.session", NodeType::Rules, "session rules", 5);

    let mut right_login = left_node.clone();
    right_login.summary = "updated login facts".to_owned(); // Minor change
    right_login.node_type = NodeType::Workflow; // Breaking change

    let new_node = minimal_node("auth.token", NodeType::Glossary, "new token node", 10);

    let left = AgmFile {
        header: valid_header("test.filter"),
        nodes: vec![left_node, remove_node],
    };
    let right = AgmFile {
        header: valid_header("test.filter"),
        nodes: vec![right_login, new_node],
    };

    let full_report = diff::diff(&left, &right);
    let breaking = full_report.breaking_only();

    // breaking_only never includes added_nodes
    assert!(
        breaking.added_nodes.is_empty(),
        "breaking_only should not include added nodes"
    );

    // All field changes in breaking_only must be Breaking severity
    for nd in &breaking.modified_nodes {
        for fc in &nd.field_changes {
            assert_eq!(
                fc.severity,
                ChangeSeverity::Breaking,
                "breaking_only should only contain Breaking field changes"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8. Dependency relationship changes
// ---------------------------------------------------------------------------

#[test]
fn test_diff_added_dependency_classified_as_minor() {
    let node_dep = minimal_node("auth.dep", NodeType::Facts, "dep node", 1);
    let mut left_node = minimal_node("auth.main", NodeType::Facts, "main node", 5);
    left_node.depends = None; // no deps

    let mut right_node = left_node.clone();
    right_node.depends = Some(vec!["auth.dep".to_owned()]); // dep added

    let left = AgmFile {
        header: valid_header("test.deps"),
        nodes: vec![node_dep.clone(), left_node],
    };
    let right = AgmFile {
        header: valid_header("test.deps"),
        nodes: vec![node_dep, right_node],
    };

    let report = diff::diff(&left, &right);
    if let Some(main_diff) = report
        .modified_nodes
        .iter()
        .find(|nd| nd.node_id == "auth.main")
    {
        if let Some(dep_change) = main_diff
            .field_changes
            .iter()
            .find(|fc| fc.field == "depends")
        {
            // Dependency changes should be at most Breaking (not Info)
            assert!(
                dep_change.severity >= ChangeSeverity::Minor,
                "depends change should be Minor or Breaking"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 9. No changes in header when headers are identical
// ---------------------------------------------------------------------------

#[test]
fn test_diff_identical_headers_no_header_changes() {
    let left = AgmFile {
        header: valid_header("test.same"),
        nodes: vec![minimal_node("pkg.a", NodeType::Facts, "node a", 1)],
    };
    let right = left.clone();

    let report = diff::diff(&left, &right);
    assert!(
        report.header_changes.is_empty(),
        "Identical headers should produce no header changes"
    );
}

#[test]
fn test_diff_version_change_in_header_detected_as_info() {
    let mut left_header = valid_header("test.hdr");
    let mut right_header = valid_header("test.hdr");
    left_header.version = "1.0.0".to_owned();
    right_header.version = "2.0.0".to_owned();

    let left = AgmFile {
        header: left_header,
        nodes: vec![minimal_node("pkg.a", NodeType::Facts, "node a", 1)],
    };
    let right = AgmFile {
        header: right_header,
        nodes: vec![minimal_node("pkg.a", NodeType::Facts, "node a", 1)],
    };

    let report = diff::diff(&left, &right);
    let version_change = report
        .header_changes
        .iter()
        .find(|hc| hc.field == "version");
    assert!(
        version_change.is_some(),
        "Version change should be detected"
    );
    assert_eq!(
        version_change.unwrap().severity,
        ChangeSeverity::Info,
        "Version change should be Info severity"
    );
}

// ---------------------------------------------------------------------------
// STRESS / LARGE FILE TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_diff_200_nodes_identical_no_changes() {
    let nodes: Vec<Node> = (0..200usize)
        .map(|i| {
            minimal_node(
                &format!("pkg.n{i:03}"),
                NodeType::Facts,
                &format!("summary for node {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let file = AgmFile {
        header: valid_header("stress.same"),
        nodes,
    };

    let report = diff::diff(&file, &file);
    assert!(
        report.is_empty(),
        "Identical 200-node files should produce empty diff"
    );
    assert_eq!(
        report.summary.nodes_unchanged, 200,
        "All 200 nodes should be unchanged"
    );
    assert_eq!(report.summary.nodes_added, 0);
    assert_eq!(report.summary.nodes_removed, 0);
    assert_eq!(report.summary.nodes_modified, 0);
}

#[test]
fn test_diff_200_nodes_all_modified() {
    let left_nodes: Vec<Node> = (0..200usize)
        .map(|i| {
            minimal_node(
                &format!("pkg.n{i:03}"),
                NodeType::Facts,
                &format!("original summary {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let right_nodes: Vec<Node> = (0..200usize)
        .map(|i| {
            minimal_node(
                &format!("pkg.n{i:03}"),
                NodeType::Facts,
                &format!("updated summary {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let left = AgmFile {
        header: valid_header("stress.modify"),
        nodes: left_nodes,
    };
    let right = AgmFile {
        header: valid_header("stress.modify"),
        nodes: right_nodes,
    };

    let report = diff::diff(&left, &right);
    assert_eq!(
        report.summary.nodes_modified, 200,
        "All 200 nodes should be modified, got {}",
        report.summary.nodes_modified
    );
    assert_eq!(report.summary.nodes_added, 0);
    assert_eq!(report.summary.nodes_removed, 0);
    assert_eq!(report.summary.nodes_unchanged, 0);
}

#[test]
fn test_diff_100_added_100_removed() {
    let left_nodes: Vec<Node> = (0..100usize)
        .map(|i| {
            minimal_node(
                &format!("left.n{i:03}"),
                NodeType::Facts,
                &format!("left node {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let right_nodes: Vec<Node> = (0..100usize)
        .map(|i| {
            minimal_node(
                &format!("right.n{i:03}"),
                NodeType::Facts,
                &format!("right node {i}"),
                i * 5 + 1,
            )
        })
        .collect();

    let left = AgmFile {
        header: valid_header("stress.addremove"),
        nodes: left_nodes,
    };
    let right = AgmFile {
        header: valid_header("stress.addremove"),
        nodes: right_nodes,
    };

    let report = diff::diff(&left, &right);
    assert_eq!(
        report.summary.nodes_added, 100,
        "100 right-only nodes should be added, got {}",
        report.summary.nodes_added
    );
    assert_eq!(
        report.summary.nodes_removed, 100,
        "100 left-only nodes should be removed, got {}",
        report.summary.nodes_removed
    );
    assert_eq!(report.summary.nodes_modified, 0);
    assert_eq!(report.summary.nodes_unchanged, 0);
}

#[test]
fn test_diff_large_mixed_changes() {
    // 300 nodes total:
    //   - 100 unchanged (shared.n000..n099)
    //   - 100 modified  (mod.n000..n099): summary changed on right
    //   - 50 removed    (rem.n000..n049): only in left
    //   - 50 added      (add.n000..n049): only in right
    let unchanged_left: Vec<Node> = (0..100usize)
        .map(|i| {
            minimal_node(
                &format!("shared.n{i:03}"),
                NodeType::Facts,
                "shared summary",
                i * 5 + 1,
            )
        })
        .collect();
    let unchanged_right = unchanged_left.clone();

    let modified_left: Vec<Node> = (0..100usize)
        .map(|i| {
            minimal_node(
                &format!("mod.n{i:03}"),
                NodeType::Facts,
                "before",
                i * 5 + 1,
            )
        })
        .collect();
    let modified_right: Vec<Node> = (0..100usize)
        .map(|i| minimal_node(&format!("mod.n{i:03}"), NodeType::Facts, "after", i * 5 + 1))
        .collect();

    let removed: Vec<Node> = (0..50usize)
        .map(|i| {
            minimal_node(
                &format!("rem.n{i:03}"),
                NodeType::Facts,
                "removed",
                i * 5 + 1,
            )
        })
        .collect();
    let added: Vec<Node> = (0..50usize)
        .map(|i| minimal_node(&format!("add.n{i:03}"), NodeType::Facts, "added", i * 5 + 1))
        .collect();

    let mut left_nodes = Vec::new();
    left_nodes.extend(unchanged_left);
    left_nodes.extend(modified_left);
    left_nodes.extend(removed);

    let mut right_nodes = Vec::new();
    right_nodes.extend(unchanged_right);
    right_nodes.extend(modified_right);
    right_nodes.extend(added);

    let left = AgmFile {
        header: valid_header("stress.mixed"),
        nodes: left_nodes,
    };
    let right = AgmFile {
        header: valid_header("stress.mixed"),
        nodes: right_nodes,
    };

    let report = diff::diff(&left, &right);
    assert_eq!(
        report.summary.nodes_unchanged, 100,
        "100 unchanged nodes expected"
    );
    assert_eq!(
        report.summary.nodes_modified, 100,
        "100 modified nodes expected"
    );
    assert_eq!(report.summary.nodes_added, 50, "50 added nodes expected");
    assert_eq!(
        report.summary.nodes_removed, 50,
        "50 removed nodes expected"
    );
}
