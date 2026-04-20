//! Validator: checks a parsed AST for AGM spec compliance.
//!
//! Produces a `DiagnosticCollection` of errors and warnings covering file-level
//! checks, node-level checks, reference resolution, cycle detection, field
//! compatibility, code block validation, and more.
//!
//! # Validation passes
//!
//! 1. **File-level** (`file`): header required fields, at least one node.
//! 2. **Node-level** (`node`): ID pattern/uniqueness, summary, dates, status.
//! 3. **Structural per-node** (`code`, `verify`, `context`, `orchestration`,
//!    `execution`, `memory`): structured sub-field validation.
//! 4. **Type schema** (`type_schema`): per-type field schema enforcement.
//! 5. **Cross-node** (`references`, `cycles`, `compatibility`): reference
//!    resolution, cycle detection, conflict detection.
//! 6. **Cross-package** (`imports`): import resolution and cross-package refs.
//!    Only runs when `ValidateOptions.import_resolver` is `Some`.

use std::collections::HashSet;
use std::sync::Arc;

use crate::error::diagnostic::{AgmError, DiagnosticCollection};
use crate::import::ImportResolver;
use crate::model::file::AgmFile;
use crate::model::schema::EnforcementLevel;

pub mod code;
pub mod compatibility;
pub mod context;
pub mod cycles;
pub mod execution;
pub mod file;
pub mod imports;
pub mod memory;
pub mod node;
pub mod orchestration;
pub mod references;
pub mod ticket;
pub mod type_schema;
pub mod verify;

// ---------------------------------------------------------------------------
// ValidationScope
// ---------------------------------------------------------------------------

/// Controls which validation passes run.
///
/// - `File` (default): all passes, including cross-node reference resolution,
///   cycle detection, and orchestration cross-checks. Use when validating a
///   complete `AgmFile`. This is the scope used by `agm validate` / `agm lint`.
///
/// - `SingleNode`: skips cross-node checks (Pass 5 — references, cycles,
///   compatibility) and cross-node reference checks within Pass 3 (V004 for
///   `agent_context.load_nodes`, `verify.node_status`, and relationship fields).
///   Use when validating a node in isolation, e.g. inside a builder's `.build()`.
///
///   **Rationale**: a node under construction legitimately references nodes in
///   sibling files that don't exist in the scratch `AgmFile` created by the
///   builder. Cross-node checks would fire V004 spuriously. The full-file
///   validator enforces reference integrity when the node is assembled into a
///   real `AgmFile`.
///
///   Note: `SingleNode` does NOT bypass schema/type/format checks (Passes 1–4).
///   Those still run and still catch malformed nodes. The only skipped checks
///   are those that require the full node-set of a complete file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValidationScope {
    /// Validate all passes against a complete file. Default.
    #[default]
    File,
    /// Skip cross-node reference checks. Used by builder `.build()`.
    SingleNode,
}

// ---------------------------------------------------------------------------
// ValidateOptions
// ---------------------------------------------------------------------------

/// Options controlling validation behavior.
///
/// Note: `import_resolver` uses `Arc<dyn ImportResolver + Send + Sync>` to allow
/// the struct to be `Clone` while still supporting dynamic dispatch.
#[derive(Clone)]
pub struct ValidateOptions {
    /// Schema enforcement level applied in Pass 4.
    /// Default: `Standard`.
    pub enforcement_level: EnforcementLevel,
    /// Optional import resolver for cross-package validation (Pass 6).
    /// If `None`, import-related rules (I001–I005) are skipped.
    pub import_resolver: Option<Arc<dyn ImportResolver + Send + Sync>>,
    /// Validation scope: controls whether cross-node checks run.
    /// Default: `File` (all passes). Use `SingleNode` inside builder `.build()`.
    pub scope: ValidationScope,
}

impl std::fmt::Debug for ValidateOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidateOptions")
            .field("enforcement_level", &self.enforcement_level)
            .field("import_resolver", &self.import_resolver.is_some())
            .field("scope", &self.scope)
            .finish()
    }
}

impl Default for ValidateOptions {
    fn default() -> Self {
        Self {
            enforcement_level: EnforcementLevel::Standard,
            import_resolver: None,
            scope: ValidationScope::File,
        }
    }
}

// ---------------------------------------------------------------------------
// validate()
// ---------------------------------------------------------------------------

/// Validates a parsed AGM file against all spec rules.
///
/// Runs all validation passes in dependency order and returns a
/// `DiagnosticCollection` with all diagnostics sorted by line number
/// (ascending), then by severity (errors before warnings at the same line).
#[must_use]
pub fn validate(
    agm_file: &AgmFile,
    source: &str,
    file_name: &str,
    options: &ValidateOptions,
) -> DiagnosticCollection {
    let mut collection = DiagnosticCollection::new(file_name, source);
    let mut all_errors: Vec<AgmError> = Vec::new();

    // Pass 1: File-level checks
    all_errors.extend(file::validate_file(agm_file, file_name));

    // Pass 2: Node-level checks (also builds all_ids set for later passes)
    all_errors.extend(node::validate_node_ids(&agm_file.nodes, file_name));
    let all_ids: HashSet<String> = agm_file.nodes.iter().map(|n| n.id.clone()).collect();
    for n in &agm_file.nodes {
        all_errors.extend(node::validate_node(n, &all_ids, file_name));
    }

    // Build set of all memory topics declared across nodes (used in Pass 3)
    let all_memory_topics: HashSet<String> = agm_file
        .nodes
        .iter()
        .filter_map(|n| n.memory.as_ref())
        .flat_map(|entries| entries.iter().map(|e| e.topic.clone()))
        .collect();

    let single_node = options.scope == ValidationScope::SingleNode;

    // Pass 3: Structural per-node checks
    for n in &agm_file.nodes {
        all_errors.extend(code::validate_code(n, file_name));
        // In SingleNode scope skip cross-node ref checks in verify (node_status).
        all_errors.extend(verify::validate_verify(n, &all_ids, file_name, single_node));
        // In SingleNode scope skip V004 for load_nodes.
        all_errors.extend(context::validate_context(
            n,
            &all_ids,
            &all_memory_topics,
            file_name,
            single_node,
        ));
        // In SingleNode scope skip group-node cross-refs in orchestration.
        all_errors.extend(orchestration::validate_orchestration(
            n, &all_ids, file_name, single_node,
        ));
        all_errors.extend(execution::validate_execution(n, file_name));
        all_errors.extend(memory::validate_memory(n, file_name));
        all_errors.extend(ticket::validate_ticket(n, file_name));
    }

    // Pass 4: Type schema enforcement
    for n in &agm_file.nodes {
        all_errors.extend(type_schema::validate_type_schema(
            n,
            &options.enforcement_level,
            file_name,
        ));
    }

    // Pass 5: Cross-node checks — skipped in SingleNode scope.
    if !agm_file.nodes.is_empty() && !single_node {
        all_errors.extend(references::validate_references(
            agm_file, &all_ids, file_name,
        ));
        all_errors.extend(cycles::validate_cycles(agm_file, file_name));
        all_errors.extend(compatibility::validate_compatibility(agm_file, file_name));
    }

    // Pass 6: Cross-package checks (only runs with an import resolver)
    if let Some(ref resolver) = options.import_resolver {
        all_errors.extend(imports::validate_imports(
            agm_file,
            resolver.as_ref(),
            file_name,
        ));
    }

    // Sort: ascending line number, then errors before warnings before info
    sort_diagnostics(&mut all_errors);

    collection.extend(all_errors);
    collection
}

/// Sorts diagnostics by line number (ascending), then by severity
/// (errors before warnings before info at the same line number).
fn sort_diagnostics(errors: &mut [AgmError]) {
    errors.sort_by(|a, b| {
        let line_a = a.location.line.unwrap_or(0);
        let line_b = b.location.line.unwrap_or(0);

        line_a
            .cmp(&line_b)
            .then_with(|| severity_rank(a.severity).cmp(&severity_rank(b.severity)))
    });
}

/// Maps severity to a sort rank: Error=0 (highest), Warning=1, Info=2.
fn severity_rank(sev: crate::error::diagnostic::Severity) -> u8 {
    use crate::error::diagnostic::Severity;
    match sev {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes::ErrorCode;
    use crate::model::fields::{NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;

    fn valid_header() -> Header {
        Header {
            agm: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            title: None,
            owner: None,
            imports: None,
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        }
    }

    fn minimal_node(id: &str, line: usize) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: "a test node".to_owned(),
            span: Span::new(line, line + 2),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_valid_file_returns_no_errors() {
        let file = AgmFile {
            header: valid_header(),
            nodes: vec![minimal_node("auth.login", 5)],
        };
        let result = validate(&file, "", "test.agm", &Default::default());
        assert!(!result.has_errors(), "Valid file should produce no errors");
    }

    #[test]
    fn test_validate_default_options_uses_standard_level() {
        let opts = ValidateOptions::default();
        assert_eq!(opts.enforcement_level, EnforcementLevel::Standard);
        assert!(opts.import_resolver.is_none());
    }

    #[test]
    fn test_validate_multiple_errors_sorted_by_line() {
        // Node on line 20 has an unresolved dep; node on line 5 has too-long summary.
        let mut node_a = minimal_node("auth.a", 5);
        node_a.summary = "x".repeat(201); // V012 warning, line 5

        let mut node_b = minimal_node("auth.b", 20);
        node_b.depends = Some(vec!["missing.dep".to_owned()]); // V004 error, line 20

        let file = AgmFile {
            header: valid_header(),
            nodes: vec![node_a, node_b],
        };
        let result = validate(&file, "", "test.agm", &Default::default());
        let diags = result.diagnostics();
        // All diagnostics should be present
        assert!(!diags.is_empty());
        // Verify sort order: earlier lines should come first
        for i in 1..diags.len() {
            let prev_line = diags[i - 1].location.line.unwrap_or(0);
            let curr_line = diags[i].location.line.unwrap_or(0);
            assert!(
                prev_line <= curr_line,
                "Diagnostics not sorted by line: {} > {}",
                prev_line,
                curr_line
            );
        }
    }

    #[test]
    fn test_validate_empty_header_fields_produce_p001() {
        let mut file = AgmFile {
            header: valid_header(),
            nodes: vec![minimal_node("test.node", 5)],
        };
        file.header.agm = String::new();
        let result = validate(&file, "", "test.agm", &Default::default());
        assert!(
            result
                .diagnostics()
                .iter()
                .any(|d| d.code == ErrorCode::P001)
        );
    }

    #[test]
    fn test_validate_no_nodes_produces_p008() {
        let file = AgmFile {
            header: valid_header(),
            nodes: vec![],
        };
        let result = validate(&file, "", "test.agm", &Default::default());
        assert!(
            result
                .diagnostics()
                .iter()
                .any(|d| d.code == ErrorCode::P008)
        );
    }

    #[test]
    fn test_validate_options_clone() {
        let opts = ValidateOptions::default();
        let _cloned = opts.clone();
    }

    #[test]
    fn test_sort_diagnostics_orders_by_line_then_severity() {
        use crate::error::diagnostic::{AgmError, ErrorLocation, Severity};

        let mut errors = vec![
            AgmError::with_severity(
                ErrorCode::V012,
                Severity::Warning,
                "warn",
                ErrorLocation::file_line("f", 10),
            ),
            AgmError::with_severity(
                ErrorCode::V004,
                Severity::Error,
                "err",
                ErrorLocation::file_line("f", 10),
            ),
            AgmError::with_severity(
                ErrorCode::V003,
                Severity::Error,
                "earlier",
                ErrorLocation::file_line("f", 5),
            ),
        ];

        sort_diagnostics(&mut errors);

        // Line 5 error should be first
        assert_eq!(errors[0].location.line, Some(5));
        // Line 10 error before warning
        assert_eq!(errors[1].severity, Severity::Error);
        assert_eq!(errors[2].severity, Severity::Warning);
    }
}
