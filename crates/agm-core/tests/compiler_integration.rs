//! Integration tests for the Markdown-to-AGM compiler.

#[cfg(feature = "compiler")]
mod compiler_tests {
    use agm_core::compiler::{CompileOptions, compile};
    use agm_core::parser;
    use agm_core::renderer::canonical::render_canonical;
    use agm_core::validator::{ValidateOptions, validate};

    fn default_opts(package: &str) -> CompileOptions {
        CompileOptions {
            package: package.to_owned(),
            version: "0.1.0".to_owned(),
            min_confidence: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_compile_spec_s35_4_example() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/login.md");
        let result = compile(md, &default_opts("auth.platform"));
        assert_eq!(result.file.nodes.len(), 2);

        // First node should be rules type
        assert_eq!(
            result.file.nodes[0].node_type,
            agm_core::model::fields::NodeType::Rules
        );

        // Second node should be workflow type
        assert_eq!(
            result.file.nodes[1].node_type,
            agm_core::model::fields::NodeType::Workflow
        );
    }

    #[test]
    fn test_compile_roundtrip_parse_validates() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/login.md");
        let result = compile(md, &default_opts("auth.platform"));

        // Render to canonical AGM text
        let agm_text = render_canonical(&result.file);

        // Parse the rendered text
        let parsed = parser::parse(&agm_text);
        assert!(
            parsed.is_ok(),
            "Compiled output failed to parse: {parsed:?}"
        );

        // Validate the parsed file
        let file = parsed.unwrap();
        let collection = validate(
            &file,
            &agm_text,
            "compiled.agm",
            &ValidateOptions::default(),
        );
        assert!(
            !collection.has_errors(),
            "Compiled output has validation errors"
        );
    }

    #[test]
    fn test_compile_multi_section() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/multi_section.md");
        let result = compile(md, &default_opts("auth.platform"));
        assert!(result.file.nodes.len() >= 3);
    }

    #[test]
    fn test_compile_with_code_blocks() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/with_code_blocks.md");
        let result = compile(md, &default_opts("test.pkg"));
        let nodes_with_code: Vec<_> = result
            .file
            .nodes
            .iter()
            .filter(|n| n.code.is_some() || n.code_blocks.is_some())
            .collect();
        assert!(!nodes_with_code.is_empty(), "No code blocks extracted");
    }

    #[test]
    fn test_compile_empty_file() {
        let result = compile("", &default_opts("test.pkg"));
        assert!(result.file.nodes.is_empty());
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_compile_no_headings_creates_single_node() {
        let md = include_str!("../../../tests/fixtures/compiler/invalid/no_headings.md");
        let result = compile(
            md,
            &CompileOptions {
                min_confidence: 0.0,
                ..default_opts("test.pkg")
            },
        );
        // Should produce at most one node from the body text
        assert!(result.file.nodes.len() <= 1);
    }

    #[test]
    fn test_compile_snapshot_login() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/login.md");
        let result = compile(md, &default_opts("auth.platform"));
        let agm_text = render_canonical(&result.file);
        insta::assert_snapshot!("compiler__login_roundtrip", agm_text);
    }

    #[test]
    fn test_compile_snapshot_multi_section() {
        let md = include_str!("../../../tests/fixtures/compiler/valid/multi_section.md");
        let result = compile(md, &default_opts("auth.platform"));
        let agm_text = render_canonical(&result.file);
        insta::assert_snapshot!("compiler__multi_section", agm_text);
    }
}
