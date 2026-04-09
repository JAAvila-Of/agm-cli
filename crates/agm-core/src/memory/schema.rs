//! Memory validation functions (spec S28).
//!
//! Provides standalone validation functions for memory keys, topics, entries,
//! and `load_memory` references. These functions are reusable by the validator,
//! CLI tools, and the Phase 2 runtime.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::memory::{MemoryAction, MemoryEntry};

/// Regex for memory key: starts with lowercase letter, then lowercase
/// letters, digits, underscores, or dots. Spec S28.3.
static MEMORY_KEY_RE: OnceLock<Regex> = OnceLock::new();

/// Regex for memory topic: dot-delimited segments of lowercase letters,
/// digits, and underscores. Each segment must start with a letter.
static MEMORY_TOPIC_RE: OnceLock<Regex> = OnceLock::new();

fn key_regex() -> &'static Regex {
    MEMORY_KEY_RE.get_or_init(|| Regex::new(r"^[a-z][a-z0-9_.]*$").unwrap())
}

fn topic_regex() -> &'static Regex {
    MEMORY_TOPIC_RE.get_or_init(|| Regex::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$").unwrap())
}

/// Validates a memory key against the spec S28.3 pattern.
///
/// Returns `Ok(())` if valid, or `Err(AgmError)` with code V022.
pub fn validate_memory_key(key: &str, location: ErrorLocation) -> Result<(), AgmError> {
    if key_regex().is_match(key) {
        Ok(())
    } else {
        Err(AgmError::new(
            ErrorCode::V022,
            format!("Memory key does not match required pattern: `{key}`"),
            location,
        ))
    }
}

/// Validates a memory topic string.
///
/// Topics must be dot-delimited, lowercase, each segment starting with a
/// letter. Returns `Ok(())` if valid, or `Err(AgmError)` with code V025.
pub fn validate_memory_topic(topic: &str, location: ErrorLocation) -> Result<(), AgmError> {
    if topic_regex().is_match(topic) {
        Ok(())
    } else {
        Err(AgmError::new(
            ErrorCode::V025,
            format!("Memory topic does not match required pattern: `{topic}`"),
            location,
        ))
    }
}

/// Validates a complete memory entry: key pattern (V022), topic pattern
/// (V025), and action constraints (V023: upsert requires value, search
/// requires query).
#[must_use]
pub fn validate_memory_entry(entry: &MemoryEntry, location: ErrorLocation) -> Vec<AgmError> {
    let mut errors = Vec::new();

    if let Err(e) = validate_memory_key(entry.key.as_str(), location.clone()) {
        errors.push(e);
    }

    if let Err(e) = validate_memory_topic(entry.topic.as_str(), location.clone()) {
        errors.push(e);
    }

    if entry.action == MemoryAction::Upsert && entry.value.is_none() {
        errors.push(AgmError::new(
            ErrorCode::V023,
            format!(
                "Invalid memory action: `upsert` on key `{}` requires `value`",
                entry.key
            ),
            location.clone(),
        ));
    }

    if entry.action == MemoryAction::Search && entry.query.is_none() {
        errors.push(AgmError::new(
            ErrorCode::V023,
            format!(
                "Invalid memory action: `search` on key `{}` requires `query`",
                entry.key
            ),
            location,
        ));
    }

    errors
}

/// Validates `load_memory` topic references from `agent_context`.
///
/// Each topic in `topics` is checked against `available_topics` (the set of
/// all topics declared in `memory:` entries across the file). Unresolved
/// topics produce V026 warnings.
#[must_use]
pub fn validate_load_memory(
    topics: &[String],
    available_topics: &HashSet<String>,
    location: ErrorLocation,
) -> Vec<AgmError> {
    let mut errors = Vec::new();

    for topic in topics {
        if !topic_regex().is_match(topic.as_str()) {
            errors.push(AgmError::new(
                ErrorCode::V025,
                format!("Memory topic does not match required pattern: `{topic}`"),
                location.clone(),
            ));
        } else if !available_topics.contains(topic.as_str()) {
            errors.push(AgmError::new(
                ErrorCode::V026,
                format!(
                    "Unresolved memory topic `{topic}` in `agent_context.load_memory` of node `{}`",
                    location.node.as_deref().unwrap_or("<unknown>")
                ),
                location.clone(),
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc() -> ErrorLocation {
        ErrorLocation::full("test.agm", 5, "test.node")
    }

    fn get_entry(key: &str, topic: &str) -> MemoryEntry {
        MemoryEntry {
            key: key.to_owned(),
            topic: topic.to_owned(),
            action: MemoryAction::Get,
            value: None,
            scope: None,
            ttl: None,
            query: None,
            max_results: None,
        }
    }

    // --- validate_memory_key ---

    #[test]
    fn test_validate_memory_key_valid_dotted_returns_ok() {
        assert!(validate_memory_key("repo.pattern", loc()).is_ok());
    }

    #[test]
    fn test_validate_memory_key_valid_with_underscores_returns_ok() {
        assert!(validate_memory_key("repo.kanban_column.row_mapping_pattern", loc()).is_ok());
    }

    #[test]
    fn test_validate_memory_key_valid_single_segment_returns_ok() {
        assert!(validate_memory_key("mykey", loc()).is_ok());
    }

    #[test]
    fn test_validate_memory_key_invalid_uppercase_returns_v022() {
        let err = validate_memory_key("Invalid-Key", loc()).unwrap_err();
        assert_eq!(err.code, ErrorCode::V022);
    }

    #[test]
    fn test_validate_memory_key_invalid_starts_digit_returns_v022() {
        let err = validate_memory_key("1abc", loc()).unwrap_err();
        assert_eq!(err.code, ErrorCode::V022);
    }

    #[test]
    fn test_validate_memory_key_invalid_hyphen_returns_v022() {
        let err = validate_memory_key("repo-pattern", loc()).unwrap_err();
        assert_eq!(err.code, ErrorCode::V022);
    }

    #[test]
    fn test_validate_memory_key_empty_returns_v022() {
        let err = validate_memory_key("", loc()).unwrap_err();
        assert_eq!(err.code, ErrorCode::V022);
    }

    // --- validate_memory_topic ---

    #[test]
    fn test_validate_memory_topic_valid_dotted_returns_ok() {
        assert!(validate_memory_topic("rust.repository", loc()).is_ok());
    }

    #[test]
    fn test_validate_memory_topic_valid_single_returns_ok() {
        assert!(validate_memory_topic("infrastructure", loc()).is_ok());
    }

    #[test]
    fn test_validate_memory_topic_invalid_uppercase_returns_v025() {
        let err = validate_memory_topic("Rust.models", loc()).unwrap_err();
        assert_eq!(err.code, ErrorCode::V025);
    }

    #[test]
    fn test_validate_memory_topic_invalid_leading_dot_returns_v025() {
        let err = validate_memory_topic(".rust", loc()).unwrap_err();
        assert_eq!(err.code, ErrorCode::V025);
    }

    #[test]
    fn test_validate_memory_topic_empty_returns_v025() {
        let err = validate_memory_topic("", loc()).unwrap_err();
        assert_eq!(err.code, ErrorCode::V025);
    }

    // --- validate_memory_entry ---

    #[test]
    fn test_validate_memory_entry_valid_get_returns_empty() {
        let entry = get_entry("repo.pattern", "rust.repository");
        assert!(validate_memory_entry(&entry, loc()).is_empty());
    }

    #[test]
    fn test_validate_memory_entry_valid_upsert_returns_empty() {
        let mut entry = get_entry("repo.pattern", "rust.repository");
        entry.action = MemoryAction::Upsert;
        entry.value = Some("some value".to_owned());
        assert!(validate_memory_entry(&entry, loc()).is_empty());
    }

    #[test]
    fn test_validate_memory_entry_upsert_no_value_returns_v023() {
        let mut entry = get_entry("repo.pattern", "rust.repository");
        entry.action = MemoryAction::Upsert;
        let errors = validate_memory_entry(&entry, loc());
        assert!(errors.iter().any(|e| e.code == ErrorCode::V023));
    }

    #[test]
    fn test_validate_memory_entry_search_no_query_returns_v023() {
        let mut entry = get_entry("repo.pattern", "rust.repository");
        entry.action = MemoryAction::Search;
        let errors = validate_memory_entry(&entry, loc());
        assert!(errors.iter().any(|e| e.code == ErrorCode::V023));
    }

    #[test]
    fn test_validate_memory_entry_bad_key_and_bad_topic_returns_both() {
        let entry = get_entry("Bad-Key", "Bad.Topic");
        let errors = validate_memory_entry(&entry, loc());
        assert!(errors.iter().any(|e| e.code == ErrorCode::V022));
        assert!(errors.iter().any(|e| e.code == ErrorCode::V025));
    }

    // --- validate_load_memory ---

    #[test]
    fn test_validate_load_memory_all_resolved_returns_empty() {
        let topics = vec!["rust.repository".to_owned()];
        let mut available = HashSet::new();
        available.insert("rust.repository".to_owned());
        assert!(validate_load_memory(&topics, &available, loc()).is_empty());
    }

    #[test]
    fn test_validate_load_memory_unresolved_returns_v026() {
        let topics = vec!["rust.repository".to_owned()];
        let available = HashSet::new();
        let errors = validate_load_memory(&topics, &available, loc());
        assert!(errors.iter().any(|e| e.code == ErrorCode::V026));
    }

    #[test]
    fn test_validate_load_memory_invalid_format_returns_v025() {
        let topics = vec!["Rust.Models".to_owned()];
        let available = HashSet::new();
        let errors = validate_load_memory(&topics, &available, loc());
        assert!(errors.iter().any(|e| e.code == ErrorCode::V025));
    }

    #[test]
    fn test_validate_load_memory_empty_list_returns_empty() {
        let topics: Vec<String> = vec![];
        let available = HashSet::new();
        assert!(validate_load_memory(&topics, &available, loc()).is_empty());
    }
}
