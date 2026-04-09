//! Top-level AGM file and header types (spec S8, S9).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::de;
use serde::{Deserialize, Serialize};

use super::imports::ImportEntry;
use super::node::Node;

// ---------------------------------------------------------------------------
// TokenEstimate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum TokenEstimate {
    Count(u64),
    Variable,
}

impl fmt::Display for TokenEstimate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count(n) => write!(f, "{n}"),
            Self::Variable => write!(f, "variable"),
        }
    }
}

impl FromStr for TokenEstimate {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("variable") {
            Ok(Self::Variable)
        } else {
            s.parse::<u64>().map(Self::Count)
        }
    }
}

impl Serialize for TokenEstimate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Count(n) => serializer.serialize_u64(*n),
            Self::Variable => serializer.serialize_str("variable"),
        }
    }
}

impl<'de> Deserialize<'de> for TokenEstimate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = TokenEstimate;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a positive integer or the string \"variable\"")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(TokenEstimate::Count(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                if v >= 0 {
                    Ok(TokenEstimate::Count(v as u64))
                } else {
                    Err(E::custom("token estimate must be non-negative"))
                }
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v.eq_ignore_ascii_case("variable") {
                    Ok(TokenEstimate::Variable)
                } else {
                    Err(E::custom(format!("expected \"variable\", got {v:?}")))
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

// ---------------------------------------------------------------------------
// LoadProfile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadProfile {
    pub filter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<TokenEstimate>,
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub agm: String,
    pub package: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<Vec<ImportEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_load: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_profiles: Option<BTreeMap<String, LoadProfile>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_runtime: Option<String>,
}

// ---------------------------------------------------------------------------
// AgmFile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgmFile {
    #[serde(flatten)]
    pub header: Header,
    pub nodes: Vec<Node>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimate_from_str_number_returns_count() {
        let t: TokenEstimate = "1200".parse().unwrap();
        assert_eq!(t, TokenEstimate::Count(1200));
    }

    #[test]
    fn test_token_estimate_from_str_variable_returns_variable() {
        let t: TokenEstimate = "variable".parse().unwrap();
        assert_eq!(t, TokenEstimate::Variable);
    }

    #[test]
    fn test_token_estimate_from_str_variable_case_insensitive() {
        let t: TokenEstimate = "Variable".parse().unwrap();
        assert_eq!(t, TokenEstimate::Variable);
    }

    #[test]
    fn test_token_estimate_from_str_invalid_returns_error() {
        assert!("not_a_number".parse::<TokenEstimate>().is_err());
    }

    #[test]
    fn test_token_estimate_display_count() {
        assert_eq!(TokenEstimate::Count(4200).to_string(), "4200");
    }

    #[test]
    fn test_token_estimate_display_variable() {
        assert_eq!(TokenEstimate::Variable.to_string(), "variable");
    }

    #[test]
    fn test_token_estimate_serde_count() {
        let t = TokenEstimate::Count(1200);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "1200");
        let back: TokenEstimate = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn test_token_estimate_serde_variable() {
        let t = TokenEstimate::Variable;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"variable\"");
        let back: TokenEstimate = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn test_header_required_fields_only() {
        let h = Header {
            agm: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            title: None,
            owner: None,
            imports: None,
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        };
        assert_eq!(h.agm, "1.0");
        assert_eq!(h.package, "test.pkg");
    }

    #[test]
    fn test_load_profile_serde_roundtrip() {
        let lp = LoadProfile {
            filter: "priority in [critical]".to_owned(),
            estimated_tokens: Some(TokenEstimate::Count(1200)),
        };
        let json = serde_json::to_string(&lp).unwrap();
        let back: LoadProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(lp, back);
    }

    #[test]
    fn test_agm_file_serde_minimal() {
        let file = AgmFile {
            header: Header {
                agm: "1.0".to_owned(),
                package: "test.pkg".to_owned(),
                version: "0.1.0".to_owned(),
                title: None,
                owner: None,
                imports: None,
                default_load: None,
                description: None,
                tags: None,
                status: None,
                load_profiles: None,
                target_runtime: None,
            },
            nodes: vec![],
        };
        let json = serde_json::to_string(&file).unwrap();
        let back: AgmFile = serde_json::from_str(&json).unwrap();
        assert_eq!(file, back);
    }
}
