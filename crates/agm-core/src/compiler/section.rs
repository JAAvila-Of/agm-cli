//! Markdown section extraction using pulldown-cmark.
//!
//! Splits a Markdown document into heading-delimited sections, each
//! capturing the heading text, body paragraphs, list items, and code blocks.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

// ---------------------------------------------------------------------------
// MarkdownSection
// ---------------------------------------------------------------------------

/// An intermediate extracted section from a Markdown document.
///
/// Each section corresponds to one heading and its content up to (but not
/// including) the next heading of equal or higher level.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MarkdownSection {
    /// The heading text (stripped of formatting).
    pub heading: String,
    /// The heading level (1 = h1, 2 = h2, etc.).
    pub heading_level: u8,
    /// Concatenated paragraph text (non-list, non-code body).
    pub body_text: String,
    /// Items extracted from unordered and ordered lists.
    /// For ordered lists, each item is prefixed with its index implicitly
    /// (the ordering is preserved in the Vec).
    pub list_items: Vec<String>,
    /// Whether the list items came from an ordered (numbered) list.
    pub is_ordered_list: bool,
    /// Fenced code blocks: (optional language tag, code content).
    pub code_blocks: Vec<(Option<String>, String)>,
    /// 1-indexed line number where this section starts in the source.
    pub source_line_start: usize,
    /// 1-indexed line number where this section ends in the source.
    pub source_line_end: usize,
}

// ---------------------------------------------------------------------------
// extract_sections()
// ---------------------------------------------------------------------------

/// Extracts heading-delimited sections from Markdown text.
///
/// If the input has no headings, returns a single section with an empty
/// heading covering the entire document body.
///
/// Sections are delimited by headings. A heading at level N closes any
/// open section at level >= N. This means h2 sections are peers, and an
/// h3 within an h2 creates a child section (separate `MarkdownSection`).
pub(crate) fn extract_sections(markdown: &str) -> Vec<MarkdownSection> {
    let opts = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(markdown, opts);

    let line_offsets = build_line_offsets(markdown);

    let mut sections: Vec<MarkdownSection> = Vec::new();
    let mut current: Option<MarkdownSection> = None;

    // State tracking
    let mut in_heading = false;
    let mut heading_text = String::new();
    let mut heading_level: u8 = 0;
    let mut _in_list = false;
    let mut in_list_item = false;
    let mut list_item_text = String::new();
    let mut is_ordered = false;
    let mut in_code_block = false;
    let mut code_lang: Option<String> = None;
    let mut code_body = String::new();
    let mut in_paragraph = false;

    for (event, range) in parser.into_offset_iter() {
        let line_num = offset_to_line(&line_offsets, range.start);

        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Close current section
                if let Some(mut sec) = current.take() {
                    sec.source_line_end = line_num.saturating_sub(1).max(sec.source_line_start);
                    sections.push(sec);
                }
                in_heading = true;
                heading_level = heading_level_to_u8(level);
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                current = Some(MarkdownSection {
                    heading: heading_text.trim().to_owned(),
                    heading_level,
                    body_text: String::new(),
                    list_items: Vec::new(),
                    is_ordered_list: false,
                    code_blocks: Vec::new(),
                    source_line_start: line_num,
                    source_line_end: line_num,
                });
            }

            Event::Start(Tag::List(first_item)) => {
                _in_list = true;
                is_ordered = first_item.is_some();
            }
            Event::End(TagEnd::List(_)) => {
                _in_list = false;
                if let Some(ref mut sec) = current {
                    sec.is_ordered_list = is_ordered;
                }
            }
            Event::Start(Tag::Item) => {
                in_list_item = true;
                list_item_text.clear();
            }
            Event::End(TagEnd::Item) => {
                in_list_item = false;
                if let Some(ref mut sec) = current {
                    sec.list_items.push(list_item_text.trim().to_owned());
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let l = lang.trim().to_owned();
                        if l.is_empty() { None } else { Some(l) }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                code_body.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                if let Some(ref mut sec) = current {
                    sec.code_blocks
                        .push((code_lang.take(), code_body.trim_end().to_owned()));
                } else {
                    // Code block before any heading -- create implicit section
                    let mut sec = MarkdownSection {
                        heading: String::new(),
                        heading_level: 0,
                        body_text: String::new(),
                        list_items: Vec::new(),
                        is_ordered_list: false,
                        code_blocks: vec![(code_lang.take(), code_body.trim_end().to_owned())],
                        source_line_start: line_num,
                        source_line_end: line_num,
                    };
                    sec.source_line_end = line_num;
                    current = Some(sec);
                }
                code_body.clear();
            }

            Event::Start(Tag::Paragraph) => {
                in_paragraph = true;
            }
            Event::End(TagEnd::Paragraph) => {
                in_paragraph = false;
            }

            Event::Text(text) | Event::Code(text) => {
                if in_heading {
                    heading_text.push_str(&text);
                } else if in_code_block {
                    code_body.push_str(&text);
                } else if in_list_item {
                    list_item_text.push_str(&text);
                } else if in_paragraph {
                    if let Some(ref mut sec) = current {
                        if !sec.body_text.is_empty() && !sec.body_text.ends_with('\n') {
                            sec.body_text.push(' ');
                        }
                        sec.body_text.push_str(&text);
                    }
                }
            }

            Event::SoftBreak | Event::HardBreak => {
                if in_heading {
                    heading_text.push(' ');
                } else if in_list_item {
                    list_item_text.push(' ');
                } else if let Some(ref mut sec) = current {
                    if in_paragraph {
                        sec.body_text.push(' ');
                    }
                }
            }

            _ => {}
        }
    }

    // Close final section
    if let Some(mut sec) = current.take() {
        let total_lines = markdown.lines().count();
        sec.source_line_end = total_lines.max(sec.source_line_start);
        sections.push(sec);
    }

    // Handle: no headings at all
    if sections.is_empty() && !markdown.trim().is_empty() {
        sections.push(MarkdownSection {
            heading: String::new(),
            heading_level: 0,
            body_text: markdown.to_owned(),
            list_items: Vec::new(),
            is_ordered_list: false,
            code_blocks: Vec::new(),
            source_line_start: 1,
            source_line_end: markdown.lines().count().max(1),
        });
    }

    sections
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds a sorted vec of byte offsets where each line starts.
fn build_line_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Converts a byte offset to a 1-indexed line number.
fn offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    }
}

/// Converts pulldown-cmark HeadingLevel to u8.
fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sections_single_heading_with_list() {
        let md = "## Login Flow\n\n- Step one\n- Step two\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading, "Login Flow");
        assert_eq!(sections[0].heading_level, 2);
        assert_eq!(sections[0].list_items.len(), 2);
        assert_eq!(sections[0].list_items[0], "Step one");
    }

    #[test]
    fn test_extract_sections_two_headings() {
        let md = "## First\n\nParagraph one.\n\n## Second\n\nParagraph two.\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading, "First");
        assert_eq!(sections[1].heading, "Second");
    }

    #[test]
    fn test_extract_sections_ordered_list_detected() {
        let md = "## Steps\n\n1. Do this\n2. Do that\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 1);
        assert!(sections[0].is_ordered_list);
        assert_eq!(sections[0].list_items.len(), 2);
    }

    #[test]
    fn test_extract_sections_code_block_captured() {
        let md = "## Code Example\n\n```rust\nfn main() {}\n```\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].code_blocks.len(), 1);
        assert_eq!(sections[0].code_blocks[0].0.as_deref(), Some("rust"));
        assert!(sections[0].code_blocks[0].1.contains("fn main()"));
    }

    #[test]
    fn test_extract_sections_no_headings_returns_single_section() {
        let md = "Just some text without any headings.\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 1);
        assert!(sections[0].heading.is_empty());
        assert!(sections[0].body_text.contains("Just some text"));
    }

    #[test]
    fn test_extract_sections_empty_input_returns_empty() {
        let sections = extract_sections("");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_sections_mixed_content() {
        let md = "\
## Constraints

- Must do X
- Must not do Y

Some explanatory text.

## Flow

1. Step one
2. Step two

```bash
echo hello
```
";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].list_items.len(), 2);
        assert!(sections[0].body_text.contains("explanatory"));
        assert_eq!(sections[1].list_items.len(), 2);
        assert!(sections[1].is_ordered_list);
        assert_eq!(sections[1].code_blocks.len(), 1);
    }

    #[test]
    fn test_extract_sections_nested_headings() {
        let md = "## Parent\n\nText.\n\n### Child\n\nChild text.\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading, "Parent");
        assert_eq!(sections[0].heading_level, 2);
        assert_eq!(sections[1].heading, "Child");
        assert_eq!(sections[1].heading_level, 3);
    }

    #[test]
    fn test_extract_sections_source_line_numbers() {
        let md = "## First\n\nLine.\n\n## Second\n\nText.\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 2);
        assert!(sections[0].source_line_start >= 1);
        assert!(sections[1].source_line_start > sections[0].source_line_start);
    }

    #[test]
    fn test_extract_sections_body_text_paragraph() {
        let md = "## Heading\n\nFirst paragraph text.\n\nSecond paragraph text.\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 1);
        // body_text should contain content from paragraphs
        assert!(sections[0].body_text.contains("First paragraph"));
    }
}
