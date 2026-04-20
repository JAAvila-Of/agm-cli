//! Memory model types (spec S28).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::fields::ParseEnumError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAction {
    Get,
    Upsert,
    Delete,
    List,
    Search,
}

impl fmt::Display for MemoryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => write!(f, "get"),
            Self::Upsert => write!(f, "upsert"),
            Self::Delete => write!(f, "delete"),
            Self::List => write!(f, "list"),
            Self::Search => write!(f, "search"),
        }
    }
}

impl FromStr for MemoryAction {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "get" => Ok(Self::Get),
            "upsert" => Ok(Self::Upsert),
            "delete" => Ok(Self::Delete),
            "list" => Ok(Self::List),
            "search" => Ok(Self::Search),
            _ => Err(ParseEnumError {
                type_name: "MemoryAction",
                value: s.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Node,
    Session,
    Project,
    Global,
}

impl fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node => write!(f, "node"),
            Self::Session => write!(f, "session"),
            Self::Project => write!(f, "project"),
            Self::Global => write!(f, "global"),
        }
    }
}

impl FromStr for MemoryScope {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "node" => Ok(Self::Node),
            "session" => Ok(Self::Session),
            "project" => Ok(Self::Project),
            "global" => Ok(Self::Global),
            _ => Err(ParseEnumError {
                type_name: "MemoryScope",
                value: s.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryTtl {
    Permanent,
    Session,
    Duration(String),
}

impl fmt::Display for MemoryTtl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permanent => write!(f, "permanent"),
            Self::Session => write!(f, "session"),
            Self::Duration(d) => write!(f, "duration:{d}"),
        }
    }
}

impl FromStr for MemoryTtl {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "permanent" => Ok(Self::Permanent),
            "session" => Ok(Self::Session),
            _ if s.starts_with("duration:") => {
                let duration = s.strip_prefix("duration:").unwrap().to_owned();
                if duration.is_empty() {
                    return Err(ParseEnumError {
                        type_name: "MemoryTtl",
                        value: s.to_owned(),
                    });
                }
                Ok(Self::Duration(duration))
            }
            _ => Err(ParseEnumError {
                type_name: "MemoryTtl",
                value: s.to_owned(),
            }),
        }
    }
}

impl Serialize for MemoryTtl {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for MemoryTtl {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub topic: String,
    pub action: MemoryAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<MemoryTtl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(flatten, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extra_fields: std::collections::BTreeMap<String, super::fields::FieldValue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_action_from_str_valid_returns_ok() {
        assert_eq!("get".parse::<MemoryAction>().unwrap(), MemoryAction::Get);
        assert_eq!(
            "upsert".parse::<MemoryAction>().unwrap(),
            MemoryAction::Upsert
        );
        assert_eq!(
            "delete".parse::<MemoryAction>().unwrap(),
            MemoryAction::Delete
        );
        assert_eq!("list".parse::<MemoryAction>().unwrap(), MemoryAction::List);
        assert_eq!(
            "search".parse::<MemoryAction>().unwrap(),
            MemoryAction::Search
        );
    }

    #[test]
    fn test_memory_action_from_str_invalid_returns_error() {
        let err = "update".parse::<MemoryAction>().unwrap_err();
        assert_eq!(err.type_name, "MemoryAction");
    }

    #[test]
    fn test_memory_action_display_roundtrip() {
        for a in [
            MemoryAction::Get,
            MemoryAction::Upsert,
            MemoryAction::Delete,
            MemoryAction::List,
            MemoryAction::Search,
        ] {
            let text = a.to_string();
            assert_eq!(text.parse::<MemoryAction>().unwrap(), a);
        }
    }

    #[test]
    fn test_memory_scope_from_str_valid_returns_ok() {
        assert_eq!("node".parse::<MemoryScope>().unwrap(), MemoryScope::Node);
        assert_eq!(
            "session".parse::<MemoryScope>().unwrap(),
            MemoryScope::Session
        );
        assert_eq!(
            "project".parse::<MemoryScope>().unwrap(),
            MemoryScope::Project
        );
        assert_eq!(
            "global".parse::<MemoryScope>().unwrap(),
            MemoryScope::Global
        );
    }

    #[test]
    fn test_memory_scope_from_str_invalid_returns_error() {
        let err = "workspace".parse::<MemoryScope>().unwrap_err();
        assert_eq!(err.type_name, "MemoryScope");
    }

    #[test]
    fn test_memory_scope_display_roundtrip() {
        for s in [
            MemoryScope::Node,
            MemoryScope::Session,
            MemoryScope::Project,
            MemoryScope::Global,
        ] {
            let text = s.to_string();
            assert_eq!(text.parse::<MemoryScope>().unwrap(), s);
        }
    }

    #[test]
    fn test_memory_ttl_from_str_permanent_returns_ok() {
        assert_eq!(
            "permanent".parse::<MemoryTtl>().unwrap(),
            MemoryTtl::Permanent
        );
    }

    #[test]
    fn test_memory_ttl_from_str_session_returns_ok() {
        assert_eq!("session".parse::<MemoryTtl>().unwrap(), MemoryTtl::Session);
    }

    #[test]
    fn test_memory_ttl_from_str_duration_returns_ok() {
        assert_eq!(
            "duration:P7D".parse::<MemoryTtl>().unwrap(),
            MemoryTtl::Duration("P7D".to_owned())
        );
    }

    #[test]
    fn test_memory_ttl_from_str_duration_pt1h_returns_ok() {
        assert_eq!(
            "duration:PT1H".parse::<MemoryTtl>().unwrap(),
            MemoryTtl::Duration("PT1H".to_owned())
        );
    }

    #[test]
    fn test_memory_ttl_from_str_empty_duration_returns_error() {
        assert!("duration:".parse::<MemoryTtl>().is_err());
    }

    #[test]
    fn test_memory_ttl_from_str_invalid_returns_error() {
        let err = "forever".parse::<MemoryTtl>().unwrap_err();
        assert_eq!(err.type_name, "MemoryTtl");
    }

    #[test]
    fn test_memory_ttl_display_roundtrip() {
        for t in [
            MemoryTtl::Permanent,
            MemoryTtl::Session,
            MemoryTtl::Duration("P30D".to_owned()),
        ] {
            let text = t.to_string();
            assert_eq!(text.parse::<MemoryTtl>().unwrap(), t);
        }
    }

    #[test]
    fn test_memory_ttl_serde_roundtrip() {
        for t in [
            MemoryTtl::Permanent,
            MemoryTtl::Session,
            MemoryTtl::Duration("P7D".to_owned()),
        ] {
            let json = serde_json::to_string(&t).unwrap();
            let back: MemoryTtl = serde_json::from_str(&json).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn test_memory_entry_upsert_serde_roundtrip() {
        let entry = MemoryEntry {
            key: "repo.pattern".to_owned(),
            topic: "rust.repository".to_owned(),
            action: MemoryAction::Upsert,
            value: Some("row_to_column uses get()".to_owned()),
            scope: Some(MemoryScope::Project),
            ttl: Some(MemoryTtl::Permanent),
            query: None,
            max_results: None,
            extra_fields: Default::default(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn test_memory_entry_search_serde_roundtrip() {
        let entry = MemoryEntry {
            key: "search.patterns".to_owned(),
            topic: "rust.repository".to_owned(),
            action: MemoryAction::Search,
            value: None,
            scope: None,
            ttl: None,
            query: Some("how are optional fields handled".to_owned()),
            max_results: Some(5),
            extra_fields: Default::default(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn test_memory_entry_optional_fields_absent() {
        let entry = MemoryEntry {
            key: "test.key".to_owned(),
            topic: "test".to_owned(),
            action: MemoryAction::Get,
            value: None,
            scope: None,
            ttl: None,
            query: None,
            max_results: None,
            extra_fields: Default::default(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("value"));
        assert!(!json.contains("scope"));
        assert!(!json.contains("ttl"));
        assert!(!json.contains("query"));
        assert!(!json.contains("max_results"));
    }
}
