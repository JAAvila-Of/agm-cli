//! Agent context validation (spec S25).
//!
//! Pass 3 (structural): validates agent_context field on a node.

use std::collections::HashSet;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation, Severity};
use crate::model::node::Node;

/// Returns true if the path is unsafe (absolute or contains traversal).
fn is_unsafe_path(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || path.contains("..")
}

/// Validates the `agent_context` field on a node.
///
/// Rules: V004 (unresolved load_nodes reference), V015 (unsafe load_files
/// path), V025/V026 (invalid or unresolved load_memory topics).
///
/// When `single_node` is `true`, the V004 check for `load_nodes` references
/// is skipped — referenced nodes may exist in sibling files not present in
/// the scratch `AgmFile` created by the builder.
#[must_use]
pub fn validate_context(
    node: &Node,
    all_ids: &HashSet<String>,
    all_memory_topics: &HashSet<String>,
    file_name: &str,
    single_node: bool,
) -> Vec<AgmError> {
    let ctx = match &node.agent_context {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut errors = Vec::new();
    let line = node.span.start_line;
    let id = node.id.as_str();

    // V004 — load_nodes must reference existing node IDs.
    // Skipped in SingleNode scope: referenced nodes may live in sibling files.
    if !single_node {
        if let Some(ref load_nodes) = ctx.load_nodes {
            for ref_id in load_nodes {
                if !all_ids.contains(ref_id.as_str()) {
                    errors.push(AgmError::new(
                        ErrorCode::V004,
                        format!(
                            "Unresolved reference `{ref_id}` in `agent_context.load_nodes` of node `{id}`"
                        ),
                        ErrorLocation::full(file_name, line, id),
                    ));
                }
            }
        }
    }

    // V015 — load_files paths must be relative and traversal-free (warning)
    if let Some(ref load_files) = ctx.load_files {
        for load_file in load_files {
            if is_unsafe_path(&load_file.path) {
                errors.push(AgmError::with_severity(
                    ErrorCode::V015,
                    Severity::Warning,
                    format!(
                        "`target` path is absolute or contains traversal: `{}`",
                        load_file.path
                    ),
                    ErrorLocation::full(file_name, line, id),
                ));
            }
        }
    }

    // V025/V026 — load_memory topics must be valid and resolvable
    if let Some(ref load_memory) = ctx.load_memory {
        errors.extend(crate::memory::schema::validate_load_memory(
            load_memory,
            all_memory_topics,
            ErrorLocation::full(file_name, line, id),
        ));
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::model::context::{AgentContext, FileRange, LoadFile};
    use crate::model::fields::{NodeType, Span};
    use crate::model::node::Node;

    fn minimal_node() -> Node {
        Node {
            id: "test.node".to_owned(),
            node_type: NodeType::Facts,
            summary: "a test node".to_owned(),
            span: Span::new(5, 7),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_context_none_returns_empty() {
        let node = minimal_node();
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_context_load_nodes_valid_returns_empty() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: Some(vec!["auth.login".to_owned()]),
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: None,
        });
        let mut all_ids = HashSet::new();
        all_ids.insert("auth.login".to_owned());
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_context_load_nodes_unresolved_returns_v004() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: Some(vec!["missing.node".to_owned()]),
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: None,
        });
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V004));
    }

    #[test]
    fn test_validate_context_load_files_relative_path_returns_empty() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: None,
            load_files: Some(vec![LoadFile {
                path: "src/auth.rs".to_owned(),
                range: FileRange::Full,
            }]),
            system_hint: None,
            max_tokens: None,
            load_memory: None,
        });
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_context_load_files_absolute_returns_v015() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: None,
            load_files: Some(vec![LoadFile {
                path: "/etc/passwd".to_owned(),
                range: FileRange::Full,
            }]),
            system_hint: None,
            max_tokens: None,
            load_memory: None,
        });
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V015));
    }

    #[test]
    fn test_validate_context_load_files_traversal_returns_v015() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: None,
            load_files: Some(vec![LoadFile {
                path: "src/../../../etc/shadow".to_owned(),
                range: FileRange::Full,
            }]),
            system_hint: None,
            max_tokens: None,
            load_memory: None,
        });
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V015));
    }

    #[test]
    fn test_validate_context_load_memory_none_returns_empty() {
        let node = minimal_node();
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_context_load_memory_valid_returns_empty() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: None,
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: Some(vec!["rust.repository".to_owned()]),
        });
        let all_ids = HashSet::new();
        let mut all_memory_topics = HashSet::new();
        all_memory_topics.insert("rust.repository".to_owned());
        let errors = validate_context(&node, &all_ids, &all_memory_topics, "test.agm", false);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_context_load_memory_unresolved_returns_v026() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: None,
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: Some(vec!["rust.repository".to_owned()]),
        });
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V026));
    }

    #[test]
    fn test_validate_context_load_memory_invalid_format_returns_v025() {
        let mut node = minimal_node();
        node.agent_context = Some(AgentContext {
            load_nodes: None,
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: Some(vec!["Rust.Models".to_owned()]),
        });
        let all_ids = HashSet::new();
        let errors = validate_context(&node, &all_ids, &HashSet::new(), "test.agm", false);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V025));
    }
}
