//! Version constraint validation for AGM imports (spec S10.2).

use semver::VersionReq;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::imports::ImportEntry;

// ---------------------------------------------------------------------------
// ValidatedImport
// ---------------------------------------------------------------------------

/// An import entry whose version constraint has been validated as a semver range.
/// If the original ImportEntry had no version constraint, `version_req` is None
/// (meaning "any version matches").
#[derive(Debug, Clone)]
pub struct ValidatedImport {
    /// The original import entry (package name + raw constraint string).
    pub entry: ImportEntry,
    /// The parsed semver version requirement, or None if no constraint was specified.
    pub version_req: Option<semver::VersionReq>,
}

impl ValidatedImport {
    /// Returns the package name.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.entry.package
    }

    /// Returns true if the given version satisfies this import's constraint.
    /// If no constraint was specified, any version matches.
    #[must_use]
    pub fn matches_version(&self, version: &semver::Version) -> bool {
        match &self.version_req {
            Some(req) => req.matches(version),
            None => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Parses a version constraint string into a semver::VersionReq.
///
/// Thin wrapper around `semver::VersionReq::parse()` that maps parse failures
/// to AgmError with appropriate context.
pub fn parse_version_constraint(constraint: &str) -> Result<VersionReq, AgmError> {
    let trimmed = constraint.trim();
    semver::VersionReq::parse(trimmed).map_err(|e| {
        AgmError::new(
            ErrorCode::I002,
            format!("Invalid version constraint: `{constraint}` ({e})"),
            ErrorLocation::default(),
        )
    })
}

/// Validates an ImportEntry's version constraint string as a semver range.
///
/// If the entry has no version constraint (`None`), returns a ValidatedImport
/// with `version_req: None` (any version matches).
///
/// If the constraint string is present but not a valid semver range, returns
/// an error using error code I002.
///
/// This function does NOT modify the original ImportEntry.
pub fn validate_import(entry: &ImportEntry) -> Result<ValidatedImport, AgmError> {
    match &entry.version_constraint {
        None => Ok(ValidatedImport {
            entry: entry.clone(),
            version_req: None,
        }),
        Some(constraint) => {
            let req = parse_version_constraint(constraint)?;
            Ok(ValidatedImport {
                entry: entry.clone(),
                version_req: Some(req),
            })
        }
    }
}

/// Validates all imports in a header, returning validated imports and any errors.
///
/// Errors are collected (not short-circuited): all imports are attempted.
/// Returns (validated_imports, errors).
pub fn validate_all_imports(imports: &[ImportEntry]) -> (Vec<ValidatedImport>, Vec<AgmError>) {
    let mut validated = Vec::new();
    let mut errors = Vec::new();

    for entry in imports {
        match validate_import(entry) {
            Ok(v) => validated.push(v),
            Err(e) => errors.push(e),
        }
    }

    (validated, errors)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::model::imports::ImportEntry;

    // Category A: Version constraint parsing

    #[test]
    fn test_parse_version_constraint_caret_returns_req() {
        let req = parse_version_constraint("^1.0.0").unwrap();
        assert!(req.matches(&semver::Version::parse("1.2.3").unwrap()));
        assert!(!req.matches(&semver::Version::parse("2.0.0").unwrap()));
    }

    #[test]
    fn test_parse_version_constraint_tilde_returns_req() {
        let req = parse_version_constraint("~1.2.0").unwrap();
        assert!(req.matches(&semver::Version::parse("1.2.5").unwrap()));
        assert!(!req.matches(&semver::Version::parse("1.3.0").unwrap()));
    }

    #[test]
    fn test_parse_version_constraint_exact_returns_req() {
        // semver crate: a bare "2.0.0" is treated as "^2.0.0" (caret default).
        // Use "=2.0.0" for an exact-match requirement.
        let req = parse_version_constraint("=2.0.0").unwrap();
        assert!(req.matches(&semver::Version::parse("2.0.0").unwrap()));
        assert!(!req.matches(&semver::Version::parse("2.0.1").unwrap()));
    }

    #[test]
    fn test_parse_version_constraint_wildcard_returns_req() {
        let req = parse_version_constraint("1.*").unwrap();
        assert!(req.matches(&semver::Version::parse("1.0.0").unwrap()));
        assert!(req.matches(&semver::Version::parse("1.9.9").unwrap()));
        assert!(!req.matches(&semver::Version::parse("2.0.0").unwrap()));
    }

    #[test]
    fn test_parse_version_constraint_invalid_returns_error() {
        let err = parse_version_constraint("not_semver").unwrap_err();
        assert_eq!(err.code, ErrorCode::I002);
    }

    // Category B: Import validation

    #[test]
    fn test_validate_import_with_constraint_returns_validated() {
        let entry = ImportEntry::new("shared.security".to_owned(), Some("^1.0.0".to_owned()));
        let result = validate_import(&entry).unwrap();
        assert_eq!(result.package(), "shared.security");
        assert!(result.version_req.is_some());
    }

    #[test]
    fn test_validate_import_without_constraint_returns_none_req() {
        let entry = ImportEntry::new("shared.http".to_owned(), None);
        let result = validate_import(&entry).unwrap();
        assert_eq!(result.package(), "shared.http");
        assert!(result.version_req.is_none());
    }

    #[test]
    fn test_validate_import_invalid_constraint_returns_error() {
        let entry = ImportEntry::new("pkg".to_owned(), Some("bogus".to_owned()));
        let err = validate_import(&entry).unwrap_err();
        assert_eq!(err.code, ErrorCode::I002);
    }

    #[test]
    fn test_validate_all_imports_mixed_returns_partial() {
        let imports = vec![
            ImportEntry::new("shared.security".to_owned(), Some("^1.0.0".to_owned())),
            ImportEntry::new("bad.pkg".to_owned(), Some("bogus".to_owned())),
        ];
        let (validated, errors) = validate_all_imports(&imports);
        assert_eq!(validated.len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(validated[0].package(), "shared.security");
        assert_eq!(errors[0].code, ErrorCode::I002);
    }

    // Category C: ValidatedImport::matches_version

    #[test]
    fn test_matches_version_with_caret_matching_returns_true() {
        let entry = ImportEntry::new("pkg".to_owned(), Some("^1.0.0".to_owned()));
        let validated = validate_import(&entry).unwrap();
        let version = semver::Version::parse("1.5.0").unwrap();
        assert!(validated.matches_version(&version));
    }

    #[test]
    fn test_matches_version_with_caret_not_matching_returns_false() {
        let entry = ImportEntry::new("pkg".to_owned(), Some("^1.0.0".to_owned()));
        let validated = validate_import(&entry).unwrap();
        let version = semver::Version::parse("2.0.0").unwrap();
        assert!(!validated.matches_version(&version));
    }

    #[test]
    fn test_matches_version_none_constraint_returns_true() {
        let entry = ImportEntry::new("pkg".to_owned(), None);
        let validated = validate_import(&entry).unwrap();
        let version = semver::Version::parse("99.99.99").unwrap();
        assert!(validated.matches_version(&version));
    }
}
