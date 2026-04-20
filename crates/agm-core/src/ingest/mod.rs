//! Ingest: turn tool-call JSON args into validated [`Node`] values.
//!
//! The primary entry points are:
//! - [`ingest_one`] — ingest a single JSON object into a `Node`.
//! - [`ingest_many`] — ingest a JSON array into `Vec<Node>`.
//!
//! # Pipeline
//!
//! ```text
//! JSON value
//!   ↓ schema_check (optional)
//!   ↓ *_from_json builder helper  (recognizes known fields, stores unknowns in extra_fields)
//!   ↓ builder.build_with(Permissive)  (constructs Node without cross-ref checks)
//!   ↓ normalize pass (optional, via normalize_ast on scratch AgmFile)
//!   → Node
//! ```
//!
//! Full Standard/Strict validation is intentionally **deferred** to the caller
//! so that single-node cross-reference errors (V004, V005, etc.) are not raised
//! in isolation. The `agm ingest` CLI command runs a final multi-node validation
//! pass after assembling the complete `AgmFile`.

pub mod from_json;
mod many;
mod one;
mod schema_check;

pub use many::ingest_many;
pub use one::ingest_one;

use crate::builder::BuildError;
use crate::model::schema::EnforcementLevel;

// ---------------------------------------------------------------------------
// IngestConfig
// ---------------------------------------------------------------------------

/// Configuration for ingest.
#[derive(Debug, Clone)]
pub struct IngestConfig {
    /// Normalize field and type names before building. Default: true.
    pub normalize: bool,
    /// Run JSON Schema validation before building. Default: true.
    pub schema_check: bool,
    /// Enforcement level used for the final single-node build. Default: Standard.
    pub enforcement: EnforcementLevel,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            normalize: true,
            schema_check: true,
            enforcement: EnforcementLevel::Standard,
        }
    }
}

// ---------------------------------------------------------------------------
// IngestError
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("input is not a JSON object")]
    NotAnObject,
    #[error("schema check failed: {0}")]
    SchemaCheck(String),
    #[error("node id is missing and no default was provided")]
    MissingId,
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error("JSON parse error: {0}")]
    JsonParse(String),
    #[error("batch ingest failed ({} error(s))", .errors.len())]
    Batch { errors: Vec<(usize, IngestError)> },
}
