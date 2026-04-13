// crates/agm-lsp/src/diagnostics.rs

use agm_core::error::{AgmError, Severity};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

/// Convert an AgmError to an LSP Diagnostic.
///
/// AGM line numbers are 1-indexed; LSP positions are 0-indexed.
#[must_use]
pub fn to_lsp_diagnostic(error: &AgmError, source: &str) -> Diagnostic {
    let line = error.location.line.unwrap_or(1);
    let lsp_line = if line > 0 { (line - 1) as u32 } else { 0 };

    let line_len = source
        .lines()
        .nth(lsp_line as usize)
        .map(|l| l.len() as u32)
        .unwrap_or(0);

    let range = Range {
        start: Position::new(lsp_line, 0),
        end: Position::new(lsp_line, line_len),
    };

    let severity = match error.severity {
        Severity::Error => Some(DiagnosticSeverity::ERROR),
        Severity::Warning => Some(DiagnosticSeverity::WARNING),
        Severity::Info => Some(DiagnosticSeverity::INFORMATION),
    };

    Diagnostic {
        range,
        severity,
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            error.code.display_code(),
        )),
        code_description: None,
        source: Some("agm".to_owned()),
        message: error.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Convert a slice of AgmErrors to LSP Diagnostics.
#[must_use]
pub fn to_lsp_diagnostics(errors: &[AgmError], source: &str) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| to_lsp_diagnostic(e, source))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agm_core::error::{ErrorCode, ErrorLocation};

    fn make_error(severity: Severity, line: Option<usize>, code: ErrorCode) -> AgmError {
        AgmError::with_severity(
            code,
            severity,
            "test message",
            ErrorLocation::new(None, line, None),
        )
    }

    #[test]
    fn test_to_lsp_diagnostic_error_severity() {
        let err = make_error(Severity::Error, Some(1), ErrorCode::V003);
        let diag = to_lsp_diagnostic(&err, "");
        assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_to_lsp_diagnostic_warning_severity() {
        let err = make_error(Severity::Warning, Some(1), ErrorCode::V010);
        let diag = to_lsp_diagnostic(&err, "");
        assert_eq!(diag.severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn test_to_lsp_diagnostic_info_severity() {
        let err = make_error(Severity::Info, Some(1), ErrorCode::P010);
        let diag = to_lsp_diagnostic(&err, "");
        assert_eq!(diag.severity, Some(DiagnosticSeverity::INFORMATION));
    }

    #[test]
    fn test_to_lsp_diagnostic_line_conversion() {
        // Line 10 (1-indexed) -> LSP line 9 (0-indexed)
        let err = make_error(Severity::Error, Some(10), ErrorCode::V003);
        let source = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
        let diag = to_lsp_diagnostic(&err, source);
        assert_eq!(diag.range.start.line, 9);
    }

    #[test]
    fn test_to_lsp_diagnostic_no_line_defaults_to_zero() {
        let err = make_error(Severity::Error, None, ErrorCode::V003);
        let diag = to_lsp_diagnostic(&err, "");
        assert_eq!(diag.range.start.line, 0);
    }

    #[test]
    fn test_to_lsp_diagnostic_code_format() {
        let err = make_error(Severity::Error, Some(1), ErrorCode::V003);
        let diag = to_lsp_diagnostic(&err, "");
        match diag.code {
            Some(tower_lsp::lsp_types::NumberOrString::String(ref s)) => {
                assert_eq!(s, "AGM-V003");
            }
            _ => panic!("expected string code"),
        }
    }

    #[test]
    fn test_to_lsp_diagnostic_source_is_agm() {
        let err = make_error(Severity::Error, Some(1), ErrorCode::V003);
        let diag = to_lsp_diagnostic(&err, "");
        assert_eq!(diag.source, Some("agm".to_owned()));
    }

    #[test]
    fn test_to_lsp_diagnostics_multiple() {
        let errors = vec![
            make_error(Severity::Error, Some(1), ErrorCode::V003),
            make_error(Severity::Warning, Some(2), ErrorCode::V010),
            make_error(Severity::Info, Some(3), ErrorCode::P010),
        ];
        let diags = to_lsp_diagnostics(&errors, "line1\nline2\nline3\n");
        assert_eq!(diags.len(), 3);
    }
}
