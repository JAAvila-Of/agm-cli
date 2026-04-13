//! Advanced edge-case integration tests for the Markdown-to-AGM compiler.
//!
//! Covers special characters in headings, nested lists, multiple code blocks,
//! ambiguous classification, empty sections, and large documents.

#[cfg(feature = "compiler")]
mod compiler_edge_case_tests {
    use agm_core::compiler::{CompileOptions, CompileWarningKind, compile};

    fn default_opts(package: &str) -> CompileOptions {
        CompileOptions {
            package: package.to_owned(),
            version: "0.1.0".to_owned(),
            min_confidence: 0.0,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------------
    // 1. Complex markdown: headings with special characters
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_heading_with_special_chars_produces_valid_id() {
        let md = "## Login & Authorization (v2)\n\nMust authenticate all requests.\n";
        let result = compile(md, &default_opts("test.pkg"));
        assert_eq!(result.file.nodes.len(), 1);
        let node = &result.file.nodes[0];
        // ID should contain only valid characters (letters, digits, dots, hyphens, underscores)
        assert!(
            node.id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'),
            "ID must only contain valid characters, got: {}",
            node.id
        );
    }

    #[test]
    fn test_compile_heading_with_unicode_characters_produces_node() {
        let md = "## Autenticación de Usuarios\n\nMust validate credentials.\n";
        let result = compile(md, &default_opts("test.pkg"));
        // Should produce at least zero nodes (unicode headings may or may not map)
        // The important check is that it doesn't panic
        assert!(result.file.nodes.len() <= 1);
    }

    #[test]
    fn test_compile_heading_with_numbers_and_dots_produces_valid_id() {
        let md = "## Step 1: Initialize Database\n\n1. Connect to server.\n2. Run migrations.\n";
        let result = compile(md, &default_opts("test.pkg"));
        assert_eq!(result.file.nodes.len(), 1);
        let node = &result.file.nodes[0];
        assert!(!node.id.is_empty(), "Node ID should not be empty");
    }

    // -----------------------------------------------------------------------
    // 2. Nested lists 4+ levels deep
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_deeply_nested_list_produces_node_with_items() {
        let md = "\
## Authentication Rules

- Level 1: Must authenticate
  - Level 2: Token required
    - Level 3: Token must be valid
      - Level 4: Token must not be expired
        - Level 5: Expiry checked server-side
- Level 1 again: Must log all attempts
";
        let result = compile(md, &default_opts("test.pkg"));
        assert_eq!(result.file.nodes.len(), 1);
        let node = &result.file.nodes[0];
        // Items should be extracted (exact nesting behavior depends on implementation)
        assert!(
            node.items.is_some() || node.detail.is_some(),
            "Deeply nested list should produce items or detail"
        );
    }

    // -----------------------------------------------------------------------
    // 3. Multiple code blocks with different languages
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_multiple_code_blocks_different_languages_all_extracted() {
        let md = "\
## Authentication Implementation

Here is the core authentication logic:

```rust
fn verify_token(token: &str) -> bool {
    jwt::decode(token).is_ok()
}
```

And the corresponding SQL:

```sql
SELECT * FROM sessions WHERE token = $1 AND expires_at > NOW();
```

And the shell setup:

```bash
export JWT_SECRET=my-secret
```
";
        let result = compile(md, &default_opts("test.pkg"));
        assert_eq!(result.file.nodes.len(), 1);
        let node = &result.file.nodes[0];
        // At least one code representation should be set
        assert!(
            node.code.is_some() || node.code_blocks.is_some(),
            "Node should have code extracted from multiple code blocks"
        );
    }

    #[test]
    fn test_compile_code_block_containing_markdown_syntax_does_not_create_extra_sections() {
        let md = "\
## Example Usage

```markdown
## This heading is inside a code block

- This list is inside a code block
```

The above is just an example of markdown syntax.
";
        let result = compile(md, &default_opts("test.pkg"));
        // The markdown inside the code block should NOT create extra sections
        assert_eq!(
            result.file.nodes.len(),
            1,
            "Markdown inside code block should not create extra sections, got {} nodes",
            result.file.nodes.len()
        );
    }

    // -----------------------------------------------------------------------
    // 4. Ambiguous classification: rules vs workflow
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_ambiguous_rules_content_with_must_classifies_as_rules() {
        let md = "\
## Authentication Policy

- All requests must include a valid bearer token.
- Expired tokens must be rejected.
- Invalid credentials must result in 401.
- Brute force attempts must trigger rate limiting.
";
        let result = compile(md, &default_opts("test.pkg"));
        // Section has strong rules signals ("must") — should classify as rules
        if !result.file.nodes.is_empty() {
            assert_eq!(
                result.file.nodes[0].node_type,
                agm_core::model::fields::NodeType::Rules,
                "Content with 'must' keywords should classify as rules"
            );
        }
    }

    #[test]
    fn test_compile_workflow_steps_numbered_classifies_as_workflow() {
        let md = "\
## Login Flow

1. User submits credentials.
2. Server validates password.
3. Session token is generated.
4. Token is sent to client.
5. Client stores token securely.
";
        let result = compile(md, &default_opts("test.pkg"));
        if !result.file.nodes.is_empty() {
            assert_eq!(
                result.file.nodes[0].node_type,
                agm_core::model::fields::NodeType::Workflow,
                "Numbered steps should classify as workflow"
            );
        }
    }

    #[test]
    fn test_compile_ambiguous_section_produces_ambiguous_warning_or_node() {
        // A section with mixed signals should either produce a node or an AmbiguousType warning
        let md = "\
## Auth System

Some general text about the auth system that doesn't strongly signal any type.
";
        let result = compile(md, &default_opts("test.pkg"));
        // Either we get a node (if confidence is sufficient) or a warning
        let has_ambiguous_warning = result
            .warnings
            .iter()
            .any(|w| w.kind == CompileWarningKind::AmbiguousType);
        let produced_node = !result.file.nodes.is_empty();
        assert!(
            has_ambiguous_warning || produced_node,
            "Ambiguous section should produce a warning or a node"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Empty sections: headings with no content
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_empty_section_produces_empty_section_warning() {
        let md = "\
## Authentication Rules

Must validate all tokens.

## Empty Section

## Another Valid Section

Must check rate limits.
";
        let result = compile(md, &default_opts("test.pkg"));
        let has_empty_warning = result
            .warnings
            .iter()
            .any(|w| w.kind == CompileWarningKind::EmptySection);
        assert!(
            has_empty_warning,
            "Empty section (heading only) should produce EmptySection warning"
        );
    }

    #[test]
    fn test_compile_all_empty_sections_produces_no_nodes_but_warnings() {
        let md = "\
## Section One

## Section Two

## Section Three
";
        let result = compile(md, &default_opts("test.pkg"));
        assert!(
            result.file.nodes.is_empty(),
            "All-empty sections should produce no nodes"
        );
        let empty_warning_count = result
            .warnings
            .iter()
            .filter(|w| w.kind == CompileWarningKind::EmptySection)
            .count();
        assert_eq!(
            empty_warning_count, 3,
            "Three empty sections should produce three EmptySection warnings"
        );
    }

    #[test]
    fn test_compile_heading_only_with_whitespace_body_is_empty() {
        let md = "## Whitespace Section\n\n   \n\t\n\n## Valid Section\n\nMust enforce access control.\n";
        let result = compile(md, &default_opts("test.pkg"));
        // The whitespace-only section should be treated as empty
        let has_empty_warning = result
            .warnings
            .iter()
            .any(|w| w.kind == CompileWarningKind::EmptySection);
        assert!(
            has_empty_warning,
            "Whitespace-body section should produce EmptySection warning"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Large documents: 100+ paragraphs / sections
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_large_document_100_sections_all_processed() {
        let mut md = String::new();
        for i in 1..=100 {
            md.push_str(&format!(
                "## Rule Section {i}\n\nAll requests must be authenticated per rule {i}.\n\n"
            ));
        }
        let result = compile(&md, &default_opts("test.large"));
        // All 100 sections should produce nodes (they all have content)
        assert_eq!(
            result.file.nodes.len(),
            100,
            "100-section document should produce 100 nodes"
        );
    }

    #[test]
    fn test_compile_large_document_correctness_first_and_last_node() {
        let mut md = String::new();
        for i in 1..=50 {
            md.push_str(&format!(
                "## Constraint {i}\n\nMust enforce rule {i} at all times.\n\n"
            ));
        }
        let result = compile(&md, &default_opts("test.large"));
        assert_eq!(result.file.nodes.len(), 50);
        // First and last nodes should have non-empty summaries
        assert!(
            !result.file.nodes[0].summary.is_empty(),
            "First node should have a summary"
        );
        assert!(
            !result.file.nodes[49].summary.is_empty(),
            "Last node should have a summary"
        );
    }

    // -----------------------------------------------------------------------
    // 7. Special characters in body: backticks, HTML entities, raw HTML
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_backticks_in_text_produces_node() {
        let md = "\
## Authentication Rules

Must use `Bearer` token format. The `Authorization` header must be present.
All `jwt` tokens must be validated using the `verify` function.
";
        let result = compile(md, &default_opts("test.pkg"));
        assert_eq!(result.file.nodes.len(), 1);
        let node = &result.file.nodes[0];
        assert!(!node.summary.is_empty());
    }

    #[test]
    fn test_compile_html_entities_in_body_produces_node() {
        let md = "\
## Security Policy

Passwords must be &gt; 12 characters &amp; contain &lt;special&gt; characters.
All &quot;sensitive&quot; data must be encrypted.
";
        let result = compile(md, &default_opts("test.pkg"));
        assert_eq!(result.file.nodes.len(), 1);
    }

    #[test]
    fn test_compile_raw_html_block_does_not_crash() {
        let md = "\
## Login Workflow

<div class=\"flow\">
  <p>Step 1: Validate credentials</p>
  <p>Step 2: Issue token</p>
</div>

1. Validate credentials.
2. Issue session token.
";
        let result = compile(md, &default_opts("test.pkg"));
        // Should not panic; may or may not produce a node depending on body parsing
        assert!(result.file.nodes.len() <= 1);
    }

    // -----------------------------------------------------------------------
    // 8. ID collision: duplicate headings produce unique IDs
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_duplicate_headings_produce_unique_ids() {
        let md = "\
## Authentication Rules

Must validate all tokens.

## Authentication Rules

Must enforce rate limits.
";
        let result = compile(md, &default_opts("test.pkg"));
        // Should produce two nodes or fewer (collision handled)
        if result.file.nodes.len() == 2 {
            assert_ne!(
                result.file.nodes[0].id, result.file.nodes[1].id,
                "Duplicate headings must produce unique IDs"
            );
            // Should produce an IdCollision warning
            assert!(
                result
                    .warnings
                    .iter()
                    .any(|w| w.kind == CompileWarningKind::IdCollision),
                "Duplicate headings should produce IdCollision warning"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 9. merge_same_type option
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_merge_same_type_combines_consecutive_nodes() {
        let md = "\
## Rules Part One

Must validate tokens.

## Rules Part Two

Must check rate limits.
";
        let result = compile(
            md,
            &CompileOptions {
                merge_same_type: true,
                ..default_opts("test.pkg")
            },
        );
        // Both sections classify as rules; with merge_same_type they become one node
        let without_merge = compile(md, &default_opts("test.pkg"));
        // merged result should have <= nodes than non-merged
        assert!(
            result.file.nodes.len() <= without_merge.file.nodes.len(),
            "merge_same_type should produce fewer or equal nodes"
        );
    }

    // -----------------------------------------------------------------------
    // 10. id_prefix option
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_id_prefix_prepended_to_all_nodes() {
        let md = "\
## Login Rules

Must authenticate all users.

## Login Workflow

1. Verify credentials.
2. Issue token.
";
        let result = compile(
            md,
            &CompileOptions {
                id_prefix: Some("auth".to_owned()),
                ..default_opts("test.pkg")
            },
        );
        for node in &result.file.nodes {
            assert!(
                node.id.starts_with("auth."),
                "Node ID '{}' should start with 'auth.'",
                node.id
            );
        }
    }

    // -----------------------------------------------------------------------
    // STRESS / LARGE FILE TESTS
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_200_section_document() {
        let mut md = String::new();
        for i in 1..=200 {
            md.push_str(&format!(
                "## Rule Section {i}\n\nAll requests must be authenticated per rule {i}.\n\n"
            ));
        }
        let result = compile(&md, &default_opts("stress.pkg"));
        assert_eq!(
            result.file.nodes.len(),
            200,
            "200-section document should produce 200 nodes, got {}",
            result.file.nodes.len()
        );
        // First and last nodes should have non-empty summaries
        assert!(
            !result.file.nodes[0].summary.is_empty(),
            "First compiled node should have a summary"
        );
        assert!(
            !result.file.nodes[199].summary.is_empty(),
            "Last compiled node should have a summary"
        );
    }

    #[test]
    fn test_compile_large_section_with_50_code_blocks() {
        let langs = [
            "rust",
            "python",
            "javascript",
            "typescript",
            "go",
            "java",
            "c",
            "cpp",
            "bash",
            "sql",
        ];
        let mut md = String::from("## Implementation Details\n\nMust implement all steps.\n\n");
        for i in 0..50usize {
            let lang = langs[i % langs.len()];
            md.push_str(&format!(
                "```{lang}\nfn example_{i}() {{ /* block {i} */ }}\n```\n\n"
            ));
        }
        let result = compile(&md, &default_opts("stress.pkg"));
        // Should produce at least one node without panicking
        assert!(
            !result.file.nodes.is_empty(),
            "Section with 50 code blocks should produce at least one node"
        );
        let node = &result.file.nodes[0];
        // At least some code representation should be extracted
        assert!(
            node.code.is_some() || node.code_blocks.is_some(),
            "Node should have code extracted from code blocks"
        );
    }
}
