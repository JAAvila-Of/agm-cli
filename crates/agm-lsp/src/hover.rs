// crates/agm-lsp/src/hover.rs

use crate::document::DocumentState;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

#[must_use]
pub fn provide_hover(state: &DocumentState, position: Position) -> Option<Hover> {
    let line_idx = position.line as usize;
    let col = position.character as usize;
    let line_text = state.source.lines().nth(line_idx)?;

    let word = extract_node_id_at(line_text, col)?;
    let node = state.find_node(word)?;

    let content = format!("**{}** (`{}`)\n\n{}", node.id, node.node_type, node.summary);

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: None,
    })
}

/// Extract a potential node ID at the given column position in the line.
/// Node IDs consist of `[a-z0-9_.]` characters.
pub fn extract_node_id_at(line: &str, col: usize) -> Option<&str> {
    if col >= line.len() {
        return None;
    }

    let bytes = line.as_bytes();

    if !is_node_id_char(bytes[col]) {
        return None;
    }

    let start = (0..=col)
        .rev()
        .take_while(|&i| is_node_id_char(bytes[i]))
        .last()?;

    let end = (col..line.len())
        .take_while(|&i| is_node_id_char(bytes[i]))
        .last()
        .map(|i| i + 1)?;

    let candidate = &line[start..end];
    if candidate.is_empty() {
        return None;
    }
    Some(candidate)
}

fn is_node_id_char(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.'
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

    #[test]
    fn test_hover_on_known_node_id_returns_summary() {
        let state = make_state();
        // Line 11 is "depends: [auth.login]"
        // "auth.login" starts at col 10
        let hover = provide_hover(&state, Position::new(11, 10));
        assert!(hover.is_some());
        let hover = hover.unwrap();
        if let HoverContents::Markup(mc) = hover.contents {
            assert!(mc.value.contains("Login facts"));
            assert!(mc.value.contains("auth.login"));
        } else {
            panic!("expected markup content");
        }
    }

    #[test]
    fn test_hover_on_unknown_id_returns_none() {
        let state = make_state();
        // Line 11 "depends: [auth.login]", col 0 is 'd' which is part of "depends"
        // "depends" is not a node ID — find_node returns None
        let hover = provide_hover(&state, Position::new(11, 0));
        assert!(hover.is_none());
    }

    #[test]
    fn test_hover_on_non_id_text_returns_none() {
        let state = make_state();
        // Line 5 is "type: facts", col 0 is 't' of "type"
        // "type" has uppercase-like chars? No, it's all lowercase ascii. But
        // "type" is not a node ID in the document.
        let hover = provide_hover(&state, Position::new(5, 0));
        assert!(hover.is_none());
    }

    #[test]
    fn test_extract_node_id_at_middle_of_id() {
        let line = "depends: [auth.login]";
        // col 14 is in the middle of "auth.login" (after the dot)
        let result = extract_node_id_at(line, 14);
        assert_eq!(result, Some("auth.login"));
    }

    #[test]
    fn test_extract_node_id_at_start_of_id() {
        let line = "depends: [auth.login]";
        // col 10 is 'a' of "auth.login"
        let result = extract_node_id_at(line, 10);
        assert_eq!(result, Some("auth.login"));
    }

    #[test]
    fn test_extract_node_id_at_end_of_line_returns_none() {
        let line = "depends: [auth.login]";
        // col past end
        let result = extract_node_id_at(line, line.len());
        assert!(result.is_none());
    }
}
