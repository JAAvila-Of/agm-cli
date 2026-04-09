//! JSON renderer: pretty-prints the AgmFile AST using serde_json.

use crate::model::file::AgmFile;

/// Pretty-prints the `AgmFile` AST as JSON using serde_json.
///
/// This is a direct serialization of the Rust model types. It preserves
/// serde field names (e.g., `"node"` for id, `"type"` for node_type).
/// Optional fields set to `None` are omitted. `span` is skipped.
#[must_use]
pub fn render_json(file: &AgmFile) -> String {
    serde_json::to_string_pretty(file).expect("AgmFile is always serializable")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::{FieldValue, NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;
    use std::collections::BTreeMap;

    fn minimal_file() -> AgmFile {
        AgmFile {
            header: Header {
                agm: "1".to_owned(),
                package: "test.minimal".to_owned(),
                version: "0.1.0".to_owned(),
                title: None,
                owner: None,
                imports: None,
                default_load: None,
                description: None,
                tags: None,
                status: None,
                load_profiles: None,
                target_runtime: None,
            },
            nodes: vec![Node {
                id: "test.node".to_owned(),
                node_type: NodeType::Facts,
                summary: "a minimal test node".to_owned(),
                priority: None,
                stability: None,
                confidence: None,
                status: None,
                depends: None,
                related_to: None,
                replaces: None,
                conflicts: None,
                see_also: None,
                items: None,
                steps: None,
                fields: None,
                input: None,
                output: None,
                detail: None,
                rationale: None,
                tradeoffs: None,
                resolution: None,
                examples: None,
                notes: None,
                code: None,
                code_blocks: None,
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
                span: Span::new(1, 3),
            }],
        }
    }

    #[test]
    fn test_render_json_minimal_valid_json() {
        let file = minimal_file();
        let output = render_json(&file);
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("should be valid JSON");
        assert!(parsed.is_object());
    }

    #[test]
    fn test_render_json_omits_none_fields() {
        let file = minimal_file();
        let output = render_json(&file);
        assert!(!output.contains("priority"));
        assert!(!output.contains("depends"));
        assert!(!output.contains("steps"));
        assert!(!output.contains("code"));
        assert!(!output.contains("execution_status"));
        assert!(!output.contains("memory"));
        assert!(!output.contains("title"));
        assert!(!output.contains("owner"));
    }

    #[test]
    fn test_render_json_span_not_serialized() {
        let file = minimal_file();
        let output = render_json(&file);
        assert!(!output.contains("start_line"));
        assert!(!output.contains("end_line"));
        assert!(!output.contains("span"));
    }

    #[test]
    fn test_render_json_uses_spec_field_names() {
        let file = minimal_file();
        let output = render_json(&file);
        // serde renames: id -> "node", node_type -> "type"
        assert!(output.contains("\"node\""));
        assert!(output.contains("\"type\""));
        assert!(!output.contains("\"id\""));
        assert!(!output.contains("\"node_type\""));
    }

    #[test]
    fn test_render_json_extra_fields_inlined() {
        let mut file = minimal_file();
        file.nodes[0].extra_fields.insert(
            "custom_key".to_owned(),
            FieldValue::Scalar("custom_val".to_owned()),
        );
        let output = render_json(&file);
        assert!(output.contains("custom_key"));
        assert!(output.contains("custom_val"));
    }

    #[test]
    fn test_render_json_agm_version_as_stored_string() {
        // The raw serde renderer emits agm as stored (string "1"),
        // while the canonical renderer converts it to integer.
        let file = minimal_file();
        let output = render_json(&file);
        // The agm field is present.
        assert!(output.contains("\"agm\""));
    }
}
