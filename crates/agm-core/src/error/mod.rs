//! Error model and diagnostics for AGM files (spec sections 21, Appendix D).

pub mod codes;
pub mod diagnostic;
pub mod output;

pub use codes::ErrorCode;
pub use diagnostic::{AgmError, DiagnosticCollection, ErrorLocation, Severity};
pub use output::{ErrorOutputFormat, format_error_json, format_error_text};
