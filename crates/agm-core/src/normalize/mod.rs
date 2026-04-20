//! Normalization layer: rewrites non-canonical field and type names to their
//! canonical forms per spec v1.2.0.
//!
//! The normalizer operates strictly on field names and node types. It does
//! **not** guess semantics, invent values, or autocorrect grammar. If a field
//! is ambiguous or the rule collides with a canonical form already present,
//! the normalizer emits a warning and preserves the canonical version.

pub mod engine;
pub mod report;
pub mod rules;

pub use report::{NormalizeReport, NormalizeWarning, NormalizeWarningCode, Rewrite};
pub use rules::{RuleParseError, RuleSet};

use crate::error::diagnostic::AgmError;
use crate::model::file::AgmFile;

// ---------------------------------------------------------------------------
// Canonical node type list (used by the engine for UnknownNodeType warnings)
// ---------------------------------------------------------------------------

/// All canonical node type strings recognized by spec v1.2.0.
pub(crate) const CANONICAL_NODE_TYPES: &[&str] = &[
    "facts",
    "rules",
    "workflow",
    "entity",
    "decision",
    "exception",
    "example",
    "glossary",
    "anti_pattern",
    "orchestration",
    "ticket",
];

// ---------------------------------------------------------------------------
// NormalizeConfig
// ---------------------------------------------------------------------------

/// Configuration for the normalizer.
#[derive(Debug, Clone)]
pub struct NormalizeConfig {
    /// Rules to apply. Defaults to the built-in rule set.
    pub rules: RuleSet,
    /// When `true` (default), emits a warning when a synonym is found alongside
    /// its canonical field and preserves the canonical form.
    pub warn_on_collision: bool,
    /// When `true` (default), applies type-level synonyms (e.g. `plan_execution` → `orchestration`).
    pub normalize_types: bool,
    /// When `true` (default), applies field-level synonyms (e.g. `depends_on` → `depends`).
    pub normalize_fields: bool,
}

impl Default for NormalizeConfig {
    fn default() -> Self {
        Self {
            rules: RuleSet::builtin(),
            warn_on_collision: true,
            normalize_types: true,
            normalize_fields: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Normalizes raw AGM text. Runs the parser, applies AST-level rewrites,
/// re-emits canonical AGM, and returns the report.
///
/// Returns `Err` only on hard parse errors. Normalization itself never fails —
/// it emits warnings via the report.
///
/// # Errors
///
/// Returns `Err(Vec<AgmError>)` when the input cannot be parsed.
pub fn normalize_text(
    raw: &str,
    config: &NormalizeConfig,
) -> Result<(String, NormalizeReport), Vec<AgmError>> {
    let mut file: AgmFile = crate::parser::parse(raw)?;
    let report = engine::normalize_ast(&mut file, config);
    let canonical = crate::renderer::canonical::render_canonical(&file);
    Ok((canonical, report))
}

/// Normalizes an already-parsed `AgmFile` in place. Returns the report of rewrites.
/// Used by consumers who already have an AST (e.g. ingest, compile).
#[must_use]
pub fn normalize_ast(file: &mut AgmFile, config: &NormalizeConfig) -> NormalizeReport {
    engine::normalize_ast(file, config)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_AGM: &str = "\
agm: 1.0
package: test.pkg
version: 0.1.0

node n1
type: workflow
summary: a workflow node
depends_on: [dep.node]
";

    #[test]
    fn test_normalize_text_rewrites_synonym() {
        let config = NormalizeConfig::default();
        let (output, report) = normalize_text(MINIMAL_AGM, &config).expect("parse error");
        assert!(
            !report.rewrites.is_empty(),
            "expected a rewrite for depends_on"
        );
        assert!(
            output.contains("depends:"),
            "output should contain canonical 'depends:'"
        );
        assert!(
            !output.contains("depends_on:"),
            "output should not contain synonym"
        );
    }

    #[test]
    fn test_normalize_text_parse_error_returns_err() {
        let config = NormalizeConfig::default();
        let bad = "this is not agm";
        let result = normalize_text(bad, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_text_already_canonical_is_empty_report() {
        let canonical_agm = "\
agm: 1.0
package: test.pkg
version: 0.1.0

node n1
type: workflow
summary: a workflow node
depends: [dep.node]
";
        let config = NormalizeConfig::default();
        let (_, report) = normalize_text(canonical_agm, &config).expect("parse error");
        assert!(
            report.rewrites.is_empty(),
            "canonical input should produce no rewrites"
        );
    }

    #[test]
    fn test_default_config_uses_builtin_rules() {
        let config = NormalizeConfig::default();
        assert!(config.normalize_types);
        assert!(config.normalize_fields);
        assert!(config.warn_on_collision);
        // Should recognize at least one synonym
        assert!(config.rules.canonical_type("plan_execution").is_some());
    }
}
