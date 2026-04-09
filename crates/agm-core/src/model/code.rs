//! Code block types (spec S23).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::fields::ParseEnumError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeAction {
    Create,
    Append,
    Prepend,
    Replace,
    InsertBefore,
    InsertAfter,
    Full,
}

impl fmt::Display for CodeAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create => write!(f, "create"),
            Self::Append => write!(f, "append"),
            Self::Prepend => write!(f, "prepend"),
            Self::Replace => write!(f, "replace"),
            Self::InsertBefore => write!(f, "insert_before"),
            Self::InsertAfter => write!(f, "insert_after"),
            Self::Full => write!(f, "full"),
        }
    }
}

impl FromStr for CodeAction {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "create" => Ok(Self::Create),
            "append" => Ok(Self::Append),
            "prepend" => Ok(Self::Prepend),
            "replace" => Ok(Self::Replace),
            "insert_before" => Ok(Self::InsertBefore),
            "insert_after" => Ok(Self::InsertAfter),
            "full" => Ok(Self::Full),
            _ => Err(ParseEnumError {
                type_name: "CodeAction",
                value: s.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub action: CodeAction,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_action_from_str_valid_returns_ok() {
        assert_eq!("create".parse::<CodeAction>().unwrap(), CodeAction::Create);
        assert_eq!("append".parse::<CodeAction>().unwrap(), CodeAction::Append);
        assert_eq!(
            "prepend".parse::<CodeAction>().unwrap(),
            CodeAction::Prepend
        );
        assert_eq!(
            "replace".parse::<CodeAction>().unwrap(),
            CodeAction::Replace
        );
        assert_eq!(
            "insert_before".parse::<CodeAction>().unwrap(),
            CodeAction::InsertBefore
        );
        assert_eq!(
            "insert_after".parse::<CodeAction>().unwrap(),
            CodeAction::InsertAfter
        );
        assert_eq!("full".parse::<CodeAction>().unwrap(), CodeAction::Full);
    }

    #[test]
    fn test_code_action_from_str_invalid_returns_error() {
        let err = "overwrite".parse::<CodeAction>().unwrap_err();
        assert_eq!(err.type_name, "CodeAction");
        assert_eq!(err.value, "overwrite");
    }

    #[test]
    fn test_code_action_display_roundtrip() {
        for a in [
            CodeAction::Create,
            CodeAction::Append,
            CodeAction::Prepend,
            CodeAction::Replace,
            CodeAction::InsertBefore,
            CodeAction::InsertAfter,
            CodeAction::Full,
        ] {
            let s = a.to_string();
            assert_eq!(s.parse::<CodeAction>().unwrap(), a);
        }
    }

    #[test]
    fn test_code_block_serde_roundtrip() {
        let cb = CodeBlock {
            lang: Some("rust".to_owned()),
            target: Some("src/main.rs".to_owned()),
            action: CodeAction::Append,
            body: "fn main() {}".to_owned(),
            anchor: None,
            old: None,
        };
        let json = serde_json::to_string(&cb).unwrap();
        let back: CodeBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(cb, back);
    }

    #[test]
    fn test_code_block_with_replace_fields() {
        let cb = CodeBlock {
            lang: Some("rust".to_owned()),
            target: Some("src/lib.rs".to_owned()),
            action: CodeAction::Replace,
            body: "fn new_impl() {}".to_owned(),
            anchor: None,
            old: Some("fn old_impl() {}".to_owned()),
        };
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains("old_impl"));
        let back: CodeBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(cb, back);
    }

    #[test]
    fn test_code_block_with_anchor() {
        let cb = CodeBlock {
            lang: Some("rust".to_owned()),
            target: Some("src/lib.rs".to_owned()),
            action: CodeAction::InsertAfter,
            body: "    pub new_field: String,".to_owned(),
            anchor: Some("pub struct Foo {".to_owned()),
            old: None,
        };
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains("anchor"));
        let back: CodeBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(cb, back);
    }
}
