//! Core diagnostic types: `Severity`, `ErrorLocation`, `AgmError`,
//! and `DiagnosticCollection`.

use std::fmt;

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceSpan};
use serde::{Deserialize, Serialize};

use super::codes::ErrorCode;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Diagnostic severity level (spec section 21.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    #[must_use]
    pub fn to_miette(self) -> miette::Severity {
        match self {
            Self::Error => miette::Severity::Error,
            Self::Warning => miette::Severity::Warning,
            Self::Info => miette::Severity::Advice,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// ErrorLocation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ErrorLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
}

impl ErrorLocation {
    #[must_use]
    pub fn new(file: Option<String>, line: Option<usize>, node: Option<String>) -> Self {
        Self { file, line, node }
    }

    #[must_use]
    pub fn file_line(file: impl Into<String>, line: usize) -> Self {
        Self {
            file: Some(file.into()),
            line: Some(line),
            node: None,
        }
    }

    #[must_use]
    pub fn full(file: impl Into<String>, line: usize, node: impl Into<String>) -> Self {
        Self {
            file: Some(file.into()),
            line: Some(line),
            node: Some(node.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// AgmError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("[{code}] {severity}: {message}")]
pub struct AgmError {
    pub code: ErrorCode,
    pub severity: Severity,
    pub message: String,
    pub location: ErrorLocation,
}

impl AgmError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>, location: ErrorLocation) -> Self {
        Self {
            severity: code.default_severity(),
            code,
            message: message.into(),
            location,
        }
    }

    #[must_use]
    pub fn with_severity(
        code: ErrorCode,
        severity: Severity,
        message: impl Into<String>,
        location: ErrorLocation,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            location,
        }
    }

    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    #[must_use]
    pub fn is_warning(&self) -> bool {
        self.severity == Severity::Warning
    }

    #[must_use]
    pub fn is_info(&self) -> bool {
        self.severity == Severity::Info
    }
}

impl Diagnostic for AgmError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(self.code))
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(self.severity.to_miette())
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        None
    }
}

// ---------------------------------------------------------------------------
// SourcedAgmError (internal)
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
#[error("{inner}")]
struct SourcedAgmError {
    inner: AgmError,
    source_code: NamedSource<String>,
    label_span: SourceSpan,
}

impl Diagnostic for SourcedAgmError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.inner.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.inner.severity()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source_code)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.inner.message.clone()),
            self.label_span,
        ))))
    }
}

// ---------------------------------------------------------------------------
// DiagnosticCollection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DiagnosticCollection {
    diagnostics: Vec<AgmError>,
    source_code: String,
    file_name: String,
}

impl DiagnosticCollection {
    #[must_use]
    pub fn new(file_name: impl Into<String>, source_code: impl Into<String>) -> Self {
        Self {
            diagnostics: Vec::new(),
            source_code: source_code.into(),
            file_name: file_name.into(),
        }
    }

    pub fn push(&mut self, error: AgmError) {
        self.diagnostics.push(error);
    }

    pub fn extend(&mut self, errors: impl IntoIterator<Item = AgmError>) {
        self.diagnostics.extend(errors);
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[AgmError] {
        &self.diagnostics
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<AgmError> {
        self.diagnostics
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_warning()).count()
    }

    #[must_use]
    pub fn info_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_info()).count()
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.source_code
    }

    fn line_byte_offset(&self, line: usize) -> Option<usize> {
        if line == 0 {
            return None;
        }
        let mut current_line = 1usize;
        if current_line == line {
            return Some(0);
        }
        for (offset, ch) in self.source_code.char_indices() {
            if ch == '\n' {
                current_line += 1;
                if current_line == line {
                    return Some(offset + 1);
                }
            }
        }
        None
    }

    fn line_byte_len(&self, line: usize) -> usize {
        if let Some(start) = self.line_byte_offset(line) {
            let rest = &self.source_code[start..];
            rest.find('\n').unwrap_or(rest.len())
        } else {
            0
        }
    }

    #[must_use]
    pub fn render_miette(&self) -> String {
        use miette::GraphicalReportHandler;

        let handler = GraphicalReportHandler::new();
        let mut output = String::new();

        for diag in &self.diagnostics {
            let sourced = self.wrap_with_source(diag);
            let _ = handler.render_report(&mut output, sourced.as_ref());
            output.push('\n');
        }

        output
    }

    fn wrap_with_source(&self, error: &AgmError) -> Box<dyn Diagnostic + '_> {
        let (offset, len) = if let Some(line) = error.location.line {
            let off = self.line_byte_offset(line).unwrap_or(0);
            let length = self.line_byte_len(line);
            (off, length)
        } else {
            (0, 0)
        };

        Box::new(SourcedAgmError {
            inner: error.clone(),
            source_code: NamedSource::new(self.file_name.clone(), self.source_code.clone()),
            label_span: SourceSpan::new(offset.into(), len),
        })
    }
}

impl fmt::Display for DiagnosticCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} error(s), {} warning(s), {} info(s)",
            self.file_name,
            self.error_count(),
            self.warning_count(),
            self.info_count(),
        )
    }
}

impl std::error::Error for DiagnosticCollection {}

impl Diagnostic for DiagnosticCollection {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        None
    }

    fn severity(&self) -> Option<miette::Severity> {
        if self.has_errors() {
            Some(miette::Severity::Error)
        } else if self.warning_count() > 0 {
            Some(miette::Severity::Warning)
        } else if self.info_count() > 0 {
            Some(miette::Severity::Advice)
        } else {
            None
        }
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        if self.diagnostics.is_empty() {
            return None;
        }
        Some(Box::new(
            self.diagnostics.iter().map(|d| d as &dyn Diagnostic),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes::ErrorCode;

    fn sample_error() -> AgmError {
        AgmError::new(
            ErrorCode::V003,
            "Duplicate node ID: `auth.login`".to_string(),
            ErrorLocation::full("file.agm", 42, "auth.login"),
        )
    }

    fn sample_warning() -> AgmError {
        AgmError::new(
            ErrorCode::V010,
            "Node type `workflow` typically includes field `steps` (missing)".to_string(),
            ErrorLocation::full("file.agm", 87, "deploy.step3"),
        )
    }

    fn sample_info() -> AgmError {
        AgmError::new(
            ErrorCode::P010,
            "File spec version `2.0` newer than parser version `1.0`".to_string(),
            ErrorLocation::file_line("file.agm", 1),
        )
    }

    #[test]
    fn test_severity_display_lowercase() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Info.to_string(), "info");
    }

    #[test]
    fn test_severity_to_miette_mapping() {
        assert_eq!(Severity::Error.to_miette(), miette::Severity::Error);
        assert_eq!(Severity::Warning.to_miette(), miette::Severity::Warning);
        assert_eq!(Severity::Info.to_miette(), miette::Severity::Advice);
    }

    #[test]
    fn test_error_location_full_sets_all_fields() {
        let loc = ErrorLocation::full("test.agm", 10, "node.id");
        assert_eq!(loc.file.as_deref(), Some("test.agm"));
        assert_eq!(loc.line, Some(10));
        assert_eq!(loc.node.as_deref(), Some("node.id"));
    }

    #[test]
    fn test_error_location_file_line_no_node() {
        let loc = ErrorLocation::file_line("test.agm", 5);
        assert_eq!(loc.file.as_deref(), Some("test.agm"));
        assert_eq!(loc.line, Some(5));
        assert_eq!(loc.node, None);
    }

    #[test]
    fn test_error_location_default_all_none() {
        let loc = ErrorLocation::default();
        assert_eq!(loc.file, None);
        assert_eq!(loc.line, None);
        assert_eq!(loc.node, None);
    }

    #[test]
    fn test_agm_error_new_uses_default_severity() {
        let err = AgmError::new(ErrorCode::V003, "test", ErrorLocation::default());
        assert_eq!(err.severity, Severity::Error);
        assert_eq!(err.code, ErrorCode::V003);
    }

    #[test]
    fn test_agm_error_with_severity_overrides_default() {
        let err = AgmError::with_severity(
            ErrorCode::V003,
            Severity::Warning,
            "test",
            ErrorLocation::default(),
        );
        assert_eq!(err.severity, Severity::Warning);
    }

    #[test]
    fn test_agm_error_is_error_true_for_errors() {
        let err = sample_error();
        assert!(err.is_error());
        assert!(!err.is_warning());
        assert!(!err.is_info());
    }

    #[test]
    fn test_agm_error_is_warning_true_for_warnings() {
        let warn = sample_warning();
        assert!(!warn.is_error());
        assert!(warn.is_warning());
        assert!(!warn.is_info());
    }

    #[test]
    fn test_agm_error_is_info_true_for_info() {
        let info = sample_info();
        assert!(!info.is_error());
        assert!(!info.is_warning());
        assert!(info.is_info());
    }

    #[test]
    fn test_agm_error_display_format() {
        let err = sample_error();
        let display = err.to_string();
        assert_eq!(display, "[AGM-V003] error: Duplicate node ID: `auth.login`");
    }

    #[test]
    fn test_agm_error_display_warning_format() {
        let warn = sample_warning();
        let display = warn.to_string();
        assert!(display.starts_with("[AGM-V010] warning:"));
    }

    #[test]
    fn test_agm_error_diagnostic_code() {
        let err = sample_error();
        let code = Diagnostic::code(&err).unwrap();
        assert_eq!(code.to_string(), "AGM-V003");
    }

    #[test]
    fn test_agm_error_diagnostic_severity() {
        let err = sample_error();
        assert_eq!(Diagnostic::severity(&err), Some(miette::Severity::Error));
    }

    #[test]
    fn test_collection_empty_has_no_errors() {
        let coll = DiagnosticCollection::new("test.agm", "");
        assert!(!coll.has_errors());
        assert!(coll.is_empty());
        assert_eq!(coll.len(), 0);
        assert_eq!(coll.error_count(), 0);
        assert_eq!(coll.warning_count(), 0);
        assert_eq!(coll.info_count(), 0);
    }

    #[test]
    fn test_collection_has_errors_with_error() {
        let mut coll = DiagnosticCollection::new("test.agm", "");
        coll.push(sample_error());
        assert!(coll.has_errors());
        assert_eq!(coll.error_count(), 1);
    }

    #[test]
    fn test_collection_has_errors_false_with_only_warnings() {
        let mut coll = DiagnosticCollection::new("test.agm", "");
        coll.push(sample_warning());
        assert!(!coll.has_errors());
        assert_eq!(coll.warning_count(), 1);
        assert_eq!(coll.error_count(), 0);
    }

    #[test]
    fn test_collection_mixed_counts() {
        let mut coll = DiagnosticCollection::new("test.agm", "");
        coll.push(sample_error());
        coll.push(sample_warning());
        coll.push(sample_info());
        assert_eq!(coll.len(), 3);
        assert_eq!(coll.error_count(), 1);
        assert_eq!(coll.warning_count(), 1);
        assert_eq!(coll.info_count(), 1);
        assert!(coll.has_errors());
    }

    #[test]
    fn test_collection_extend_adds_multiple() {
        let mut coll = DiagnosticCollection::new("test.agm", "");
        coll.extend(vec![sample_error(), sample_warning()]);
        assert_eq!(coll.len(), 2);
    }

    #[test]
    fn test_collection_into_diagnostics_returns_vec() {
        let mut coll = DiagnosticCollection::new("test.agm", "");
        coll.push(sample_error());
        let diags = coll.into_diagnostics();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, ErrorCode::V003);
    }

    #[test]
    fn test_collection_display_format() {
        let mut coll = DiagnosticCollection::new("test.agm", "");
        coll.push(sample_error());
        coll.push(sample_warning());
        let display = coll.to_string();
        assert_eq!(display, "test.agm: 1 error(s), 1 warning(s), 0 info(s)");
    }

    #[test]
    fn test_collection_diagnostic_severity_worst() {
        let mut coll = DiagnosticCollection::new("test.agm", "");
        coll.push(sample_warning());
        assert_eq!(Diagnostic::severity(&coll), Some(miette::Severity::Warning));
        coll.push(sample_error());
        assert_eq!(Diagnostic::severity(&coll), Some(miette::Severity::Error));
    }

    #[test]
    fn test_collection_line_byte_offset_line_1() {
        let coll = DiagnosticCollection::new("test.agm", "hello\nworld\n");
        assert_eq!(coll.line_byte_offset(1), Some(0));
        assert_eq!(coll.line_byte_offset(2), Some(6));
    }

    #[test]
    fn test_collection_line_byte_offset_zero_returns_none() {
        let coll = DiagnosticCollection::new("test.agm", "hello");
        assert_eq!(coll.line_byte_offset(0), None);
    }

    #[test]
    fn test_collection_line_byte_offset_beyond_end_returns_none() {
        let coll = DiagnosticCollection::new("test.agm", "hello");
        assert_eq!(coll.line_byte_offset(2), None);
    }

    #[test]
    fn test_collection_line_byte_len() {
        let coll = DiagnosticCollection::new("test.agm", "hello\nworld\n");
        assert_eq!(coll.line_byte_len(1), 5);
        assert_eq!(coll.line_byte_len(2), 5);
    }

    #[test]
    fn test_collection_render_miette_produces_output() {
        let source = "agm: 1\npackage: test\nversion: 0.1.0\n\nnode auth.login\ntype: facts\nsummary: first\n\nnode auth.login\ntype: facts\nsummary: duplicate\n";
        let mut coll = DiagnosticCollection::new("file.agm", source);
        coll.push(AgmError::new(
            ErrorCode::V003,
            "Duplicate node ID: `auth.login`",
            ErrorLocation::full("file.agm", 9, "auth.login"),
        ));
        let rendered = coll.render_miette();
        assert!(rendered.contains("AGM-V003"));
        assert!(rendered.contains("Duplicate node ID"));
    }
}
