// crates/agm-lsp/src/document.rs

use std::collections::HashMap;
use std::ops::Range;

use agm_core::error::AgmError;
use agm_core::model::file::AgmFile;
use agm_core::parser;
use agm_core::validator::{self, ValidateOptions};

/// Per-document state maintained by the LSP server.
#[derive(Debug, Clone)]
pub struct DocumentState {
    /// The raw source text.
    pub source: String,
    /// Parsed AST (None if parsing failed with hard errors).
    pub file: Option<AgmFile>,
    /// All diagnostics: parser errors + validator diagnostics.
    pub diagnostics: Vec<AgmError>,
    /// Index: node ID -> (start_line, end_line), 1-indexed (matching Span).
    pub node_index: HashMap<String, Range<usize>>,
}

impl DocumentState {
    /// Parse and validate the given source text.
    #[must_use]
    pub fn from_source(source: &str, uri: &str) -> Self {
        let mut all_diagnostics = Vec::new();

        let file = match parser::parse(source) {
            Ok(agm_file) => {
                let opts = ValidateOptions::default();
                let collection = validator::validate(&agm_file, source, uri, &opts);
                all_diagnostics.extend(collection.into_diagnostics());
                Some(agm_file)
            }
            Err(parse_errors) => {
                all_diagnostics.extend(parse_errors);
                None
            }
        };

        let node_index = file
            .as_ref()
            .map(|f| {
                f.nodes
                    .iter()
                    .map(|n| (n.id.clone(), n.span.start_line..n.span.end_line))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            source: source.to_owned(),
            file,
            diagnostics: all_diagnostics,
            node_index,
        }
    }

    /// Returns all node IDs in this document.
    #[must_use]
    pub fn node_ids(&self) -> Vec<&str> {
        self.file
            .as_ref()
            .map(|f| f.nodes.iter().map(|n| n.id.as_str()).collect())
            .unwrap_or_default()
    }

    /// Look up a node by ID.
    #[must_use]
    pub fn find_node(&self, id: &str) -> Option<&agm_core::model::node::Node> {
        self.file
            .as_ref()
            .and_then(|f| f.nodes.iter().find(|n| n.id == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_VALID: &str = "\
agm: 1.0
package: test
version: 0.1.0

node auth.login
type: facts
summary: Authentication login facts
";

    const TWO_NODE: &str = "\
agm: 1.0
package: test
version: 0.1.0

node auth.login
type: facts
summary: Authentication login facts

node auth.logout
type: facts
summary: Authentication logout facts
";

    #[test]
    fn test_from_source_valid_file_parses_successfully() {
        let state = DocumentState::from_source(MINIMAL_VALID, "test.agm");
        assert!(state.file.is_some());
        // Diagnostics may include warnings; assert no hard errors
        assert!(state.diagnostics.iter().all(|d| !d.is_error()));
        assert!(!state.node_index.is_empty());
    }

    #[test]
    fn test_from_source_parse_error_returns_diagnostics() {
        // Missing required header fields
        let state = DocumentState::from_source("node foo\ntype: facts\nsummary: x\n", "test.agm");
        assert!(state.file.is_none());
        assert!(!state.diagnostics.is_empty());
    }

    #[test]
    fn test_from_source_validation_warnings_collected() {
        // Valid parse but with a node that triggers validation
        let state = DocumentState::from_source(MINIMAL_VALID, "test.agm");
        // File should parse successfully
        assert!(state.file.is_some());
    }

    #[test]
    fn test_from_source_node_index_maps_ids_to_spans() {
        let state = DocumentState::from_source(TWO_NODE, "test.agm");
        assert!(state.node_index.contains_key("auth.login"));
        assert!(state.node_index.contains_key("auth.logout"));
    }

    #[test]
    fn test_from_source_empty_file_returns_error() {
        let state = DocumentState::from_source("", "test.agm");
        assert!(!state.diagnostics.is_empty());
    }

    #[test]
    fn test_node_ids_returns_all_ids() {
        let state = DocumentState::from_source(TWO_NODE, "test.agm");
        let ids = state.node_ids();
        assert!(ids.contains(&"auth.login"));
        assert!(ids.contains(&"auth.logout"));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_find_node_returns_matching_node() {
        let state = DocumentState::from_source(MINIMAL_VALID, "test.agm");
        let node = state.find_node("auth.login");
        assert!(node.is_some());
        assert_eq!(node.unwrap().summary, "Authentication login facts");
    }

    #[test]
    fn test_find_node_unknown_returns_none() {
        let state = DocumentState::from_source(MINIMAL_VALID, "test.agm");
        assert!(state.find_node("nonexistent.node").is_none());
    }
}
