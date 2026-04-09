//! State sidecar model types for `.agm.state` files.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::execution::ExecutionStatus;

/// Represents a parsed `.agm.state` sidecar file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateFile {
    pub format_version: String,
    pub package: String,
    pub version: String,
    pub session_id: String,
    pub started_at: String,
    pub updated_at: String,
    pub nodes: BTreeMap<String, NodeState>,
}

/// Execution state of a single node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeState {
    pub execution_status: ExecutionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_log: Option<String>,
    pub retry_count: u32,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            execution_status: ExecutionStatus::Pending,
            executed_by: None,
            executed_at: None,
            execution_log: None,
            retry_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // A: Default
    // -----------------------------------------------------------------------

    #[test]
    fn test_node_state_default_is_pending_with_zero_retries() {
        let state = NodeState::default();
        assert_eq!(state.execution_status, ExecutionStatus::Pending);
        assert_eq!(state.retry_count, 0);
        assert!(state.executed_by.is_none());
        assert!(state.executed_at.is_none());
        assert!(state.execution_log.is_none());
    }

    // -----------------------------------------------------------------------
    // B: Serde roundtrip — minimal
    // -----------------------------------------------------------------------

    #[test]
    fn test_state_file_serde_roundtrip_minimal() {
        let state = StateFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            session_id: "run-001".to_owned(),
            started_at: "2026-04-08T10:00:00Z".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            nodes: BTreeMap::new(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: StateFile = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    // -----------------------------------------------------------------------
    // C: Serde roundtrip — full
    // -----------------------------------------------------------------------

    #[test]
    fn test_state_file_serde_roundtrip_full() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "migration.001".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                executed_by: Some("shell-agent".to_owned()),
                executed_at: Some("2026-04-08T10:05:00Z".to_owned()),
                execution_log: Some(".agm/logs/migration.001.log".to_owned()),
                retry_count: 1,
            },
        );
        nodes.insert(
            "migration.002".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending,
                executed_by: None,
                executed_at: None,
                execution_log: None,
                retry_count: 0,
            },
        );
        let state = StateFile {
            format_version: "1.0".to_owned(),
            package: "acme.migration".to_owned(),
            version: "1.0.0".to_owned(),
            session_id: "run-2026-04-08-153200".to_owned(),
            started_at: "2026-04-08T15:32:00Z".to_owned(),
            updated_at: "2026-04-08T15:35:00Z".to_owned(),
            nodes,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: StateFile = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    // -----------------------------------------------------------------------
    // D: Optional fields absent in JSON when None
    // -----------------------------------------------------------------------

    #[test]
    fn test_node_state_optional_fields_absent_when_none() {
        let state = NodeState::default();
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("executed_by"));
        assert!(!json.contains("executed_at"));
        assert!(!json.contains("execution_log"));
    }

    // -----------------------------------------------------------------------
    // E: NodeState serde roundtrip with all fields set
    // -----------------------------------------------------------------------

    #[test]
    fn test_node_state_serde_roundtrip_all_fields() {
        let state = NodeState {
            execution_status: ExecutionStatus::Failed,
            executed_by: Some("ci-agent".to_owned()),
            executed_at: Some("2026-04-08T12:00:00Z".to_owned()),
            execution_log: Some(".agm/logs/test.log".to_owned()),
            retry_count: 3,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: NodeState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }
}
