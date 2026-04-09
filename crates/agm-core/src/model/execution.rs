//! Execution state types (spec S26).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::fields::ParseEnumError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Pending,
    Ready,
    InProgress,
    Completed,
    Failed,
    Blocked,
    Skipped,
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Ready => write!(f, "ready"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Blocked => write!(f, "blocked"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl FromStr for ExecutionStatus {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "ready" => Ok(Self::Ready),
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "blocked" => Ok(Self::Blocked),
            "skipped" => Ok(Self::Skipped),
            _ => Err(ParseEnumError {
                type_name: "ExecutionStatus",
                value: s.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_status_from_str_valid_returns_ok() {
        assert_eq!(
            "pending".parse::<ExecutionStatus>().unwrap(),
            ExecutionStatus::Pending
        );
        assert_eq!(
            "ready".parse::<ExecutionStatus>().unwrap(),
            ExecutionStatus::Ready
        );
        assert_eq!(
            "in_progress".parse::<ExecutionStatus>().unwrap(),
            ExecutionStatus::InProgress
        );
        assert_eq!(
            "completed".parse::<ExecutionStatus>().unwrap(),
            ExecutionStatus::Completed
        );
        assert_eq!(
            "failed".parse::<ExecutionStatus>().unwrap(),
            ExecutionStatus::Failed
        );
        assert_eq!(
            "blocked".parse::<ExecutionStatus>().unwrap(),
            ExecutionStatus::Blocked
        );
        assert_eq!(
            "skipped".parse::<ExecutionStatus>().unwrap(),
            ExecutionStatus::Skipped
        );
    }

    #[test]
    fn test_execution_status_from_str_invalid_returns_error() {
        let err = "running".parse::<ExecutionStatus>().unwrap_err();
        assert_eq!(err.type_name, "ExecutionStatus");
        assert_eq!(err.value, "running");
    }

    #[test]
    fn test_execution_status_display_roundtrip() {
        for s in [
            ExecutionStatus::Pending,
            ExecutionStatus::Ready,
            ExecutionStatus::InProgress,
            ExecutionStatus::Completed,
            ExecutionStatus::Failed,
            ExecutionStatus::Blocked,
            ExecutionStatus::Skipped,
        ] {
            let text = s.to_string();
            assert_eq!(text.parse::<ExecutionStatus>().unwrap(), s);
        }
    }

    #[test]
    fn test_execution_status_serde_roundtrip() {
        for s in [
            ExecutionStatus::Pending,
            ExecutionStatus::InProgress,
            ExecutionStatus::Completed,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: ExecutionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn test_execution_status_in_progress_serde_value() {
        let json = serde_json::to_string(&ExecutionStatus::InProgress).unwrap();
        assert_eq!(json, "\"in_progress\"");
    }
}
