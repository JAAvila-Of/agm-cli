// crates/agm-lsp/src/symbols.rs

use crate::document::DocumentState;
use tower_lsp::lsp_types::{DocumentSymbol, DocumentSymbolResponse, Position, Range, SymbolKind};

#[must_use]
pub fn document_symbols(state: &DocumentState) -> DocumentSymbolResponse {
    let symbols = state
        .file
        .as_ref()
        .map(|f| {
            f.nodes
                .iter()
                .map(|node| {
                    let start_line = node.span.start_line.saturating_sub(1) as u32;
                    let end_line = node.span.end_line.saturating_sub(1) as u32;

                    #[allow(deprecated)]
                    DocumentSymbol {
                        name: node.id.clone(),
                        detail: Some(format!("{}", node.node_type)),
                        kind: SymbolKind::OBJECT,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position::new(start_line, 0),
                            end: Position::new(end_line, u32::MAX),
                        },
                        selection_range: Range {
                            start: Position::new(start_line, 0),
                            end: Position::new(start_line, (5 + node.id.len()) as u32),
                        },
                        children: None,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    DocumentSymbolResponse::Nested(symbols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentState;

    const THREE_NODE_SRC: &str = "\
agm: 1.0
package: test
version: 0.1.0

node auth.login
type: facts
summary: Login facts

node auth.logout
type: facts
summary: Logout facts

node auth.session
type: workflow
summary: Session workflow
";

    fn make_state(src: &str) -> DocumentState {
        DocumentState::from_source(src, "test.agm")
    }

    #[test]
    fn test_document_symbols_lists_all_nodes() {
        let state = make_state(THREE_NODE_SRC);
        let response = document_symbols(&state);
        if let DocumentSymbolResponse::Nested(syms) = response {
            assert_eq!(syms.len(), 3);
        } else {
            panic!("expected nested response");
        }
    }

    #[test]
    fn test_document_symbols_names_are_node_ids() {
        let state = make_state(THREE_NODE_SRC);
        let response = document_symbols(&state);
        if let DocumentSymbolResponse::Nested(syms) = response {
            let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
            assert!(names.contains(&"auth.login"));
            assert!(names.contains(&"auth.logout"));
            assert!(names.contains(&"auth.session"));
        } else {
            panic!("expected nested response");
        }
    }

    #[test]
    fn test_document_symbols_detail_is_node_type() {
        let state = make_state(THREE_NODE_SRC);
        let response = document_symbols(&state);
        if let DocumentSymbolResponse::Nested(syms) = response {
            let session = syms.iter().find(|s| s.name == "auth.session").unwrap();
            assert_eq!(session.detail.as_deref(), Some("workflow"));
        } else {
            panic!("expected nested response");
        }
    }

    #[test]
    fn test_document_symbols_empty_file_returns_empty() {
        // Parse failure -> file is None -> empty symbols
        let state = DocumentState::from_source("", "test.agm");
        let response = document_symbols(&state);
        if let DocumentSymbolResponse::Nested(syms) = response {
            assert!(syms.is_empty());
        } else {
            panic!("expected nested response");
        }
    }

    #[test]
    fn test_document_symbols_range_matches_span() {
        let state = make_state(THREE_NODE_SRC);
        let response = document_symbols(&state);
        if let DocumentSymbolResponse::Nested(syms) = response {
            let login = syms.iter().find(|s| s.name == "auth.login").unwrap();
            // auth.login node starts at line 5 (1-indexed) -> LSP line 4 (0-indexed)
            assert_eq!(login.range.start.line, 4);
        } else {
            panic!("expected nested response");
        }
    }
}
