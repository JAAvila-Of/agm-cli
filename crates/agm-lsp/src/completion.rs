// crates/agm-lsp/src/completion.rs

use crate::document::DocumentState;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Position};

// All known node field names (universal + type-specific).
const NODE_FIELD_NAMES: &[&str] = &[
    "type",
    "summary",
    "priority",
    "stability",
    "confidence",
    "status",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "items",
    "steps",
    "fields",
    "input",
    "output",
    "detail",
    "rationale",
    "tradeoffs",
    "resolution",
    "examples",
    "notes",
    "code",
    "code_blocks",
    "verify",
    "agent_context",
    "target",
    "execution_status",
    "executed_by",
    "executed_at",
    "execution_log",
    "retry_count",
    "parallel_groups",
    "memory",
    "scope",
    "applies_when",
    "valid_from",
    "valid_until",
    "tags",
    "aliases",
    "keywords",
];

const HEADER_FIELD_NAMES: &[&str] = &[
    "agm",
    "package",
    "version",
    "title",
    "owner",
    "imports",
    "default_load",
    "description",
    "tags",
    "status",
    "load_profiles",
    "target_runtime",
];

const NODE_TYPES: &[&str] = &[
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
];

const PRIORITIES: &[&str] = &["critical", "high", "normal", "low"];
const STABILITIES: &[&str] = &["high", "medium", "low", "volatile"];
const CONFIDENCES: &[&str] = &["high", "medium", "low", "inferred", "tentative"];
const NODE_STATUSES: &[&str] = &["active", "draft", "deprecated", "superseded"];
const EXECUTION_STATUSES: &[&str] = &[
    "pending",
    "ready",
    "in_progress",
    "completed",
    "failed",
    "blocked",
    "skipped",
];

enum CompletionContext {
    FieldName { in_header: bool },
    NodeTypeValue,
    PriorityValue,
    StabilityValue,
    ConfidenceValue,
    StatusValue,
    ExecutionStatusValue,
    NodeIdReference,
    Unknown,
}

fn detect_context(
    line_text: &str,
    position: Position,
    is_before_first_node: bool,
) -> CompletionContext {
    let trimmed = line_text.trim();

    if trimmed.is_empty() || !trimmed.contains(':') {
        return CompletionContext::FieldName {
            in_header: is_before_first_node,
        };
    }

    if let Some(colon_pos) = trimmed.find(':') {
        let field_name = trimmed[..colon_pos].trim();
        let cursor_col = position.character as usize;
        let line_colon_col = line_text.find(':').unwrap_or(0);
        if cursor_col > line_colon_col {
            return match field_name {
                "type" => CompletionContext::NodeTypeValue,
                "priority" => CompletionContext::PriorityValue,
                "stability" => CompletionContext::StabilityValue,
                "confidence" => CompletionContext::ConfidenceValue,
                "status" => CompletionContext::StatusValue,
                "execution_status" => CompletionContext::ExecutionStatusValue,
                "depends" | "related_to" | "replaces" | "conflicts" | "see_also" => {
                    CompletionContext::NodeIdReference
                }
                _ => CompletionContext::Unknown,
            };
        }
    }

    CompletionContext::FieldName {
        in_header: is_before_first_node,
    }
}

#[must_use]
pub fn provide_completions(
    state: &DocumentState,
    position: Position,
) -> Option<Vec<CompletionItem>> {
    let line_idx = position.line as usize;
    let line_text = state.source.lines().nth(line_idx)?;

    let first_node_line = state
        .file
        .as_ref()
        .and_then(|f| f.nodes.first().map(|n| n.span.start_line.saturating_sub(1)));
    let is_before_first_node = first_node_line
        .map(|first| line_idx < first)
        .unwrap_or(true);

    let context = detect_context(line_text, position, is_before_first_node);

    let items = match context {
        CompletionContext::FieldName { in_header } => {
            let names = if in_header {
                HEADER_FIELD_NAMES
            } else {
                NODE_FIELD_NAMES
            };
            names
                .iter()
                .map(|name| CompletionItem {
                    label: format!("{name}:"),
                    kind: Some(CompletionItemKind::FIELD),
                    insert_text: Some(format!("{name}: ")),
                    ..Default::default()
                })
                .collect()
        }
        CompletionContext::NodeTypeValue => make_enum_completions(NODE_TYPES),
        CompletionContext::PriorityValue => make_enum_completions(PRIORITIES),
        CompletionContext::StabilityValue => make_enum_completions(STABILITIES),
        CompletionContext::ConfidenceValue => make_enum_completions(CONFIDENCES),
        CompletionContext::StatusValue => make_enum_completions(NODE_STATUSES),
        CompletionContext::ExecutionStatusValue => make_enum_completions(EXECUTION_STATUSES),
        CompletionContext::NodeIdReference => state
            .node_ids()
            .into_iter()
            .map(|id| CompletionItem {
                label: id.to_owned(),
                kind: Some(CompletionItemKind::REFERENCE),
                ..Default::default()
            })
            .collect(),
        CompletionContext::Unknown => return None,
    };

    Some(items)
}

fn make_enum_completions(values: &[&str]) -> Vec<CompletionItem> {
    values
        .iter()
        .map(|v| CompletionItem {
            label: v.to_string(),
            kind: Some(CompletionItemKind::ENUM_MEMBER),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentState;
    use tower_lsp::lsp_types::Position;

    const MULTI_NODE_SRC: &str = "\
agm: 1.0
package: test
version: 0.1.0

node auth.login
type: facts
summary: Login facts

node auth.logout
type: facts
summary: Logout facts
";

    fn make_state(src: &str) -> DocumentState {
        DocumentState::from_source(src, "test.agm")
    }

    #[test]
    fn test_completion_empty_line_in_header_suggests_header_fields() {
        // Line 0 is "agm: 1.0", line 3 (empty) is before first node
        let state = make_state(MULTI_NODE_SRC);
        // Use line 3 (the blank line before node declaration), col 0
        let items = provide_completions(&state, Position::new(3, 0));
        let items = items.unwrap();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"agm:"));
        assert!(labels.contains(&"package:"));
        assert!(labels.contains(&"version:"));
    }

    #[test]
    fn test_completion_empty_line_in_node_suggests_node_fields() {
        let state = make_state(MULTI_NODE_SRC);
        // Line 7 is empty (between node blocks, after type/summary of first node)
        let items = provide_completions(&state, Position::new(7, 0));
        let items = items.unwrap();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"type:"));
        assert!(labels.contains(&"summary:"));
        assert!(labels.contains(&"depends:"));
    }

    #[test]
    fn test_completion_after_type_colon_suggests_node_types() {
        // "type: " — cursor is after the colon
        let src =
            "agm: 1.0\npackage: test\nversion: 0.1.0\n\nnode auth.login\ntype: \nsummary: x\n";
        let state = make_state(src);
        let items = provide_completions(&state, Position::new(5, 6));
        let items = items.unwrap();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"facts"));
        assert!(labels.contains(&"workflow"));
    }

    #[test]
    fn test_completion_after_priority_colon_suggests_priorities() {
        let src = "agm: 1.0\npackage: test\nversion: 0.1.0\n\nnode auth.login\ntype: facts\nsummary: x\npriority: \n";
        let state = make_state(src);
        let items = provide_completions(&state, Position::new(7, 10));
        let items = items.unwrap();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"critical"));
        assert!(labels.contains(&"high"));
    }

    #[test]
    fn test_completion_after_stability_colon_suggests_stabilities() {
        let src = "agm: 1.0\npackage: test\nversion: 0.1.0\n\nnode auth.login\ntype: facts\nsummary: x\nstability: \n";
        let state = make_state(src);
        let items = provide_completions(&state, Position::new(7, 11));
        let items = items.unwrap();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"high"));
        assert!(labels.contains(&"medium"));
        assert!(labels.contains(&"low"));
        assert!(labels.contains(&"volatile"));
    }

    #[test]
    fn test_completion_after_depends_bracket_suggests_node_ids() {
        // Use a valid two-node document; test completions on a line that starts with "depends: "
        // The completion context is detected from the line text, not just the parsed file.
        // We need a document where the line at position is "depends: [auth.logout]"
        // and cursor is past the colon so NodeIdReference context is triggered.
        let src = "agm: 1.0\npackage: test\nversion: 0.1.0\n\nnode auth.login\ntype: facts\nsummary: x\ndepends: [auth.logout]\n\nnode auth.logout\ntype: facts\nsummary: y\n";
        let state = make_state(src);
        // Line 7: "depends: [auth.logout]", col 10 is past the colon
        let items = provide_completions(&state, Position::new(7, 10));
        let items = items.unwrap();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"auth.login") || labels.contains(&"auth.logout"));
    }

    #[test]
    fn test_completion_no_file_still_suggests_field_names() {
        // Invalid source — parse fails, file is None
        let state = DocumentState::from_source("not valid agm content\n", "test.agm");
        assert!(state.file.is_none());
        // Field name completions on empty-ish line should still work (heuristic)
        let items = provide_completions(&state, Position::new(0, 0));
        // Returns Some with field names (in_header = true since no nodes)
        assert!(items.is_some());
    }

    #[test]
    fn test_completion_unknown_field_returns_none() {
        let src = "agm: 1.0\npackage: test\nversion: 0.1.0\n\nnode auth.login\ntype: facts\nsummary: x\ncustom_field: val\n";
        let state = make_state(src);
        // cursor after colon on "custom_field: val" line
        let items = provide_completions(&state, Position::new(7, 14));
        assert!(items.is_none());
    }
}
