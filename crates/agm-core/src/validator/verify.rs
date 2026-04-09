//! Verify entry validation (spec S24).
//!
//! Pass 3 (structural): validates verify check entries on a node.

use std::collections::HashSet;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::node::Node;
use crate::model::verify::VerifyCheck;

/// Validates all verify checks on a node.
///
/// Rules: V009 (missing required field in verify entry, unresolved node ref).
#[must_use]
pub fn validate_verify(node: &Node, all_ids: &HashSet<String>, file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();
    let checks = match &node.verify {
        Some(v) => v,
        None => return errors,
    };

    let line = node.span.start_line;
    let id = node.id.as_str();
    let loc = ErrorLocation::full(file_name, line, id);

    for check in checks {
        match check {
            VerifyCheck::Command { run, .. } => {
                if run.trim().is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `run` (empty command)",
                        loc.clone(),
                    ));
                }
            }

            VerifyCheck::FileExists { file } => {
                if file.trim().is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `file` (empty path)",
                        loc.clone(),
                    ));
                }
            }

            VerifyCheck::FileContains { file, pattern } => {
                if file.trim().is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry `file_contains` missing required field: `file`",
                        loc.clone(),
                    ));
                }
                if pattern.trim().is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry `file_contains` missing required field: `pattern`",
                        loc.clone(),
                    ));
                }
            }

            VerifyCheck::FileNotContains { file, pattern } => {
                if file.trim().is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry `file_not_contains` missing required field: `file`",
                        loc.clone(),
                    ));
                }
                if pattern.trim().is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry `file_not_contains` missing required field: `pattern`",
                        loc.clone(),
                    ));
                }
            }

            VerifyCheck::NodeStatus { node: ref_node, .. } => {
                if ref_node.trim().is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry `node_status` missing required field: `node`",
                        loc.clone(),
                    ));
                } else if !all_ids.contains(ref_node.as_str()) {
                    // Cross-package refs are handled by imports.rs; only flag truly unresolved
                    // local refs (those that don't look like cross-package refs).
                    // Simple heuristic: if the ref contains a dot segment matching an
                    // imported package, skip. Here we flag all unresolved local refs.
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        format!(
                            "Verify entry `node_status` references non-existent node: `{ref_node}`"
                        ),
                        loc.clone(),
                    ));
                }
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::HashSet;

    use super::*;
    use crate::model::fields::{NodeType, Span};
    use crate::model::node::Node;
    use crate::model::verify::VerifyCheck;

    fn minimal_node() -> Node {
        Node {
            id: "test.node".to_owned(),
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
            span: Span::new(5, 7),
        }
    }

    #[test]
    fn test_validate_verify_none_returns_empty() {
        let node = minimal_node();
        let all_ids = HashSet::new();
        let errors = validate_verify(&node, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_verify_valid_command_returns_empty() {
        let mut node = minimal_node();
        node.verify = Some(vec![VerifyCheck::Command {
            run: "cargo test".to_owned(),
            expect: None,
        }]);
        let all_ids = HashSet::new();
        let errors = validate_verify(&node, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_verify_command_empty_run_returns_v009() {
        let mut node = minimal_node();
        node.verify = Some(vec![VerifyCheck::Command {
            run: "   ".to_owned(),
            expect: None,
        }]);
        let all_ids = HashSet::new();
        let errors = validate_verify(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
    }

    #[test]
    fn test_validate_verify_file_contains_empty_file_returns_v009() {
        let mut node = minimal_node();
        node.verify = Some(vec![VerifyCheck::FileContains {
            file: String::new(),
            pattern: "fn main".to_owned(),
        }]);
        let all_ids = HashSet::new();
        let errors = validate_verify(&node, &all_ids, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V009 && e.message.contains("`file`"))
        );
    }

    #[test]
    fn test_validate_verify_file_contains_empty_pattern_returns_v009() {
        let mut node = minimal_node();
        node.verify = Some(vec![VerifyCheck::FileContains {
            file: "src/main.rs".to_owned(),
            pattern: String::new(),
        }]);
        let all_ids = HashSet::new();
        let errors = validate_verify(&node, &all_ids, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V009 && e.message.contains("`pattern`"))
        );
    }

    #[test]
    fn test_validate_verify_node_status_unresolved_returns_v009() {
        let mut node = minimal_node();
        node.verify = Some(vec![VerifyCheck::NodeStatus {
            node: "missing.node".to_owned(),
            status: "completed".to_owned(),
        }]);
        let all_ids = HashSet::new();
        let errors = validate_verify(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
    }

    #[test]
    fn test_validate_verify_node_status_resolved_returns_empty() {
        let mut node = minimal_node();
        node.verify = Some(vec![VerifyCheck::NodeStatus {
            node: "auth.login".to_owned(),
            status: "completed".to_owned(),
        }]);
        let mut all_ids = HashSet::new();
        all_ids.insert("auth.login".to_owned());
        let errors = validate_verify(&node, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }
}
