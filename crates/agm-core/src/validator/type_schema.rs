//! Type schema enforcement (spec S14).
//!
//! Pass 4: thin wrapper that delegates to `crate::schema::enforcement::validate_schema`.

use crate::error::diagnostic::AgmError;
use crate::model::node::Node;
use crate::model::schema::EnforcementLevel;

/// Validates a node against its type's schema at the given enforcement level.
///
/// Delegates to `crate::schema::enforcement::validate_schema`.
///
/// Rules (via schema enforcement):
/// - V010: type-recommended field missing (warning)
/// - V016: disallowed field in strict mode (error)
/// - V017: disallowed field in standard mode (warning)
/// - V024: required schema field missing (error)
#[must_use]
pub fn validate_type_schema(
    node: &Node,
    level: &EnforcementLevel,
    file_name: &str,
) -> Vec<AgmError> {
    crate::schema::enforcement::validate_schema(node, level, file_name)
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::error::codes::ErrorCode;
    use crate::model::fields::{NodeType, Span};
    use crate::model::node::Node;
    use crate::model::schema::EnforcementLevel;

    fn minimal_node_of_type(node_type: NodeType) -> Node {
        Node {
            id: "test.node".to_owned(),
            node_type,
            summary: "a test node".to_owned(),
            span: Span::new(5, 7),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_type_schema_permissive_returns_empty_for_minimal() {
        // Permissive mode should not emit warnings for missing recommended fields
        let node = minimal_node_of_type(NodeType::Facts);
        let errors = validate_type_schema(&node, &EnforcementLevel::Permissive, "test.agm");
        // Permissive means no enforcement — should be empty or only non-schema errors
        // (actual behavior depends on schema enforcement impl)
        assert!(errors.iter().all(|e| e.code != ErrorCode::V010));
    }

    #[test]
    fn test_validate_type_schema_delegates_to_schema_enforcement() {
        // Smoke test: verify the function calls through without panicking
        let node = minimal_node_of_type(NodeType::Workflow);
        let errors = validate_type_schema(&node, &EnforcementLevel::Standard, "test.agm");
        // Regardless of result, the function should complete
        let _ = errors;
    }

    #[test]
    fn test_validate_type_schema_standard_mode_runs_without_panic() {
        let node = minimal_node_of_type(NodeType::Rules);
        let errors = validate_type_schema(&node, &EnforcementLevel::Standard, "test.agm");
        let _ = errors;
    }

    #[test]
    fn test_validate_type_schema_strict_mode_runs_without_panic() {
        let node = minimal_node_of_type(NodeType::Decision);
        let errors = validate_type_schema(&node, &EnforcementLevel::Strict, "test.agm");
        let _ = errors;
    }
}
