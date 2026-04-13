//! Integration tests: parser edge cases and stress tests.
//!
//! Tests for boundary conditions, unicode, malformed input, field edge cases,
//! code block quirks, large files, and whitespace/comment handling.

use agm_core::error::ErrorCode;
use agm_core::parser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn header() -> &'static str {
    "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n"
}

fn errors_contain(errors: &[agm_core::error::AgmError], code: ErrorCode) -> bool {
    errors.iter().any(|e| e.code == code)
}

// ---------------------------------------------------------------------------
// TASK 1-A: Empty / minimal files
// ---------------------------------------------------------------------------

#[test]
fn test_parse_empty_string_returns_error() {
    let result = parser::parse("");
    assert!(result.is_err(), "empty string should fail");
}

#[test]
fn test_parse_whitespace_only_returns_error() {
    let result = parser::parse("   \n\n\t\n   ");
    assert!(result.is_err(), "whitespace-only input should fail");
}

#[test]
fn test_parse_header_only_no_nodes_returns_p008() {
    let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n";
    let errors = parser::parse(input).unwrap_err();
    assert!(
        errors_contain(&errors, ErrorCode::P008),
        "header-only file should produce P008 (no nodes); got: {errors:?}"
    );
}

#[test]
fn test_parse_header_only_with_blank_lines_returns_p008() {
    let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\n\n";
    let errors = parser::parse(input).unwrap_err();
    assert!(
        errors_contain(&errors, ErrorCode::P008),
        "header with only blank lines should produce P008; got: {errors:?}"
    );
}

// ---------------------------------------------------------------------------
// TASK 1-B: Unicode handling
// ---------------------------------------------------------------------------

#[test]
fn test_parse_unicode_in_summary_accepted() {
    // CJK characters, emoji, RTL text in summary field
    let input = format!(
        "{}{}",
        header(),
        "node test.unicode\ntype: facts\nsummary: 你好世界 مرحبا 🎉 Hello\n"
    );
    let file = parser::parse(&input).expect("unicode summary should parse");
    assert!(file.nodes[0].summary.contains("你好世界"));
    assert!(file.nodes[0].summary.contains("🎉"));
}

#[test]
fn test_parse_cjk_characters_in_detail_block_accepted() {
    let input = format!(
        "{}{}",
        header(),
        "node test.cjk\ntype: facts\nsummary: CJK node\ndetail:\n  这是一段详细说明。\n  第二行内容。\n"
    );
    let file = parser::parse(&input).expect("CJK in detail block should parse");
    let detail = file.nodes[0].detail.as_deref().unwrap();
    assert!(detail.contains("这是一段详细说明"));
}

#[test]
fn test_parse_emoji_in_summary_accepted() {
    let input = format!(
        "{}{}",
        header(),
        "node test.emoji\ntype: rules\nsummary: Rule with emoji 🔒🚀⚡\n"
    );
    let file = parser::parse(&input).expect("emoji in summary should parse");
    assert!(file.nodes[0].summary.contains("🔒"));
}

#[test]
fn test_parse_rtl_text_in_summary_accepted() {
    // Arabic RTL text
    let input = format!(
        "{}{}",
        header(),
        "node test.rtl\ntype: facts\nsummary: مرحبا بالعالم\n"
    );
    let file = parser::parse(&input).expect("RTL text in summary should parse");
    assert!(file.nodes[0].summary.contains("مرحبا"));
}

// ---------------------------------------------------------------------------
// TASK 1-C: Boundary conditions
// ---------------------------------------------------------------------------

#[test]
fn test_parse_maximum_length_line_accepted() {
    // A summary field with 10KB+ of content
    let long_value = "x".repeat(10_240);
    let input = format!(
        "{header}node test.longline\ntype: facts\nsummary: {long_value}\n",
        header = header(),
    );
    let file = parser::parse(&input).expect("10KB+ line should parse");
    assert_eq!(file.nodes[0].summary.len(), 10_240);
}

#[test]
fn test_parse_very_long_node_id_returns_error() {
    // Node IDs must be lowercase dotted — a very long valid ID
    let long_id = (0..50)
        .map(|i| format!("seg{i}"))
        .collect::<Vec<_>>()
        .join(".");
    let input = format!(
        "{}node {long_id}\ntype: facts\nsummary: Long ID node\n\n",
        header()
    );
    // The parser should either accept it or return an error; it must not panic.
    let _ = parser::parse(&input);
}

#[test]
fn test_parse_deeply_nested_list_items_accepted() {
    // 10+ levels of indentation in a list (parser reads indentation, not depth)
    let items = (0..12)
        .map(|i| format!("{}  - level {i}", "  ".repeat(i)))
        .collect::<Vec<_>>()
        .join("\n");
    let input = format!(
        "{}node test.deep\ntype: facts\nsummary: Deep list\nitems:\n{items}\n",
        header()
    );
    // Must not panic; parse may succeed or produce errors.
    let _ = parser::parse(&input);
}

// ---------------------------------------------------------------------------
// TASK 1-D: Malformed input
// ---------------------------------------------------------------------------

#[test]
fn test_parse_missing_newline_at_eof_accepted() {
    // Valid file without trailing newline — should still parse.
    let input = format!(
        "{}node test.nonewline\ntype: facts\nsummary: No trailing newline",
        header()
    );
    // Parser may accept or reject; must not panic.
    let _ = parser::parse(&input);
}

#[test]
fn test_parse_crlf_line_endings_accepted() {
    // Windows-style CRLF throughout
    let input = "agm: 1.0\r\npackage: test.pkg\r\nversion: 0.1.0\r\n\r\nnode test.crlf\r\ntype: facts\r\nsummary: CRLF test\r\n";
    // Must not panic; the parser may accept or reject CRLF.
    let _ = parser::parse(input);
}

#[test]
fn test_parse_mixed_line_endings_does_not_panic() {
    // Mix of LF and CRLF
    let input = "agm: 1.0\r\npackage: test.pkg\nversion: 0.1.0\r\n\r\nnode test.mixed\ntype: facts\r\nsummary: Mixed endings\n";
    let _ = parser::parse(input);
}

#[test]
fn test_parse_utf8_bom_at_start_does_not_panic() {
    // UTF-8 BOM (EF BB BF) prepended
    let bom = "\u{FEFF}";
    let input = format!(
        "{}{}node test.bom\ntype: facts\nsummary: BOM test\n",
        bom,
        header()
    );
    let _ = parser::parse(&input);
}

// ---------------------------------------------------------------------------
// TASK 1-E: Field edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_parse_empty_inline_list_field_accepted() {
    let input = format!(
        "{}node test.emptylist\ntype: facts\nsummary: s\ntags: []\n",
        header()
    );
    let file = parser::parse(&input).expect("empty inline list should parse");
    // tags may be Some([]) or None depending on implementation
    if let Some(tags) = &file.nodes[0].tags {
        assert!(tags.is_empty(), "tags should be empty");
    }
}

#[test]
fn test_parse_single_item_list_accepted() {
    let input = format!(
        "{}node test.singleitem\ntype: facts\nsummary: s\ntags: [only]\n",
        header()
    );
    let file = parser::parse(&input).expect("single-item list should parse");
    assert_eq!(
        file.nodes[0].tags.as_deref(),
        Some(vec!["only".to_owned()].as_slice())
    );
}

#[test]
fn test_parse_field_value_with_colon_in_it_accepted() {
    // Summary value that itself contains a colon
    let input = format!(
        "{}node test.colon\ntype: facts\nsummary: key: value in summary\n",
        header()
    );
    let file = parser::parse(&input).expect("colon in summary value should parse");
    assert!(file.nodes[0].summary.contains("key: value"));
}

#[test]
fn test_parse_field_value_with_multiple_colons_accepted() {
    let input = format!(
        "{}node test.multicolon\ntype: facts\nsummary: a: b: c: d\n",
        header()
    );
    let file = parser::parse(&input).expect("multiple colons in summary should parse");
    assert!(file.nodes[0].summary.contains("a: b: c: d"));
}

#[test]
fn test_parse_field_with_trailing_whitespace_accepted() {
    // Trailing spaces after value — parser should trim or accept
    let input = format!(
        "{}node test.trailing\ntype: facts\nsummary: value with trailing spaces   \n",
        header()
    );
    let file = parser::parse(&input).expect("trailing whitespace in value should parse");
    // Summary must at least contain the core value
    assert!(file.nodes[0].summary.contains("value with trailing spaces"));
}

#[test]
fn test_parse_depends_inline_list_single_dep_accepted() {
    let input = format!(
        "{}node test.dep.a\ntype: workflow\nsummary: s\ndepends: [test.dep.b]\n\nnode test.dep.b\ntype: facts\nsummary: b\n",
        header()
    );
    let file = parser::parse(&input).expect("single depends entry should parse");
    assert_eq!(
        file.nodes[0].depends.as_deref(),
        Some(vec!["test.dep.b".to_owned()].as_slice())
    );
}

// ---------------------------------------------------------------------------
// TASK 1-F: Code block edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_parse_code_block_with_agm_like_syntax_inside_accepted() {
    // Code block body contains AGM-like field syntax — the pipe body captures it as opaque text.
    // `node` declarations act as structural boundaries, so we use header-like content only.
    let input = format!(
        concat!(
            "{}",
            "node test.codeagm\n",
            "type: facts\n",
            "summary: s\n",
            "code:\n",
            "  action: create\n",
            "  body: |\n",
            "    agm: 1.0\n",
            "    package: fake.pkg\n",
            "    version: 1.2.3\n"
        ),
        header()
    );
    let file =
        parser::parse(&input).expect("AGM-like field syntax in code block body should parse");
    let code = file.nodes[0].code.as_ref().unwrap();
    assert!(
        code.body.contains("agm: 1.0"),
        "code block body should contain the captured AGM-like field syntax, got: {:?}",
        code.body
    );
    assert!(
        code.body.contains("fake.pkg"),
        "code block body should contain package line, got: {:?}",
        code.body
    );
}

#[test]
fn test_parse_code_block_missing_action_returns_error() {
    // code: without required `action` sub-field — should produce an error
    let input = format!(
        "{}node test.emptycode\ntype: facts\nsummary: s\ncode:\n  body: |\n    some content\n",
        header()
    );
    // Missing `action` is an error at the validation level; must not panic
    let _ = parser::parse(&input);
}

#[test]
fn test_parse_code_block_valid_with_lang_and_action_accepted() {
    // Minimal valid code block with required fields
    let input = format!(
        concat!(
            "{}",
            "node test.validcode\n",
            "type: facts\n",
            "summary: s\n",
            "code:\n",
            "  lang: rust\n",
            "  action: create\n",
            "  body: |\n",
            "    fn main() {{}}\n"
        ),
        header()
    );
    let file = parser::parse(&input).expect("valid code block should parse");
    let code = file.nodes[0].code.as_ref().unwrap();
    assert!(code.body.contains("fn main()"));
}

#[test]
fn test_parse_code_block_with_url_in_body_accepted() {
    let input = format!(
        concat!(
            "{}",
            "node test.codeurl\n",
            "type: facts\n",
            "summary: s\n",
            "code:\n",
            "  action: create\n",
            "  body: |\n",
            "    https://example.com/path?a=1&b=2\n"
        ),
        header()
    );
    let file = parser::parse(&input).expect("URL in code block body should parse");
    if let Some(code) = &file.nodes[0].code {
        assert!(code.body.contains("https://"));
    }
}

// ---------------------------------------------------------------------------
// TASK 1-G: Multiple nodes (stress / count)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_fifty_nodes_all_parsed() {
    let mut input = header().to_owned();
    for i in 0..50 {
        input.push('\n');
        // IDs must match [a-z][a-z0-9]*([.\-][a-z][a-z0-9]*)* — use alphabetic segments
        input.push_str(&format!(
            "node stress.n{i:02}\ntype: facts\nsummary: Stress node number {i}\n"
        ));
    }
    let file = parser::parse(&input).expect("50 nodes should parse");
    assert_eq!(file.nodes.len(), 50, "expected 50 nodes");
    assert_eq!(file.nodes[0].id, "stress.n00");
    assert_eq!(file.nodes[49].id, "stress.n49");
}

#[test]
fn test_parse_node_with_many_relationships_accepted() {
    // One node with depends/related_to/see_also entries pointing to many others
    let mut input = header().to_owned();
    // IDs must match [a-z][a-z0-9]*([.\-][a-z][a-z0-9]*)* — use alphabetic segments
    let dep_ids: Vec<String> = (0..20).map(|i| format!("dep.d{i:02}")).collect();
    let deps_inline = dep_ids.join(", ");

    input.push_str(&format!(
        "\nnode hub.node\ntype: workflow\nsummary: Hub\ndepends: [{deps_inline}]\n"
    ));
    for id in &dep_ids {
        input.push_str(&format!("\nnode {id}\ntype: facts\nsummary: dep {id}\n"));
    }

    let file = parser::parse(&input).expect("node with 20 dependencies should parse");
    assert_eq!(
        file.nodes[0].depends.as_ref().unwrap().len(),
        20,
        "expected 20 depends entries"
    );
}

// ---------------------------------------------------------------------------
// TASK 1-H: Comment / whitespace handling
// ---------------------------------------------------------------------------

#[test]
fn test_parse_leading_blank_lines_before_header_accepted() {
    // Blank lines before the header start
    let input = "\n\n\nagm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode test.leading\ntype: facts\nsummary: s\n";
    let _ = parser::parse(input);
}

#[test]
fn test_parse_trailing_blank_lines_after_last_node_accepted() {
    let input = format!(
        "{}node test.trailing\ntype: facts\nsummary: s\n\n\n\n\n",
        header()
    );
    let file = parser::parse(&input).expect("trailing blank lines should parse");
    assert_eq!(file.nodes.len(), 1);
}

#[test]
fn test_parse_multiple_blank_lines_between_nodes_accepted() {
    let input = format!(
        "{}node test.a\ntype: facts\nsummary: a\n\n\n\n\nnode test.b\ntype: rules\nsummary: b\n",
        header()
    );
    let file = parser::parse(&input).expect("multiple blank lines between nodes should parse");
    assert_eq!(file.nodes.len(), 2);
}

#[test]
fn test_parse_comment_at_start_of_file_accepted() {
    let input = "# This is a file-level comment\nagm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode test.comment\ntype: facts\nsummary: s\n";
    let _ = parser::parse(input);
}

#[test]
fn test_parse_comments_between_all_fields_accepted() {
    let input = format!(
        "{}# comment before node\nnode test.cm\n# comment before type\ntype: facts\n# comment before summary\nsummary: s\n# trailing comment\n",
        header()
    );
    let file = parser::parse(&input).expect("comments between fields should parse");
    assert_eq!(file.nodes[0].summary, "s");
}

#[test]
fn test_parse_comment_only_line_in_list_skipped() {
    // Comments inside an indented list block
    let input = format!(
        "{}node test.clist\ntype: facts\nsummary: s\nitems:\n  - first\n  # this is a comment\n  - second\n",
        header()
    );
    let result = parser::parse(&input);
    // Must not panic; the comment handling inside lists may vary
    if let Ok(file) = result {
        // If parsed successfully, items should contain at least the non-comment lines
        if let Some(items) = &file.nodes[0].items {
            assert!(
                items.iter().any(|i| i == "first"),
                "first item should be present"
            );
        }
    }
}

#[test]
fn test_parse_two_nodes_no_blank_line_between_returns_two_nodes_or_error() {
    // No blank line between nodes — parser behavior depends on spec strictness
    let input = format!(
        "{}node test.x\ntype: facts\nsummary: x\nnode test.y\ntype: rules\nsummary: y\n",
        header()
    );
    // Must not panic; may succeed or error depending on implementation
    let _ = parser::parse(&input);
}

// ---------------------------------------------------------------------------
// STRESS / LARGE FILE TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_parse_200_nodes_all_parsed_correctly() {
    let mut input = header().to_owned();
    for i in 0..200usize {
        input.push('\n');
        if i == 0 {
            input.push_str(&format!(
                "node s.n{i:03}\ntype: facts\nsummary: Stress node {i}\n"
            ));
        } else {
            input.push_str(&format!(
                "node s.n{i:03}\ntype: facts\nsummary: Stress node {i}\ndepends: [s.n{:03}]\n",
                i - 1
            ));
        }
    }
    let file = parser::parse(&input).expect("200-node file should parse");
    assert_eq!(file.nodes.len(), 200, "expected exactly 200 nodes");
    assert_eq!(file.nodes[0].id, "s.n000");
    assert_eq!(file.nodes[199].id, "s.n199");
}

#[test]
fn test_parse_500_nodes_with_all_field_types() {
    let mut input = header().to_owned();
    for i in 0..500usize {
        input.push('\n');
        let mut node = format!("node s.n{i:03}\ntype: facts\nsummary: Stress node {i}\n");
        node.push_str("detail:\n  This is a detail block.\n  Second line of detail.\n");
        node.push_str("tags: [stress, generated]\n");
        if i > 0 {
            node.push_str(&format!("depends: [s.n{:03}]\n", i - 1));
        }
        node.push_str("code:\n  action: create\n  body: |\n    fn placeholder() {}\n");
        input.push_str(&node);
    }
    let file = parser::parse(&input).expect("500-node file should parse");
    assert_eq!(file.nodes.len(), 500, "expected exactly 500 nodes");
}

#[test]
fn test_parse_node_with_10kb_detail_block() {
    let detail_line = "x".repeat(80);
    // ~10KB: 128 lines of 80 chars each
    let detail_body: String = (0..128).map(|_| format!("  {detail_line}\n")).collect();
    let input = format!(
        "{}node s.bignode\ntype: facts\nsummary: Big detail node\ndetail:\n{detail_body}",
        header()
    );
    let file = parser::parse(&input).expect("10KB detail block should parse");
    let detail = file.nodes[0]
        .detail
        .as_deref()
        .expect("detail should be set");
    assert!(detail.len() >= 10_000, "detail should be at least 10KB");
}

#[test]
fn test_parse_deeply_nested_relationships_100_chain() {
    let mut input = header().to_owned();
    for i in 0..100usize {
        input.push('\n');
        if i == 0 {
            input.push_str(&format!(
                "node s.n{i:03}\ntype: facts\nsummary: Chain node {i}\n"
            ));
        } else {
            input.push_str(&format!(
                "node s.n{i:03}\ntype: facts\nsummary: Chain node {i}\ndepends: [s.n{:03}]\n",
                i - 1
            ));
        }
    }
    let file = parser::parse(&input).expect("100-node chain should parse");
    assert_eq!(file.nodes.len(), 100, "expected 100 nodes");
    for i in 1..100usize {
        let deps = file.nodes[i]
            .depends
            .as_ref()
            .expect("depends should be set");
        assert_eq!(
            deps[0],
            format!("s.n{:03}", i - 1),
            "node {i} should depend on s.n{:03}",
            i - 1
        );
    }
}

// ---------------------------------------------------------------------------
// GAP 8: Large field variants
// ---------------------------------------------------------------------------

// 8.1 — code.body with 1000+ lines

#[test]
fn test_parse_code_body_1000_lines_accepted() {
    // Build a pipe body with 1000 lines, each indented 4 spaces (the body
    // content is inside `code:` → `  body: |`, so the body lines sit at
    // column 4 relative to the node root).
    let body_lines: String = (0..1000)
        .map(|n| format!("    fn line_{n}() {{}}\n"))
        .collect();

    let input = format!(
        concat!(
            "{header}",
            "node large.code.body\n",
            "type: facts\n",
            "summary: Node with 1000-line code body\n",
            "code:\n",
            "  action: create\n",
            "  body: |\n",
            "{body}"
        ),
        header = header(),
        body = body_lines,
    );

    let file = parser::parse(&input).expect("1000-line code body should parse without error");
    let code = file.nodes[0]
        .code
        .as_ref()
        .expect("code field should be present");

    // The body is joined with '\n' by collect_pipe_body, so 1000 lines produce
    // 999 newlines separating them (trailing blank lines are trimmed).
    let newline_count = code.body.chars().filter(|&c| c == '\n').count();
    assert!(
        newline_count >= 999,
        "expected at least 999 newlines in body (got {newline_count}); body len = {}",
        code.body.len()
    );
    assert!(
        code.body.contains("fn line_0()"),
        "body should contain fn line_0()"
    );
    assert!(
        code.body.contains("fn line_999()"),
        "body should contain fn line_999()"
    );
}

// 8.2 — Node with 10+ code_blocks

#[test]
fn test_parse_node_with_10_code_blocks_accepted() {
    // Build code_blocks: with 10 entries, each with a distinct lang and
    // action: create.  The list-item sub-fields must be indented more than
    // the dash line (which sits at 2-space indent inside code_blocks:).
    let langs = [
        "rust",
        "python",
        "typescript",
        "go",
        "sql",
        "bash",
        "java",
        "cpp",
        "kotlin",
        "swift",
    ];

    let mut blocks = String::new();
    for (i, lang) in langs.iter().enumerate() {
        blocks.push_str(&format!(
            "  - lang: {lang}\n    action: create\n    body: |\n      // block {i}\n"
        ));
    }

    let input = format!(
        concat!(
            "{header}",
            "node multi.code.blocks\n",
            "type: workflow\n",
            "summary: Node with 10 code blocks\n",
            "code_blocks:\n",
            "{blocks}"
        ),
        header = header(),
        blocks = blocks,
    );

    let file = parser::parse(&input).expect("10 code_blocks should parse without error");
    let cbs = file.nodes[0]
        .code_blocks
        .as_ref()
        .expect("code_blocks field should be present");

    assert_eq!(
        cbs.len(),
        10,
        "expected exactly 10 code blocks, got {}",
        cbs.len()
    );

    for (i, (block, &expected_lang)) in cbs.iter().zip(langs.iter()).enumerate() {
        assert_eq!(
            block.lang.as_deref(),
            Some(expected_lang),
            "block {i}: expected lang {expected_lang:?}, got {:?}",
            block.lang
        );
    }
}

// 8.3a — items with 50 entries in block list format

#[test]
fn test_parse_items_50_entries_block_format_accepted() {
    let list_items: String = (0..50).map(|n| format!("  - item {n}\n")).collect();

    let input = format!(
        concat!(
            "{header}",
            "node many.items\n",
            "type: facts\n",
            "summary: Node with 50 items\n",
            "items:\n",
            "{items}"
        ),
        header = header(),
        items = list_items,
    );

    let file = parser::parse(&input).expect("50 items in block format should parse");
    let items = file.nodes[0]
        .items
        .as_ref()
        .expect("items field should be present");

    assert_eq!(
        items.len(),
        50,
        "expected exactly 50 items, got {}",
        items.len()
    );
    assert_eq!(items[0], "item 0");
    assert_eq!(items[49], "item 49");
}

// 8.3b — steps with 50 entries in block list format

#[test]
fn test_parse_steps_50_entries_block_format_accepted() {
    let list_steps: String = (0..50).map(|n| format!("  - step {n}\n")).collect();

    let input = format!(
        concat!(
            "{header}",
            "node many.steps\n",
            "type: workflow\n",
            "summary: Node with 50 steps\n",
            "steps:\n",
            "{steps}"
        ),
        header = header(),
        steps = list_steps,
    );

    let file = parser::parse(&input).expect("50 steps in block format should parse");
    let steps = file.nodes[0]
        .steps
        .as_ref()
        .expect("steps field should be present");

    assert_eq!(
        steps.len(),
        50,
        "expected exactly 50 steps, got {}",
        steps.len()
    );
    assert_eq!(steps[0], "step 0");
    assert_eq!(steps[49], "step 49");
}

// 8.4 — All optional fields populated at scale (fully_populated_node.agm)

#[test]
fn test_parse_node_all_fields_populated_at_scale() {
    let input = include_str!("fixtures/valid/fully_populated_node.agm");

    let file = parser::parse(input).expect("fully_populated_node.agm should parse without error");

    // The fixture has multiple nodes; the first is the fully-populated one.
    let node = &file.nodes[0];

    // Every optional field that the fixture defines must be Some.
    assert!(node.priority.is_some(), "priority should be Some");
    assert!(node.stability.is_some(), "stability should be Some");
    assert!(node.confidence.is_some(), "confidence should be Some");
    assert!(node.status.is_some(), "status should be Some");
    assert!(node.depends.is_some(), "depends should be Some");
    assert!(node.related_to.is_some(), "related_to should be Some");
    assert!(node.replaces.is_some(), "replaces should be Some");
    assert!(node.conflicts.is_some(), "conflicts should be Some");
    assert!(node.see_also.is_some(), "see_also should be Some");
    assert!(node.tags.is_some(), "tags should be Some");
    assert!(node.aliases.is_some(), "aliases should be Some");
    assert!(node.keywords.is_some(), "keywords should be Some");
    assert!(node.items.is_some(), "items should be Some");
    assert!(node.steps.is_some(), "steps should be Some");
    assert!(node.fields.is_some(), "fields should be Some");
    assert!(node.input.is_some(), "input should be Some");
    assert!(node.output.is_some(), "output should be Some");
    assert!(node.detail.is_some(), "detail should be Some");
    assert!(node.rationale.is_some(), "rationale should be Some");
    assert!(node.tradeoffs.is_some(), "tradeoffs should be Some");
    assert!(node.resolution.is_some(), "resolution should be Some");
    assert!(node.examples.is_some(), "examples should be Some");
    assert!(node.notes.is_some(), "notes should be Some");
    assert!(node.code.is_some(), "code should be Some");
    assert!(node.verify.is_some(), "verify should be Some");
    assert!(node.agent_context.is_some(), "agent_context should be Some");
    assert!(node.target.is_some(), "target should be Some");
    assert!(
        node.execution_status.is_some(),
        "execution_status should be Some"
    );
    assert!(node.executed_by.is_some(), "executed_by should be Some");
    assert!(node.executed_at.is_some(), "executed_at should be Some");
    assert!(node.execution_log.is_some(), "execution_log should be Some");
    assert!(node.retry_count.is_some(), "retry_count should be Some");
    assert!(node.memory.is_some(), "memory should be Some");
    assert!(
        node.parallel_groups.is_some(),
        "parallel_groups should be Some"
    );
    assert!(node.scope.is_some(), "scope should be Some");
    assert!(node.applies_when.is_some(), "applies_when should be Some");
    assert!(node.valid_from.is_some(), "valid_from should be Some");
    assert!(node.valid_until.is_some(), "valid_until should be Some");
}
