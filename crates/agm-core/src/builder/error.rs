//! `BuildError` — unified error type for the builder API.

use crate::error::diagnostic::DiagnosticCollection;

// ---------------------------------------------------------------------------
// BuildError
// ---------------------------------------------------------------------------

/// Error returned by builder `.build()` methods.
///
/// Callers can inspect the variant to distinguish structural pre-condition
/// failures (e.g. mismatched anchor/old flags) from validation failures
/// (same diagnostics as `agm validate`).
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Validation ran and found at least one error-level diagnostic.
    ///
    /// The boxed `DiagnosticCollection` contains the full sorted list of
    /// errors (and any warnings that were produced alongside them).
    #[error("validation failed with {count} error(s)", count = .0.error_count())]
    Validation(Box<DiagnosticCollection>),

    /// A builder pre-condition was not satisfied before validation could run.
    ///
    /// Examples: `CodeBlockBuilder::build` with both `anchor` and `old` set
    /// for a `Replace` action.
    #[error("{0}")]
    Precondition(String),
}

impl BuildError {
    /// Returns the diagnostic collection if this is a `Validation` error.
    #[must_use]
    pub fn diagnostics(&self) -> Option<&DiagnosticCollection> {
        match self {
            Self::Validation(dc) => Some(dc),
            Self::Precondition(_) => None,
        }
    }

    /// Returns `true` if this is a `Validation` error.
    #[must_use]
    pub fn is_validation(&self) -> bool {
        matches!(self, Self::Validation(_))
    }

    /// Returns `true` if this is a `Precondition` error.
    #[must_use]
    pub fn is_precondition(&self) -> bool {
        matches!(self, Self::Precondition(_))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_empty_collection() -> DiagnosticCollection {
        DiagnosticCollection::new("<builder>", "")
    }

    #[test]
    fn test_build_error_display_precondition() {
        let err = BuildError::Precondition("anchor and old are mutually exclusive".to_owned());
        assert_eq!(err.to_string(), "anchor and old are mutually exclusive");
    }

    #[test]
    fn test_build_error_display_validation() {
        use crate::error::codes::ErrorCode;
        use crate::error::diagnostic::{AgmError, ErrorLocation};

        let mut dc = make_empty_collection();
        dc.push(AgmError::new(
            ErrorCode::V002,
            "empty node id",
            ErrorLocation::default(),
        ));
        let err = BuildError::Validation(Box::new(dc));
        assert!(err.to_string().contains("1 error"));
    }

    #[test]
    fn test_diagnostics_accessor_validation_returns_some() {
        let dc = make_empty_collection();
        let err = BuildError::Validation(Box::new(dc));
        assert!(err.diagnostics().is_some());
        assert!(err.is_validation());
        assert!(!err.is_precondition());
    }

    #[test]
    fn test_diagnostics_accessor_precondition_returns_none() {
        let err = BuildError::Precondition("oops".to_owned());
        assert!(err.diagnostics().is_none());
        assert!(!err.is_validation());
        assert!(err.is_precondition());
    }
}
