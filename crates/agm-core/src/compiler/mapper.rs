//! Field mapper: converts MarkdownSection content to Node fields.

use std::collections::BTreeMap;

use crate::model::code::{CodeAction, CodeBlock};
use crate::model::fields::{NodeType, Span};
use crate::model::node::Node;

use super::section::MarkdownSection;

// ---------------------------------------------------------------------------
// FieldMapper
// ---------------------------------------------------------------------------

/// Maps section content to AGM node fields based on classified node type.
pub(crate) struct FieldMapper;

impl FieldMapper {
    pub fn new() -> Self {
        Self
    }

    /// Creates a `Node` from a `MarkdownSection`, its classified type, and ID.
    ///
    /// Field assignment rules:
    /// - Ordered list items -> `steps` (for workflow/orchestration) or `items`
    /// - Unordered list items -> `items`
    /// - Body paragraphs -> `detail`
    /// - Code blocks -> `code` (single) or `code_blocks` (multiple)
    /// - Summary is generated from the first meaningful content
    pub fn map_to_node(&self, section: &MarkdownSection, node_type: NodeType, id: &str) -> Node {
        let summary = self.generate_summary(section, &node_type);
        let detail = self.extract_detail(section);
        let (items, steps) = self.extract_list_fields(section, &node_type);
        let (code, code_blocks) = self.extract_code(section);

        Node {
            id: id.to_owned(),
            node_type,
            summary,
            priority: None,
            stability: None,
            confidence: None,
            status: None,
            depends: None,
            related_to: None,
            replaces: None,
            conflicts: None,
            see_also: None,
            items,
            steps,
            fields: None,
            input: None,
            output: None,
            detail,
            rationale: None,
            tradeoffs: None,
            resolution: None,
            examples: None,
            notes: None,
            code,
            code_blocks,
            verify: None,
            agent_context: None,
            target: None,
            execution_status: None,
            executed_by: None,
            executed_at: None,
            execution_log: None,
            retry_count: None,
            parallel_groups: None,
            memory: None,
            scope: None,
            applies_when: None,
            valid_from: None,
            valid_until: None,
            tags: None,
            aliases: None,
            keywords: None,
            extra_fields: BTreeMap::new(),
            span: Span::new(section.source_line_start, section.source_line_end),
        }
    }

    /// Generates a concise summary from section content.
    ///
    /// Strategy:
    /// 1. If body text exists, use the first sentence (up to 100 chars).
    /// 2. If only list items, join them with " -> " for workflow, "; " otherwise,
    ///    truncated to 100 chars.
    /// 3. Fallback: use the heading text.
    fn generate_summary(&self, section: &MarkdownSection, node_type: &NodeType) -> String {
        // Try body text first sentence
        let body = section.body_text.trim();
        if !body.is_empty() {
            let first_sentence = extract_first_sentence(body);
            if first_sentence.len() <= 100 {
                return first_sentence;
            }
            return truncate_at_word(&first_sentence, 100);
        }

        // Try list items
        if !section.list_items.is_empty() {
            let separator = match node_type {
                NodeType::Workflow | NodeType::Orchestration => " -> ",
                _ => "; ",
            };
            let joined = section.list_items.join(separator);
            if joined.len() <= 100 {
                return joined;
            }
            return truncate_at_word(&joined, 100);
        }

        // Fallback: heading
        let heading = section.heading.trim();
        if !heading.is_empty() {
            return heading.to_owned();
        }

        "untitled section".to_owned()
    }

    /// Extracts body text as the `detail` field.
    /// Returns `None` if there is no body text.
    fn extract_detail(&self, section: &MarkdownSection) -> Option<String> {
        let body = section.body_text.trim();
        if body.is_empty() {
            None
        } else {
            Some(body.to_owned())
        }
    }

    /// Extracts list items into `items` or `steps` depending on node type
    /// and list ordering.
    ///
    /// - Workflow/Orchestration + ordered list -> `steps`
    /// - All other combinations -> `items`
    fn extract_list_fields(
        &self,
        section: &MarkdownSection,
        node_type: &NodeType,
    ) -> (Option<Vec<String>>, Option<Vec<String>>) {
        if section.list_items.is_empty() {
            return (None, None);
        }

        let use_steps = section.is_ordered_list
            && matches!(node_type, NodeType::Workflow | NodeType::Orchestration);

        if use_steps {
            (None, Some(section.list_items.clone()))
        } else {
            (Some(section.list_items.clone()), None)
        }
    }

    /// Extracts code blocks into `code` (single block) or `code_blocks`
    /// (multiple blocks).
    fn extract_code(
        &self,
        section: &MarkdownSection,
    ) -> (Option<CodeBlock>, Option<Vec<CodeBlock>>) {
        if section.code_blocks.is_empty() {
            return (None, None);
        }

        let blocks: Vec<CodeBlock> = section
            .code_blocks
            .iter()
            .map(|(lang, body)| CodeBlock {
                lang: lang.clone(),
                target: None,
                action: CodeAction::Full,
                body: body.clone(),
                anchor: None,
                old: None,
            })
            .collect();

        if blocks.len() == 1 {
            (Some(blocks.into_iter().next().unwrap()), None)
        } else {
            (None, Some(blocks))
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extracts the first sentence from text (ends at '.', '!', or '?'
/// followed by a space or end of string).
fn extract_first_sentence(text: &str) -> String {
    for (i, ch) in text.char_indices() {
        if matches!(ch, '.' | '!' | '?') {
            let rest = &text[i + ch.len_utf8()..];
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\n') {
                return text[..=i].trim().to_owned();
            }
        }
    }
    // No sentence terminator found; return the whole text
    text.to_owned()
}

/// Truncates text at a word boundary, appending "..." if truncated.
fn truncate_at_word(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_owned();
    }

    let truncated = &text[..max_len];
    match truncated.rfind(' ') {
        Some(pos) => format!("{}...", &truncated[..pos]),
        None => format!("{truncated}..."),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::section::MarkdownSection;
    use super::*;
    use crate::model::fields::NodeType;

    fn make_section(heading: &str, body: &str, items: Vec<&str>, ordered: bool) -> MarkdownSection {
        MarkdownSection {
            heading: heading.to_owned(),
            heading_level: 2,
            body_text: body.to_owned(),
            list_items: items.into_iter().map(|s| s.to_owned()).collect(),
            is_ordered_list: ordered,
            code_blocks: Vec::new(),
            source_line_start: 1,
            source_line_end: 10,
        }
    }

    #[test]
    fn test_map_to_node_workflow_ordered_list_uses_steps() {
        let mapper = FieldMapper::new();
        let sec = make_section("Flow", "", vec!["Step A", "Step B"], true);
        let node = mapper.map_to_node(&sec, NodeType::Workflow, "auth.flow");
        assert!(node.steps.is_some());
        assert!(node.items.is_none());
        assert_eq!(node.steps.unwrap().len(), 2);
    }

    #[test]
    fn test_map_to_node_rules_unordered_list_uses_items() {
        let mapper = FieldMapper::new();
        let sec = make_section("Rules", "", vec!["Must X", "Must Y"], false);
        let node = mapper.map_to_node(&sec, NodeType::Rules, "auth.rules");
        assert!(node.items.is_some());
        assert!(node.steps.is_none());
    }

    #[test]
    fn test_map_to_node_body_text_becomes_detail() {
        let mapper = FieldMapper::new();
        let sec = make_section("Info", "Some detail text.", vec![], false);
        let node = mapper.map_to_node(&sec, NodeType::Facts, "info.node");
        assert_eq!(node.detail.as_deref(), Some("Some detail text."));
    }

    #[test]
    fn test_map_to_node_summary_from_body_first_sentence() {
        let mapper = FieldMapper::new();
        let sec = make_section("Info", "First sentence. Second sentence.", vec![], false);
        let node = mapper.map_to_node(&sec, NodeType::Facts, "info.node");
        assert_eq!(node.summary, "First sentence.");
    }

    #[test]
    fn test_map_to_node_summary_from_list_items_workflow() {
        let mapper = FieldMapper::new();
        let sec = make_section("Flow", "", vec!["A", "B", "C"], true);
        let node = mapper.map_to_node(&sec, NodeType::Workflow, "wf");
        assert_eq!(node.summary, "A -> B -> C");
    }

    #[test]
    fn test_map_to_node_summary_from_list_items_rules() {
        let mapper = FieldMapper::new();
        let sec = make_section("Rules", "", vec!["X", "Y"], false);
        let node = mapper.map_to_node(&sec, NodeType::Rules, "rules");
        assert_eq!(node.summary, "X; Y");
    }

    #[test]
    fn test_map_to_node_summary_fallback_to_heading() {
        let mapper = FieldMapper::new();
        let sec = make_section("My Heading", "", vec![], false);
        let node = mapper.map_to_node(&sec, NodeType::Facts, "heading");
        assert_eq!(node.summary, "My Heading");
    }

    #[test]
    fn test_map_to_node_single_code_block_uses_code_field() {
        let mapper = FieldMapper::new();
        let mut sec = make_section("Code", "", vec![], false);
        sec.code_blocks = vec![(Some("rust".to_owned()), "fn main() {}".to_owned())];
        let node = mapper.map_to_node(&sec, NodeType::Example, "code");
        assert!(node.code.is_some());
        assert!(node.code_blocks.is_none());
    }

    #[test]
    fn test_map_to_node_multiple_code_blocks_uses_code_blocks_field() {
        let mapper = FieldMapper::new();
        let mut sec = make_section("Code", "", vec![], false);
        sec.code_blocks = vec![
            (Some("rust".to_owned()), "fn a() {}".to_owned()),
            (Some("python".to_owned()), "def b(): pass".to_owned()),
        ];
        let node = mapper.map_to_node(&sec, NodeType::Example, "code");
        assert!(node.code.is_none());
        assert_eq!(node.code_blocks.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_map_to_node_id_and_type_correct() {
        let mapper = FieldMapper::new();
        let sec = make_section("Test", "Body.", vec![], false);
        let node = mapper.map_to_node(&sec, NodeType::Decision, "my.decision");
        assert_eq!(node.id, "my.decision");
        assert_eq!(node.node_type, NodeType::Decision);
    }

    #[test]
    fn test_extract_first_sentence_basic() {
        assert_eq!(extract_first_sentence("Hello world. More."), "Hello world.");
    }

    #[test]
    fn test_extract_first_sentence_no_period() {
        assert_eq!(extract_first_sentence("No period"), "No period");
    }

    #[test]
    fn test_truncate_at_word_short_text() {
        assert_eq!(truncate_at_word("short", 10), "short");
    }

    #[test]
    fn test_truncate_at_word_long_text() {
        let result = truncate_at_word("a very long sentence that exceeds the limit", 20);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 24); // 20 + "..."
    }
}
