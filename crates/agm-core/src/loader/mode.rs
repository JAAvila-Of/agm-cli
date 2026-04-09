//! Load mode definitions for the AGM loader.
//!
//! Defines [`LoadMode`] and [`FieldCategory`] used to classify which fields
//! are included when producing a filtered view of an AGM file.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ParseLoadModeError
// ---------------------------------------------------------------------------

/// Error returned when parsing a string into a [`LoadMode`] fails.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("invalid load mode: {value:?}; expected one of: summary, operational, executable, full")]
pub struct ParseLoadModeError {
    pub value: String,
}

// ---------------------------------------------------------------------------
// LoadMode
// ---------------------------------------------------------------------------

/// The four loading modes defined in the AGM specification.
///
/// Each mode is a superset of the previous: Summary ⊂ Operational ⊂
/// Executable ⊂ Full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadMode {
    /// Minimal view: id, type, summary, priority, stability, depends, tags.
    Summary,
    /// Adds operational fields: items, steps, fields, input, output.
    Operational,
    /// Adds executable fields: code, verify, agent_context, execution state.
    Executable,
    /// Everything, including explanatory and relational fields.
    Full,
}

impl fmt::Display for LoadMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Summary => write!(f, "summary"),
            Self::Operational => write!(f, "operational"),
            Self::Executable => write!(f, "executable"),
            Self::Full => write!(f, "full"),
        }
    }
}

impl FromStr for LoadMode {
    type Err = ParseLoadModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "summary" => Ok(Self::Summary),
            "operational" => Ok(Self::Operational),
            "executable" => Ok(Self::Executable),
            "full" => Ok(Self::Full),
            _ => Err(ParseLoadModeError {
                value: s.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// FieldCategory
// ---------------------------------------------------------------------------

/// The category a node field belongs to, determining which [`LoadMode`]
/// first includes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldCategory {
    Summary,
    Operational,
    Executable,
    Full,
}

impl FieldCategory {
    /// Returns `true` if this category's fields are included in `mode`.
    #[must_use]
    pub(crate) fn included_in(self, mode: LoadMode) -> bool {
        match mode {
            LoadMode::Summary => self == FieldCategory::Summary,
            LoadMode::Operational => {
                matches!(self, FieldCategory::Summary | FieldCategory::Operational)
            }
            LoadMode::Executable => matches!(
                self,
                FieldCategory::Summary | FieldCategory::Operational | FieldCategory::Executable
            ),
            LoadMode::Full => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_mode_from_str_valid_returns_ok() {
        assert_eq!("summary".parse::<LoadMode>().unwrap(), LoadMode::Summary);
        assert_eq!(
            "operational".parse::<LoadMode>().unwrap(),
            LoadMode::Operational
        );
        assert_eq!(
            "executable".parse::<LoadMode>().unwrap(),
            LoadMode::Executable
        );
        assert_eq!("full".parse::<LoadMode>().unwrap(), LoadMode::Full);
    }

    #[test]
    fn test_load_mode_from_str_invalid_returns_error() {
        let err = "debug".parse::<LoadMode>().unwrap_err();
        assert!(err.value == "debug");
    }

    #[test]
    fn test_load_mode_display_roundtrip() {
        for mode in [
            LoadMode::Summary,
            LoadMode::Operational,
            LoadMode::Executable,
            LoadMode::Full,
        ] {
            let s = mode.to_string();
            let parsed: LoadMode = s.parse().unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_field_category_summary_included_only_in_summary_and_above() {
        assert!(FieldCategory::Summary.included_in(LoadMode::Summary));
        assert!(FieldCategory::Summary.included_in(LoadMode::Operational));
        assert!(FieldCategory::Summary.included_in(LoadMode::Executable));
        assert!(FieldCategory::Summary.included_in(LoadMode::Full));
    }

    #[test]
    fn test_field_category_operational_not_in_summary() {
        assert!(!FieldCategory::Operational.included_in(LoadMode::Summary));
        assert!(FieldCategory::Operational.included_in(LoadMode::Operational));
        assert!(FieldCategory::Operational.included_in(LoadMode::Executable));
        assert!(FieldCategory::Operational.included_in(LoadMode::Full));
    }

    #[test]
    fn test_field_category_executable_not_in_summary_or_operational() {
        assert!(!FieldCategory::Executable.included_in(LoadMode::Summary));
        assert!(!FieldCategory::Executable.included_in(LoadMode::Operational));
        assert!(FieldCategory::Executable.included_in(LoadMode::Executable));
        assert!(FieldCategory::Executable.included_in(LoadMode::Full));
    }

    #[test]
    fn test_field_category_full_only_in_full() {
        assert!(!FieldCategory::Full.included_in(LoadMode::Summary));
        assert!(!FieldCategory::Full.included_in(LoadMode::Operational));
        assert!(!FieldCategory::Full.included_in(LoadMode::Executable));
        assert!(FieldCategory::Full.included_in(LoadMode::Full));
    }

    #[test]
    fn test_load_mode_serde_roundtrip() {
        let m = LoadMode::Executable;
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "\"executable\"");
        let back: LoadMode = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn test_parse_load_mode_error_message_contains_value() {
        let err = "bad_mode".parse::<LoadMode>().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("bad_mode"));
    }
}
