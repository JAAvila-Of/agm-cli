//! Execution state validation (spec S26).
//!
//! Pass 3 (structural): validates execution_status and associated metadata fields.

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation, Severity};
use crate::model::execution::ExecutionStatus;
use crate::model::node::Node;

/// Validates execution-related fields on a node.
///
/// Rules:
/// - V010 (warning): `completed` without `executed_by` or `executed_at`
/// - V006 (warning): `executed_at` with unparseable timestamp
/// - V020 (stub): runtime-only transition validation — no-op at static validation time
#[must_use]
pub fn validate_execution(node: &Node, file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();
    let line = node.span.start_line;
    let id = node.id.as_str();

    // V006 — validate executed_at timestamp format (warning)
    if let Some(ref ts) = node.executed_at {
        if !looks_like_iso8601(ts) {
            errors.push(AgmError::with_severity(
                ErrorCode::V006,
                Severity::Warning,
                format!("Invalid `execution_status` value: `executed_at` timestamp `{ts}` is not a valid ISO 8601 date/datetime"),
                ErrorLocation::full(file_name, line, id),
            ));
        }
    }

    // V010 — completed node should have execution metadata (warning)
    if node.execution_status == Some(ExecutionStatus::Completed) {
        if node.executed_by.is_none() {
            errors.push(AgmError::with_severity(
                ErrorCode::V010,
                Severity::Warning,
                format!(
                    "Node type `{id}` typically includes field `executed_by` (missing) for completed nodes"
                ),
                ErrorLocation::full(file_name, line, id),
            ));
        }
        if node.executed_at.is_none() {
            errors.push(AgmError::with_severity(
                ErrorCode::V010,
                Severity::Warning,
                format!(
                    "Node type `{id}` typically includes field `executed_at` (missing) for completed nodes"
                ),
                ErrorLocation::full(file_name, line, id),
            ));
        }
    }

    // V020 — execution status transition validation
    // This rule requires before/after state comparison and is only applicable
    // at runtime (not during static file validation). Left as a no-op stub.
    // The runtime phase (Phase 2) will implement full transition validation.

    errors
}

/// Checks whether a string resembles an ISO 8601 date or datetime.
///
/// Accepts `YYYY-MM-DD` (date-only) and `YYYY-MM-DDTHH:MM:SS` with optional
/// timezone or fractional seconds. This is a lightweight heuristic check that
/// avoids pulling in a date library.
fn looks_like_iso8601(s: &str) -> bool {
    let s = s.trim();

    // Must start with a 4-digit year
    if s.len() < 10 {
        return false;
    }

    let bytes = s.as_bytes();

    // YYYY-MM-DD
    let year_ok = bytes[0..4].iter().all(|b| b.is_ascii_digit());
    let dash1 = bytes[4] == b'-';
    let month_ok = bytes[5..7].iter().all(|b| b.is_ascii_digit());
    let dash2 = bytes[7] == b'-';
    let day_ok = bytes[8..10].iter().all(|b| b.is_ascii_digit());

    if !(year_ok && dash1 && month_ok && dash2 && day_ok) {
        return false;
    }

    // Date-only is acceptable
    if s.len() == 10 {
        return true;
    }

    // T separator for datetime
    bytes[10] == b'T' || bytes[10] == b' '
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::model::execution::ExecutionStatus;
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
    fn test_validate_execution_no_status_returns_empty() {
        let node = minimal_node();
        let errors = validate_execution(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_execution_completed_no_executed_by_returns_v010() {
        let mut node = minimal_node();
        node.execution_status = Some(ExecutionStatus::Completed);
        node.executed_at = Some("2025-01-01".to_owned());
        // no executed_by
        let errors = validate_execution(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V010 && e.message.contains("executed_by"))
        );
    }

    #[test]
    fn test_validate_execution_completed_no_executed_at_returns_v010() {
        let mut node = minimal_node();
        node.execution_status = Some(ExecutionStatus::Completed);
        node.executed_by = Some("agent-01".to_owned());
        // no executed_at
        let errors = validate_execution(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V010 && e.message.contains("executed_at"))
        );
    }

    #[test]
    fn test_validate_execution_completed_with_metadata_returns_empty() {
        let mut node = minimal_node();
        node.execution_status = Some(ExecutionStatus::Completed);
        node.executed_by = Some("agent-01".to_owned());
        node.executed_at = Some("2025-06-15T14:30:00Z".to_owned());
        let errors = validate_execution(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_execution_invalid_timestamp_returns_v006() {
        let mut node = minimal_node();
        node.executed_at = Some("not-a-date".to_owned());
        let errors = validate_execution(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V006));
    }

    #[test]
    fn test_validate_execution_date_only_timestamp_valid() {
        let mut node = minimal_node();
        node.executed_at = Some("2025-06-15".to_owned());
        let errors = validate_execution(&node, "test.agm");
        assert!(!errors.iter().any(|e| e.code == ErrorCode::V006));
    }

    #[test]
    fn test_validate_execution_pending_status_no_warnings() {
        let mut node = minimal_node();
        node.execution_status = Some(ExecutionStatus::Pending);
        let errors = validate_execution(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_looks_like_iso8601_date_only() {
        assert!(looks_like_iso8601("2025-01-15"));
    }

    #[test]
    fn test_looks_like_iso8601_datetime() {
        assert!(looks_like_iso8601("2025-01-15T10:30:00Z"));
    }

    #[test]
    fn test_looks_like_iso8601_invalid_returns_false() {
        assert!(!looks_like_iso8601("not-a-date"));
        assert!(!looks_like_iso8601("2025/01/15"));
        assert!(!looks_like_iso8601(""));
    }
}
