//! Reference resolution across relationship fields (spec S11).
//!
//! Pass 5 (cross-node): validates that all referenced node IDs exist locally
//! or are detectable as cross-package references.

use std::collections::HashSet;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::file::AgmFile;
use crate::model::node::Node;

/// Returns true if `ref_id` looks like a cross-package reference.
///
/// A cross-package reference starts with one of the imported package names
/// followed by a dot. For example, if the file imports `shared.security`,
/// then `shared.security.auth.rules` is a cross-package ref.
fn looks_like_cross_package_ref(ref_id: &str, file: &AgmFile) -> bool {
    let imports = match &file.header.imports {
        Some(imports) => imports,
        None => return false,
    };

    for import in imports {
        let prefix = format!("{}.", import.package);
        if ref_id.starts_with(&prefix) {
            return true;
        }
    }

    false
}

/// Checks all IDs in `refs` against `all_ids`, emitting V004 for each unresolved
/// local reference.
fn check_refs(
    refs: &Option<Vec<String>>,
    field_name: &str,
    node: &Node,
    file: &AgmFile,
    all_ids: &HashSet<String>,
    file_name: &str,
    errors: &mut Vec<AgmError>,
) {
    let ref_list = match refs {
        Some(list) => list,
        None => return,
    };

    for ref_id in ref_list {
        if all_ids.contains(ref_id.as_str()) {
            continue;
        }
        if looks_like_cross_package_ref(ref_id, file) {
            // Handled by imports.rs
            continue;
        }
        errors.push(AgmError::new(
            ErrorCode::V004,
            format!(
                "Unresolved reference `{ref_id}` in `{field_name}` of node `{}`",
                node.id
            ),
            ErrorLocation::full(file_name, node.span.start_line, &node.id),
        ));
    }
}

/// Validates all relationship-field references across the file's node set.
///
/// Checks `depends`, `related_to`, `replaces`, `conflicts`, and `see_also`.
///
/// Rule: V004 (unresolved reference).
#[must_use]
pub fn validate_references(
    file: &AgmFile,
    all_ids: &HashSet<String>,
    file_name: &str,
) -> Vec<AgmError> {
    let mut errors = Vec::new();

    for node in &file.nodes {
        check_refs(
            &node.depends,
            "depends",
            node,
            file,
            all_ids,
            file_name,
            &mut errors,
        );
        check_refs(
            &node.related_to,
            "related_to",
            node,
            file,
            all_ids,
            file_name,
            &mut errors,
        );
        check_refs(
            &node.replaces,
            "replaces",
            node,
            file,
            all_ids,
            file_name,
            &mut errors,
        );
        check_refs(
            &node.conflicts,
            "conflicts",
            node,
            file,
            all_ids,
            file_name,
            &mut errors,
        );
        check_refs(
            &node.see_also,
            "see_also",
            node,
            file,
            all_ids,
            file_name,
            &mut errors,
        );
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashSet};

    use super::*;
    use crate::model::fields::{NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::imports::ImportEntry;
    use crate::model::node::Node;

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

    fn make_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: "a node".to_owned(),
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
            span: Span::new(5, 7),
        }
    }

    fn ids(nodes: &[Node]) -> HashSet<String> {
        nodes.iter().map(|n| n.id.clone()).collect()
    }

    #[test]
    fn test_validate_references_all_resolved_returns_empty() {
        let mut node_a = make_node("auth.login");
        let node_b = make_node("auth.rules");
        node_a.depends = Some(vec!["auth.rules".to_owned()]);

        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node_a, node_b.clone()],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_references_unresolved_depends_returns_v004() {
        let mut node = make_node("auth.login");
        node.depends = Some(vec!["missing.dep".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V004 && e.message.contains("depends"))
        );
    }

    #[test]
    fn test_validate_references_unresolved_related_to_returns_v004() {
        let mut node = make_node("auth.login");
        node.related_to = Some(vec!["missing.node".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V004 && e.message.contains("related_to"))
        );
    }

    #[test]
    fn test_validate_references_unresolved_replaces_returns_v004() {
        let mut node = make_node("auth.login");
        node.replaces = Some(vec!["old.node".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V004 && e.message.contains("replaces"))
        );
    }

    #[test]
    fn test_validate_references_unresolved_conflicts_returns_v004() {
        let mut node = make_node("auth.login");
        node.conflicts = Some(vec!["conflict.node".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V004 && e.message.contains("conflicts"))
        );
    }

    #[test]
    fn test_validate_references_unresolved_see_also_returns_v004() {
        let mut node = make_node("auth.login");
        node.see_also = Some(vec!["related.doc".to_owned()]);
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V004 && e.message.contains("see_also"))
        );
    }

    #[test]
    fn test_validate_references_cross_package_ref_skipped() {
        let mut header = minimal_header();
        header.imports = Some(vec![ImportEntry {
            package: "shared.security".to_owned(),
            version_constraint: None,
        }]);

        let mut node = make_node("auth.login");
        // This ref starts with "shared.security." — should be treated as cross-package
        node.depends = Some(vec!["shared.security.auth.rules".to_owned()]);

        let file = AgmFile {
            header,
            nodes: vec![node],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        // No V004 should fire because the ref is cross-package
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V004));
    }

    #[test]
    fn test_validate_references_empty_ref_list_returns_empty() {
        let mut node = make_node("auth.login");
        node.depends = Some(vec![]); // empty list
        let file = AgmFile {
            header: minimal_header(),
            nodes: vec![node],
        };
        let all_ids = ids(&file.nodes);
        let errors = validate_references(&file, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }
}
