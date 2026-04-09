//! Type schema definitions (spec S14).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::fields::ParseEnumError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementLevel {
    Strict,
    Standard,
    Permissive,
}

impl fmt::Display for EnforcementLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strict => write!(f, "strict"),
            Self::Standard => write!(f, "standard"),
            Self::Permissive => write!(f, "permissive"),
        }
    }
}

impl FromStr for EnforcementLevel {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "strict" => Ok(Self::Strict),
            "standard" => Ok(Self::Standard),
            "permissive" => Ok(Self::Permissive),
            _ => Err(ParseEnumError {
                type_name: "EnforcementLevel",
                value: s.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeSchema {
    pub required: Vec<String>,
    pub recommended: Vec<String>,
    pub allowed: Vec<String>,
    pub disallowed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enforcement_level_from_str_valid_returns_ok() {
        assert_eq!(
            "strict".parse::<EnforcementLevel>().unwrap(),
            EnforcementLevel::Strict
        );
        assert_eq!(
            "standard".parse::<EnforcementLevel>().unwrap(),
            EnforcementLevel::Standard
        );
        assert_eq!(
            "permissive".parse::<EnforcementLevel>().unwrap(),
            EnforcementLevel::Permissive
        );
    }

    #[test]
    fn test_enforcement_level_from_str_invalid_returns_error() {
        let err = "lenient".parse::<EnforcementLevel>().unwrap_err();
        assert_eq!(err.type_name, "EnforcementLevel");
    }

    #[test]
    fn test_enforcement_level_display_roundtrip() {
        for e in [
            EnforcementLevel::Strict,
            EnforcementLevel::Standard,
            EnforcementLevel::Permissive,
        ] {
            let text = e.to_string();
            assert_eq!(text.parse::<EnforcementLevel>().unwrap(), e);
        }
    }

    #[test]
    fn test_type_schema_serde_roundtrip() {
        let schema = TypeSchema {
            required: vec!["summary".to_owned()],
            recommended: vec!["items".to_owned(), "stability".to_owned()],
            allowed: vec!["detail".to_owned(), "tags".to_owned()],
            disallowed: vec!["steps".to_owned(), "parallel_groups".to_owned()],
        };
        let json = serde_json::to_string(&schema).unwrap();
        let back: TypeSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema, back);
    }
}
