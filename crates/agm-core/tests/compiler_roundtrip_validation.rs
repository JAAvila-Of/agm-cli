//! Gap 5 — Compiler Roundtrip Validation Tests
//!
//! Verifies that compiled output:
//! 1. Renders to canonical AGM text that parses and validates without errors.
//! 2. Respects merge_same_type semantics.
//! 3. Produces unique IDs under id_prefix, including collision resolution.

#[cfg(feature = "compiler")]
mod compiler_roundtrip_tests {
    use agm_core::compiler::{CompileOptions, CompileWarningKind, compile};
    use agm_core::parser;
    use agm_core::renderer::canonical::render_canonical;
    use agm_core::validator::{ValidateOptions, validate};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn opts_with_prefix(package: &str, prefix: Option<&str>) -> CompileOptions {
        CompileOptions {
            package: package.to_owned(),
            version: "0.1.0".to_owned(),
            min_confidence: 0.0,
            merge_same_type: false,
            id_prefix: prefix.map(str::to_owned),
        }
    }

    /// Generate markdown with a mix of section types cycling through the
    /// five semantic archetypes used in the mixed-type tests.
    fn generate_mixed_markdown(section_count: usize) -> String {
        let mut md = String::new();
        let types = ["Rules", "Workflow", "Glossary", "Anti-Pattern", "Facts"];
        for i in 0..section_count {
            let kind = types[i % types.len()];
            md.push_str(&format!("## {kind} Section {i}\n\n"));
            match kind {
                "Rules" => md.push_str(&format!(
                    "- Must enforce rule {i}.\n- Shall validate input {i}.\n\n"
                )),
                "Workflow" => md.push_str(&format!(
                    "1. Step A for section {i}.\n2. Step B for section {i}.\n\n"
                )),
                "Glossary" => md.push_str(&format!(
                    "**Term {i}**: Definition of term {i}.\n\n"
                )),
                "Anti-Pattern" => md.push_str(&format!(
                    "Avoid doing X in scenario {i}. Never use pattern {i}.\n\n"
                )),
                _ => md.push_str(&format!("General information about topic {i}.\n\n")),
            }
        }
        md
    }

    // -----------------------------------------------------------------------
    // 5.1 — Compiled output passes validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_200_sections_output_validates_cleanly() {
        let md = generate_mixed_markdown(200);
        let result = compile(&md, &opts_with_prefix("large.doc", None));

        let agm_text = render_canonical(&result.file);

        let parsed = parser::parse(&agm_text);
        assert!(
            parsed.is_ok(),
            "Compiled 200-section output failed to parse: {:?}",
            parsed.err()
        );

        let file = parsed.unwrap();
        let collection = validate(&file, &agm_text, "compiled.agm", &ValidateOptions::default());
        assert!(
            !collection.has_errors(),
            "Compiled 200-section output has validation errors: {:?}",
            collection.diagnostics()
        );
    }

    #[test]
    fn test_compile_mixed_types_output_validates_cleanly() {
        // 50 sections: 10 of each semantic type
        let md = generate_mixed_markdown(50);
        let result = compile(&md, &opts_with_prefix("mixed.doc", None));

        assert!(
            !result.file.nodes.is_empty(),
            "Expected nodes from 50-section mixed document"
        );

        let agm_text = render_canonical(&result.file);

        let parsed = parser::parse(&agm_text);
        assert!(
            parsed.is_ok(),
            "Compiled mixed-types output failed to parse: {:?}",
            parsed.err()
        );

        let file = parsed.unwrap();
        let collection = validate(&file, &agm_text, "compiled.agm", &ValidateOptions::default());
        assert!(
            !collection.has_errors(),
            "Compiled mixed-types output has validation errors: {:?}",
            collection.diagnostics()
        );
    }

    // -----------------------------------------------------------------------
    // 5.2 — merge_same_type
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_merge_same_type_100_consecutive_rules_produces_one_node() {
        let mut md = String::new();
        for i in 0..100 {
            md.push_str(&format!("## Rules Section {i}\n\n"));
            md.push_str(&format!("- Must enforce rule {i}.\n- Shall validate input {i}.\n\n"));
        }

        let result = compile(
            &md,
            &CompileOptions {
                package: "rules.doc".to_owned(),
                version: "0.1.0".to_owned(),
                min_confidence: 0.0,
                merge_same_type: true,
                id_prefix: None,
            },
        );

        assert_eq!(
            result.file.nodes.len(),
            1,
            "100 consecutive rule sections with merge_same_type should produce 1 node, got {}",
            result.file.nodes.len()
        );

        // The merged node should have items from all 100 sections
        let node = &result.file.nodes[0];
        let item_count = node.items.as_ref().map_or(0, |v| v.len())
            + node.steps.as_ref().map_or(0, |v| v.len());
        assert!(
            item_count >= 100,
            "Merged node should have at least 100 items (2 per section × 100 sections), got {item_count}"
        );
    }

    #[test]
    fn test_compile_merge_same_type_alternating_types_no_merge() {
        let mut md = String::new();
        for i in 0..100 {
            if i % 2 == 0 {
                md.push_str(&format!("## Rules Section {i}\n\n"));
                md.push_str(&format!("- Must enforce rule {i}.\n- Shall validate input {i}.\n\n"));
            } else {
                md.push_str(&format!("## Workflow Section {i}\n\n"));
                md.push_str(&format!("1. Step A for section {i}.\n2. Step B for section {i}.\n\n"));
            }
        }

        let result = compile(
            &md,
            &CompileOptions {
                package: "alternating.doc".to_owned(),
                version: "0.1.0".to_owned(),
                min_confidence: 0.0,
                merge_same_type: true,
                id_prefix: None,
            },
        );

        assert_eq!(
            result.file.nodes.len(),
            100,
            "Alternating types with merge_same_type should produce 100 nodes (no consecutive same-type), got {}",
            result.file.nodes.len()
        );
    }

    // -----------------------------------------------------------------------
    // 5.3 — id_prefix uniqueness
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_id_prefix_200_sections_all_unique_ids() {
        let mut md = String::new();
        for i in 0..200 {
            md.push_str(&format!("## Section Unique Heading {i}\n\n"));
            md.push_str(&format!("General information about topic {i}.\n\n"));
        }

        let result = compile(
            &md,
            &CompileOptions {
                package: "auth.doc".to_owned(),
                version: "0.1.0".to_owned(),
                min_confidence: 0.0,
                merge_same_type: false,
                id_prefix: Some("auth".to_owned()),
            },
        );

        assert_eq!(
            result.file.nodes.len(),
            200,
            "Expected 200 nodes, got {}",
            result.file.nodes.len()
        );

        // All IDs must start with "auth."
        for node in &result.file.nodes {
            assert!(
                node.id.starts_with("auth."),
                "Node ID '{}' does not start with 'auth.'",
                node.id
            );
        }

        // All IDs must be unique
        let mut seen = std::collections::HashSet::new();
        for node in &result.file.nodes {
            assert!(
                seen.insert(node.id.clone()),
                "Duplicate node ID found: '{}'",
                node.id
            );
        }
    }

    #[test]
    fn test_compile_id_prefix_200_sections_duplicate_headings_still_unique() {
        let mut md = String::new();
        for _ in 0..200 {
            md.push_str("## Login Rules\n\n");
            md.push_str("General information about login rules.\n\n");
        }

        let result = compile(
            &md,
            &CompileOptions {
                package: "auth.doc".to_owned(),
                version: "0.1.0".to_owned(),
                min_confidence: 0.0,
                merge_same_type: false,
                id_prefix: Some("auth".to_owned()),
            },
        );

        assert_eq!(
            result.file.nodes.len(),
            200,
            "Expected 200 nodes from 200 duplicate-heading sections, got {}",
            result.file.nodes.len()
        );

        // All IDs must be unique despite duplicate headings
        let mut seen = std::collections::HashSet::new();
        for node in &result.file.nodes {
            assert!(
                seen.insert(node.id.clone()),
                "Duplicate node ID found: '{}'",
                node.id
            );
        }

        // At least one IdCollision warning must have been emitted (199 collisions)
        let collision_count = result
            .warnings
            .iter()
            .filter(|w| w.kind == CompileWarningKind::IdCollision)
            .count();
        assert!(
            collision_count >= 199,
            "Expected at least 199 IdCollision warnings for 200 identical headings, got {collision_count}"
        );
    }
}
