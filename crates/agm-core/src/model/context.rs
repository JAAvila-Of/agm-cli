//! Agent context types (spec S25).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// FileRange
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum FileRange {
    Full,
    Lines(u64, u64),
    Function(String),
}

impl FileRange {
    #[must_use]
    pub fn full() -> Self {
        Self::Full
    }

    #[must_use]
    pub fn lines(start: u64, end: u64) -> Self {
        Self::Lines(start, end)
    }

    #[must_use]
    pub fn function(name: impl Into<String>) -> Self {
        Self::Function(name.into())
    }
}

impl serde::Serialize for FileRange {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Full => serializer.serialize_str("full"),
            Self::Lines(start, end) => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(start)?;
                seq.serialize_element(end)?;
                seq.end()
            }
            Self::Function(name) => serializer.serialize_str(&format!("function: {name}")),
        }
    }
}

impl<'de> serde::Deserialize<'de> for FileRange {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        struct FileRangeVisitor;

        impl<'de> de::Visitor<'de> for FileRangeVisitor {
            type Value = FileRange;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("\"full\", [start, end], or \"function: <name>\"")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v == "full" {
                    Ok(FileRange::Full)
                } else if let Some(name) = v.strip_prefix("function:") {
                    Ok(FileRange::Function(name.trim().to_owned()))
                } else {
                    Err(E::custom(format!("invalid file range: {v:?}")))
                }
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let start: u64 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &"2"))?;
                let end: u64 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &"2"))?;
                Ok(FileRange::Lines(start, end))
            }
        }

        deserializer.deserialize_any(FileRangeVisitor)
    }
}

// ---------------------------------------------------------------------------
// LoadFile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadFile {
    pub path: String,
    pub range: FileRange,
}

// ---------------------------------------------------------------------------
// AgentContext
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_nodes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_files: Option<Vec<LoadFile>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_memory: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_range_full_serde() {
        let r = FileRange::Full;
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"full\"");
        let back: FileRange = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn test_file_range_lines_serde() {
        let r = FileRange::Lines(1, 50);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "[1,50]");
        let back: FileRange = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn test_file_range_function_serde() {
        let r = FileRange::Function("handle_request".to_owned());
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"function: handle_request\"");
        let back: FileRange = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn test_load_file_serde_roundtrip() {
        let lf = LoadFile {
            path: "src/main.rs".to_owned(),
            range: FileRange::Full,
        };
        let json = serde_json::to_string(&lf).unwrap();
        let back: LoadFile = serde_json::from_str(&json).unwrap();
        assert_eq!(lf, back);
    }

    #[test]
    fn test_agent_context_full_serde() {
        let ctx = AgentContext {
            load_nodes: Some(vec!["auth.login".to_owned()]),
            load_files: Some(vec![LoadFile {
                path: "src/auth.rs".to_owned(),
                range: FileRange::Lines(1, 50),
            }]),
            system_hint: Some("Rust project".to_owned()),
            max_tokens: Some(4000),
            load_memory: Some(vec!["rust.repository".to_owned()]),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: AgentContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, back);
    }

    #[test]
    fn test_agent_context_minimal_serde() {
        let ctx = AgentContext {
            load_nodes: None,
            load_files: None,
            system_hint: Some("hint".to_owned()),
            max_tokens: None,
            load_memory: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(!json.contains("load_nodes"));
        assert!(!json.contains("load_files"));
        assert!(!json.contains("max_tokens"));
        assert!(!json.contains("load_memory"));
        let back: AgentContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, back);
    }

    #[test]
    fn test_agent_context_deserialize_from_spec_json() {
        let json = r#"{
            "load_nodes": ["auth.constraints", "auth.session"],
            "load_files": [
                {"path": "src/handlers/auth.rs", "range": "full"}
            ],
            "system_hint": "Rust project using actix-web."
        }"#;
        let ctx: AgentContext = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.load_nodes.as_ref().unwrap().len(), 2);
        assert_eq!(ctx.load_files.as_ref().unwrap().len(), 1);
        assert_eq!(ctx.load_files.as_ref().unwrap()[0].range, FileRange::Full);
    }
}
