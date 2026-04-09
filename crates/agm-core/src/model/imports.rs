//! Import model (spec S10).

use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("invalid import entry: {input:?}")]
pub struct ParseImportError {
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportEntry {
    pub package: String,
    pub version_constraint: Option<String>,
}

impl ImportEntry {
    #[must_use]
    pub fn new(package: String, version_constraint: Option<String>) -> Self {
        Self {
            package,
            version_constraint,
        }
    }
}

impl FromStr for ImportEntry {
    type Err = ParseImportError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseImportError {
                input: s.to_owned(),
            });
        }
        if let Some((package, constraint)) = s.split_once('@') {
            let package = package.trim();
            let constraint = constraint.trim();
            if package.is_empty() {
                return Err(ParseImportError {
                    input: s.to_owned(),
                });
            }
            Ok(Self {
                package: package.to_owned(),
                version_constraint: if constraint.is_empty() {
                    None
                } else {
                    Some(constraint.to_owned())
                },
            })
        } else {
            Ok(Self {
                package: s.to_owned(),
                version_constraint: None,
            })
        }
    }
}

impl std::fmt::Display for ImportEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.package)?;
        if let Some(ref c) = self.version_constraint {
            write!(f, "@{c}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_entry_from_str_package_only() {
        let entry: ImportEntry = "shared.security".parse().unwrap();
        assert_eq!(entry.package, "shared.security");
        assert_eq!(entry.version_constraint, None);
    }

    #[test]
    fn test_import_entry_from_str_with_caret_constraint() {
        let entry: ImportEntry = "shared.security@^1.0.0".parse().unwrap();
        assert_eq!(entry.package, "shared.security");
        assert_eq!(entry.version_constraint, Some("^1.0.0".to_owned()));
    }

    #[test]
    fn test_import_entry_from_str_with_exact_version() {
        let entry: ImportEntry = "shared.http@2.0.0".parse().unwrap();
        assert_eq!(entry.package, "shared.http");
        assert_eq!(entry.version_constraint, Some("2.0.0".to_owned()));
    }

    #[test]
    fn test_import_entry_from_str_with_tilde() {
        let entry: ImportEntry = "core.utils@~1.2.0".parse().unwrap();
        assert_eq!(entry.version_constraint, Some("~1.2.0".to_owned()));
    }

    #[test]
    fn test_import_entry_from_str_with_wildcard() {
        let entry: ImportEntry = "core.utils@1.*".parse().unwrap();
        assert_eq!(entry.version_constraint, Some("1.*".to_owned()));
    }

    #[test]
    fn test_import_entry_from_str_empty_returns_error() {
        assert!("".parse::<ImportEntry>().is_err());
    }

    #[test]
    fn test_import_entry_from_str_no_package_returns_error() {
        assert!("@^1.0.0".parse::<ImportEntry>().is_err());
    }

    #[test]
    fn test_import_entry_display_without_constraint() {
        let entry = ImportEntry::new("shared.http".to_owned(), None);
        assert_eq!(entry.to_string(), "shared.http");
    }

    #[test]
    fn test_import_entry_display_with_constraint() {
        let entry = ImportEntry::new("shared.http".to_owned(), Some("^1.0.0".to_owned()));
        assert_eq!(entry.to_string(), "shared.http@^1.0.0");
    }

    #[test]
    fn test_import_entry_roundtrip() {
        let input = "shared.security@^1.0.0";
        let entry: ImportEntry = input.parse().unwrap();
        assert_eq!(entry.to_string(), input);
    }

    #[test]
    fn test_import_entry_serde_roundtrip() {
        let entry = ImportEntry::new("shared.security".to_owned(), Some("^1.0.0".to_owned()));
        let json = serde_json::to_string(&entry).unwrap();
        let back: ImportEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }
}
