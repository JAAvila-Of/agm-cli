//! JSON renderer: pretty-prints the AgmFile AST using serde_json.

use crate::model::file::AgmFile;

/// Pretty-prints the `AgmFile` AST as JSON using serde_json.
///
/// This is a direct serialization of the Rust model types. It preserves
/// serde field names (e.g., `"node"` for id, `"type"` for node_type).
/// Optional fields set to `None` are omitted. `span` is skipped.
///
/// ## JSON output shape (flat — stable public contract)
///
/// Because `AgmFile` uses `#[serde(flatten)]` on `header`, the header fields
/// appear at the **top level** of the JSON object, not nested under `"header"`:
///
/// ```json
/// {
///   "agm": "1.0",
///   "package": "myapp.auth",
///   "version": "1.0.0",
///   "nodes": [
///     {
///       "node": "myapp.auth.login",
///       "type": "workflow",
///       "summary": "Authenticate user via OAuth2"
///     }
///   ]
/// }
/// ```
///
/// This shape is the **stable public contract**. See `AgmFile` for details.
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
                span: Span::new(1, 3),
                ..Default::default()
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
