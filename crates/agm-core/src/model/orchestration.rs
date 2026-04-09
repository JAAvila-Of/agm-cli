//! Orchestration types (spec S13.10, S27).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::fields::ParseEnumError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    Sequential,
    Parallel,
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sequential => write!(f, "sequential"),
            Self::Parallel => write!(f, "parallel"),
        }
    }
}

impl FromStr for Strategy {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sequential" => Ok(Self::Sequential),
            "parallel" => Ok(Self::Parallel),
            _ => Err(ParseEnumError {
                type_name: "Strategy",
                value: s.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParallelGroup {
    pub group: String,
    pub nodes: Vec<String>,
    pub strategy: Strategy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_from_str_valid_returns_ok() {
        assert_eq!(
            "sequential".parse::<Strategy>().unwrap(),
            Strategy::Sequential
        );
        assert_eq!("parallel".parse::<Strategy>().unwrap(), Strategy::Parallel);
    }

    #[test]
    fn test_strategy_from_str_invalid_returns_error() {
        let err = "concurrent".parse::<Strategy>().unwrap_err();
        assert_eq!(err.type_name, "Strategy");
    }

    #[test]
    fn test_strategy_display_roundtrip() {
        for s in [Strategy::Sequential, Strategy::Parallel] {
            let text = s.to_string();
            assert_eq!(text.parse::<Strategy>().unwrap(), s);
        }
    }

    #[test]
    fn test_parallel_group_serde_roundtrip() {
        let pg = ParallelGroup {
            group: "1-schema".to_owned(),
            nodes: vec!["migration.schema".to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        };
        let json = serde_json::to_string(&pg).unwrap();
        let back: ParallelGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(pg, back);
    }

    #[test]
    fn test_parallel_group_with_requires_and_concurrency() {
        let pg = ParallelGroup {
            group: "3-backend".to_owned(),
            nodes: vec!["backend.repo".to_owned(), "backend.cmd".to_owned()],
            strategy: Strategy::Parallel,
            requires: Some(vec!["2-models".to_owned()]),
            max_concurrency: Some(4),
        };
        let json = serde_json::to_string(&pg).unwrap();
        let back: ParallelGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(pg, back);
    }

    #[test]
    fn test_parallel_group_optional_fields_absent_in_json() {
        let pg = ParallelGroup {
            group: "1-test".to_owned(),
            nodes: vec![],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        };
        let json = serde_json::to_string(&pg).unwrap();
        assert!(!json.contains("requires"));
        assert!(!json.contains("max_concurrency"));
    }
}
