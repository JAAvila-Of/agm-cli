//! Memory entry validation (spec S28).
//!
//! Pass 3 (structural): validates memory entries on a node.

use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::node::Node;

/// Validates all memory entries on a node.
///
/// Rules: V022 (invalid memory key pattern), V025 (invalid topic pattern),
/// V023 (upsert without value, search without query).
///
/// Delegates to `memory::schema::validate_memory_entry` for each entry.
#[must_use]
pub fn validate_memory(node: &Node, file_name: &str) -> Vec<AgmError> {
    let entries = match &node.memory {
        Some(e) => e,
        None => return Vec::new(),
    };

    let mut errors = Vec::new();
    let line = node.span.start_line;
    let id = node.id.as_str();
    let loc = ErrorLocation::full(file_name, line, id);

    for entry in entries {
        errors.extend(crate::memory::schema::validate_memory_entry(
            entry,
            loc.clone(),
        ));
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::error::codes::ErrorCode;
    use crate::model::fields::{NodeType, Span};
    use crate::model::memory::{MemoryAction, MemoryEntry};
    use crate::model::node::Node;

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

    fn valid_entry() -> MemoryEntry {
        MemoryEntry {
            key: "repo.pattern".to_owned(),
            topic: "rust.repository".to_owned(),
            action: MemoryAction::Get,
            value: None,
            scope: None,
            ttl: None,
            query: None,
            max_results: None,
        }
    }

    #[test]
    fn test_validate_memory_none_returns_empty() {
        let node = minimal_node();
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_memory_valid_entry_returns_empty() {
        let mut node = minimal_node();
        node.memory = Some(vec![valid_entry()]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_memory_invalid_key_uppercase_returns_v022() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.key = "Repo.Pattern".to_owned();
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V022));
    }

    #[test]
    fn test_validate_memory_invalid_key_special_chars_returns_v022() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.key = "repo-pattern".to_owned(); // hyphens not allowed
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V022));
    }

    #[test]
    fn test_validate_memory_invalid_key_leading_dot_returns_v022() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.key = ".repo.pattern".to_owned();
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V022));
    }

    #[test]
    fn test_validate_memory_upsert_no_value_returns_v023() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.action = MemoryAction::Upsert;
        entry.value = None;
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V023));
    }

    #[test]
    fn test_validate_memory_upsert_with_value_returns_empty() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.action = MemoryAction::Upsert;
        entry.value = Some("the value".to_owned());
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V023));
    }

    #[test]
    fn test_validate_memory_search_no_query_returns_v023() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.action = MemoryAction::Search;
        entry.query = None;
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V023));
    }

    #[test]
    fn test_validate_memory_search_with_query_returns_empty() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.action = MemoryAction::Search;
        entry.query = Some("find auth patterns".to_owned());
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V023));
    }

    #[test]
    fn test_validate_memory_get_no_query_returns_empty() {
        let mut node = minimal_node();
        let entry = valid_entry(); // action is Get, no query needed
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_memory_invalid_topic_returns_v025() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.topic = "Rust.Models".to_owned();
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V025));
    }

    #[test]
    fn test_validate_memory_valid_underscore_key_returns_empty() {
        let mut node = minimal_node();
        let mut entry = valid_entry();
        entry.key = "repo.kanban_column.row_mapping_pattern".to_owned();
        node.memory = Some(vec![entry]);
        let errors = validate_memory(&node, "test.agm");
        assert!(errors.is_empty());
    }
}
