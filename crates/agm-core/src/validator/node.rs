//! Per-node structural validation (spec S11, S12).
//!
//! Pass 2: validates node IDs, summary, dates, status constraints.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation, Severity};
use crate::model::fields::{NODE_ID_PATTERN, NodeStatus};
use crate::model::node::Node;

/// Compiled regex for validating node IDs. Pattern sourced from
/// [`NODE_ID_PATTERN`] in `crate::model::fields` -- shared with the parser
/// to prevent drift.
///
/// **Defence-in-depth**: V021 is reachable even when the parser's P002 check
/// has already run, because programmatically constructed nodes (e.g. via
/// `build_unchecked`, `serde_json::from_str`, or direct `Node { id, ... }`
/// construction) bypass the parser entirely. V021 is the validator-layer
/// guard for those code paths.
static NODE_ID_RE: OnceLock<Regex> = OnceLock::new();

fn node_id_regex() -> &'static Regex {
    NODE_ID_RE.get_or_init(|| Regex::new(NODE_ID_PATTERN).unwrap())
}

/// Validates all node IDs for uniqueness (V003) and pattern compliance (V021).
///
/// Returns errors for duplicate IDs and invalid ID patterns.
#[must_use]
pub fn validate_node_ids(nodes: &[Node], file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let pattern = node_id_regex();

    for node in nodes {
        let id = node.id.as_str();
        let line = node.span.start_line;

        // V003 — uniqueness
        if let Some(&first_line) = seen.get(id) {
            errors.push(AgmError::new(
                ErrorCode::V003,
                format!("Duplicate node ID: `{id}` (first seen at line {first_line})"),
                ErrorLocation::full(file_name, line, id),
            ));
        } else {
            seen.insert(id, line);
        }

        // V021 — ID pattern
        if !pattern.is_match(id) {
            errors.push(AgmError::new(
                ErrorCode::V021,
                format!("Node ID does not match required pattern: `{id}`"),
                ErrorLocation::full(file_name, line, id),
            ));
        }
    }

    errors
}

/// Validates a single node's structural fields.
///
/// Rules: V002 (missing summary), V007 (valid_from > valid_until),
/// V011 (empty summary warning), V012 (summary too long warning),
/// V014 (deprecated/superseded without replaces).
#[must_use]
pub fn validate_node(node: &Node, all_ids: &HashSet<String>, file_name: &str) -> Vec<AgmError> {
    let _ = all_ids; // used by callers building sets; kept in signature for consistency
    let mut errors = Vec::new();
    let line = node.span.start_line;
    let id = node.id.as_str();

    // V002 — summary required (missing entirely, not just empty)
    // In the model, summary is String (never None). We check for empty here
    // as defensive validation; parser may produce empty string.
    if node.summary.is_empty() {
        errors.push(AgmError::new(
            ErrorCode::V002,
            format!("Node `{id}` missing required field: `summary`"),
            ErrorLocation::full(file_name, line, id),
        ));
        // If truly empty, skip the whitespace/length checks
        return errors;
    }

    // V011 — summary is whitespace-only (warning)
    if node.summary.trim().is_empty() {
        errors.push(AgmError::with_severity(
            ErrorCode::V011,
            Severity::Warning,
            format!("Summary is empty in node `{id}`"),
            ErrorLocation::full(file_name, line, id),
        ));
    }

    // V012 — summary exceeds 200 characters (warning, measured in chars)
    if node.summary.chars().count() > 200 {
        errors.push(AgmError::with_severity(
            ErrorCode::V012,
            Severity::Warning,
            format!("Summary exceeds 200 characters in node `{id}`"),
            ErrorLocation::full(file_name, line, id),
        ));
    }

    // V007 — valid_from must not be after valid_until
    if let (Some(from), Some(until)) = (&node.valid_from, &node.valid_until) {
        // Compare as strings: ISO 8601 dates and datetimes sort lexicographically
        // when zero-padded (which is the standard). This handles both date-only
        // and datetime formats without pulling in a date library.
        if from.as_str() > until.as_str() {
            errors.push(AgmError::new(
                ErrorCode::V007,
                format!("`valid_from` is after `valid_until` in node `{id}`"),
                ErrorLocation::full(file_name, line, id),
            ));
        }
    }

    // V014 — deprecated or superseded node missing `replaces` (warning)
    let needs_replaces = matches!(
        &node.status,
        Some(NodeStatus::Deprecated) | Some(NodeStatus::Superseded)
    );
    if needs_replaces && node.replaces.is_none() {
        errors.push(AgmError::with_severity(
            ErrorCode::V014,
            Severity::Warning,
            format!("Deprecated node `{id}` missing `replaces` or `superseded_by`"),
            ErrorLocation::full(file_name, line, id),
        ));
    }

    errors
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::model::fields::{NodeStatus, NodeType, Span};
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
    fn test_validate_node_ids_duplicate_returns_v003() {
        let mut n1 = minimal_node();
        let mut n2 = minimal_node();
        n1.id = "auth.login".to_owned();
        n2.id = "auth.login".to_owned();
        n2.span = Span::new(10, 12);
        let errors = validate_node_ids(&[n1, n2], "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V003));
    }

    #[test]
    fn test_validate_node_id_valid_pattern_returns_empty() {
        let mut node = minimal_node();
        node.id = "auth.login".to_owned();
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_node_id_single_segment_valid() {
        let mut node = minimal_node();
        node.id = "auth".to_owned();
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_node_id_invalid_pattern_uppercase_returns_v021() {
        let mut node = minimal_node();
        node.id = "Auth.Login".to_owned();
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V021));
    }

    #[test]
    fn test_validate_node_id_leading_dot_returns_v021() {
        let mut node = minimal_node();
        node.id = ".auth.login".to_owned();
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V021));
    }

    #[test]
    fn test_validate_node_id_trailing_dot_returns_v021() {
        let mut node = minimal_node();
        node.id = "auth.login.".to_owned();
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V021));
    }

    #[test]
    fn test_validate_node_id_consecutive_dots_returns_v021() {
        let mut node = minimal_node();
        node.id = "auth..login".to_owned();
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V021));
    }

    #[test]
    fn test_validate_node_id_starts_with_digit_returns_v021() {
        let mut node = minimal_node();
        node.id = "1auth".to_owned();
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V021));
    }

    #[test]
    fn test_validate_node_id_digit_only_segment_returns_v021() {
        // Defence-in-depth: V021 fires when a node with an invalid ID is
        // constructed directly (bypassing the parser). The parser would have
        // emitted P002 first, but programmatic construction (build_unchecked,
        // serde deserialization, direct struct literal) skips the parser.
        let node = Node {
            id: "perf.000".to_owned(), // segment "000" starts with a digit
            node_type: NodeType::Facts,
            summary: "direct construction test".to_owned(),
            span: Span::new(1, 2),
            ..Default::default()
        };
        let errors = validate_node_ids(&[node], "test.agm");
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::V021),
            "digit-only segment must trigger V021 via direct struct construction"
        );
    }

    #[test]
    fn test_validate_node_empty_summary_returns_v002() {
        let mut node = minimal_node();
        node.summary = String::new();
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V002));
    }

    #[test]
    fn test_validate_node_whitespace_only_summary_returns_v011() {
        let mut node = minimal_node();
        node.summary = "   ".to_owned();
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V011));
    }

    #[test]
    fn test_validate_node_summary_exactly_200_returns_empty() {
        let mut node = minimal_node();
        node.summary = "a".repeat(200);
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.is_empty(), "200 chars should not trigger V012");
    }

    #[test]
    fn test_validate_node_summary_201_chars_returns_v012() {
        let mut node = minimal_node();
        node.summary = "a".repeat(201);
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V012));
    }

    #[test]
    fn test_validate_node_valid_from_after_until_returns_v007() {
        let mut node = minimal_node();
        node.valid_from = Some("2025-12-31".to_owned());
        node.valid_until = Some("2025-01-01".to_owned());
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V007));
    }

    #[test]
    fn test_validate_node_valid_from_equals_until_returns_empty() {
        let mut node = minimal_node();
        node.valid_from = Some("2025-06-01".to_owned());
        node.valid_until = Some("2025-06-01".to_owned());
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_node_valid_from_before_until_returns_empty() {
        let mut node = minimal_node();
        node.valid_from = Some("2025-01-01".to_owned());
        node.valid_until = Some("2025-12-31".to_owned());
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_node_deprecated_no_replaces_returns_v014() {
        let mut node = minimal_node();
        node.status = Some(NodeStatus::Deprecated);
        node.replaces = None;
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V014));
    }

    #[test]
    fn test_validate_node_superseded_no_replaces_returns_v014() {
        let mut node = minimal_node();
        node.status = Some(NodeStatus::Superseded);
        node.replaces = None;
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V014));
    }

    #[test]
    fn test_validate_node_superseded_with_replaces_returns_empty() {
        let mut node = minimal_node();
        node.status = Some(NodeStatus::Superseded);
        node.replaces = Some(vec!["other.node".to_owned()]);
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V014));
    }

    #[test]
    fn test_validate_node_valid_node_returns_empty() {
        let node = minimal_node();
        let all_ids = HashSet::new();
        let errors = validate_node(&node, &all_ids, "test.agm");
        assert!(errors.is_empty());
    }
}
