//! Renderer: converts AST (or filtered views) to output formats.
//!
//! Supported formats: JSON, JSON canonical (S37), Markdown, canonical AGM
//! text, and graph visualization (DOT/Mermaid).

pub mod canonical;
pub mod graph;
pub mod json;
pub mod json_canonical;
pub mod markdown;
pub mod mem;
pub mod state;

use crate::model::file::AgmFile;

// ---------------------------------------------------------------------------
// RenderError
// ---------------------------------------------------------------------------

/// Errors produced by the fallible [`json_canonical::json_to_agm`] function.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RenderError {
    #[error("missing required field: {field}")]
    MissingField { field: String },

    #[error("invalid field type for '{field}': expected {expected}, got {actual}")]
    InvalidType {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("invalid enum value for '{field}': {value}")]
    InvalidEnumValue { field: String, value: String },

    #[error("JSON deserialization error: {0}")]
    JsonError(String),
}

// ---------------------------------------------------------------------------
// RenderFormat
// ---------------------------------------------------------------------------

/// Output format selector for the top-level [`render`] function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFormat {
    Json,
    JsonCanonical,
    Markdown,
    Canonical,
    Dot,
    Mermaid,
}

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

/// Renders an `AgmFile` in the requested format.
///
/// All variants are infallible — they operate on a valid AST and produce
/// best-effort output.
#[must_use]
pub fn render(file: &AgmFile, format: RenderFormat) -> String {
    match format {
        RenderFormat::Json => json::render_json(file),
        RenderFormat::JsonCanonical => json_canonical::render_json_canonical(file),
        RenderFormat::Markdown => markdown::render_markdown(file),
        RenderFormat::Canonical => canonical::render_canonical(file),
        RenderFormat::Dot => graph::render_graph_dot(file),
        RenderFormat::Mermaid => graph::render_graph_mermaid(file),
    }
}
