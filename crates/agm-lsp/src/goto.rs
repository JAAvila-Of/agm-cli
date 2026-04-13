// crates/agm-lsp/src/goto.rs

use crate::document::DocumentState;
use crate::hover::extract_node_id_at;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position, Range, Url};

#[must_use]
pub fn goto_definition(
    state: &DocumentState,
    position: Position,
    uri: Url,
) -> Option<GotoDefinitionResponse> {
    let line_idx = position.line as usize;
    let col = position.character as usize;
    let line_text = state.source.lines().nth(line_idx)?;

    let node_id = extract_node_id_at(line_text, col)?;
    let span = state.node_index.get(node_id)?;

    // Convert 1-indexed start_line to 0-indexed LSP line
    let target_line = (span.start.saturating_sub(1)) as u32;

    Some(GotoDefinitionResponse::Scalar(Location {
        uri,
        range: Range {
            start: Position::new(target_line, 0),
            end: Position::new(target_line, 0),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentState;

    const SRC: &str = "\
agm: 1.0
package: test
version: 0.1.0

node auth.login
type: facts
summary: Login facts

node auth.logout
type: facts
summary: Logout facts
depends: [auth.login]
";

    fn make_state() -> DocumentState {
        DocumentState::from_source(SRC, "test.agm")
    }

    fn make_uri() -> Url {
        Url::parse("file:///test.agm").unwrap()
    }

    #[test]
    fn test_goto_known_node_returns_correct_line() {
        let state = make_state();
        // Line 11 is "depends: [auth.login]", col 10 is 'a' of "auth.login"
        // auth.login node declaration is at line 5 (1-indexed) -> LSP line 4 (0-indexed)
        let result = goto_definition(&state, Position::new(11, 10), make_uri());
        assert!(result.is_some());
        if let Some(GotoDefinitionResponse::Scalar(loc)) = result {
            assert_eq!(loc.range.start.line, 4);
        } else {
            panic!("expected scalar location");
        }
    }

    #[test]
    fn test_goto_unknown_node_returns_none() {
        let state = make_state();
        // Line 0 is "agm: 1" — "agm" is not a node ID
        let result = goto_definition(&state, Position::new(0, 0), make_uri());
        assert!(result.is_none());
    }

    #[test]
    fn test_goto_same_node_declaration_returns_self() {
        let state = make_state();
        // "node auth.login" is on line 4 (0-indexed), col 5 is 'a' of "auth.login"
        // But "node " prefix: "node auth.login" — 'n' at 0, 'a' at 5
        // extract_node_id_at on "node auth.login" at col 5 -> "auth.login"
        let result = goto_definition(&state, Position::new(4, 5), make_uri());
        assert!(result.is_some());
        if let Some(GotoDefinitionResponse::Scalar(loc)) = result {
            // Should point to itself: line 4 (0-indexed)
            assert_eq!(loc.range.start.line, 4);
        } else {
            panic!("expected scalar location");
        }
    }
}
