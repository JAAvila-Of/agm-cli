//! `ingest_many`: ingest a JSON array into `Vec<Node>`.

use serde_json::Value;

use crate::model::fields::NodeType;
use crate::model::node::Node;

use super::one::ingest_one;
use super::{IngestConfig, IngestError};

// ---------------------------------------------------------------------------
// ingest_many
// ---------------------------------------------------------------------------

/// Ingest a JSON array into a `Vec<Node>`.
///
/// - Each element must be a JSON object.
/// - Element-level `"node"` / `"id"` fields override the synthesized id.
/// - Missing ids are synthesized as `"{id_prefix}.{index}"` (0-based).
/// - All elements are attempted; errors are collected (D9).
/// - If any element fails, returns `Err(IngestError::Batch { errors })`.
#[must_use = "check the Result for IngestError"]
pub fn ingest_many(
    node_type: NodeType,
    id_prefix: &str,
    values: Vec<Value>,
    config: &IngestConfig,
) -> Result<Vec<Node>, IngestError> {
    let mut nodes: Vec<Node> = Vec::with_capacity(values.len());
    let mut errors: Vec<(usize, IngestError)> = Vec::new();

    for (i, v) in values.into_iter().enumerate() {
        // Synthesize fallback id
        let fallback = format!("{id_prefix}.{i}");
        match ingest_one(node_type.clone(), &fallback, v, config) {
            Ok(node) => nodes.push(node),
            Err(e) => errors.push((i, e)),
        }
    }

    if errors.is_empty() {
        Ok(nodes)
    } else {
        Err(IngestError::Batch { errors })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::schema::EnforcementLevel;
    use serde_json::json;

    fn permissive_config() -> IngestConfig {
        IngestConfig {
            normalize: false,
            schema_check: false,
            enforcement: EnforcementLevel::Permissive,
        }
    }

    #[test]
    fn test_ingest_many_all_valid_returns_nodes() {
        let values = vec![
            json!({"summary": "auth constraints", "items": ["sessions expire"]}),
            json!({"summary": "auth rules", "items": ["require HTTPS"]}),
        ];
        let nodes =
            ingest_many(NodeType::Facts, "auth.batch", values, &permissive_config()).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].id, "auth.batch.0");
        assert_eq!(nodes[1].id, "auth.batch.1");
    }

    #[test]
    fn test_ingest_many_uses_id_prefix_when_missing() {
        let values = vec![json!({"summary": "facts"})];
        let nodes = ingest_many(NodeType::Facts, "prefix", values, &permissive_config()).unwrap();
        assert_eq!(nodes[0].id, "prefix.0");
    }

    #[test]
    fn test_ingest_many_element_node_field_overrides_prefix() {
        let values = vec![json!({"node": "auth.explicit", "summary": "explicit id"})];
        let nodes = ingest_many(NodeType::Facts, "fallback", values, &permissive_config()).unwrap();
        assert_eq!(nodes[0].id, "auth.explicit");
    }

    #[test]
    fn test_ingest_many_aggregates_errors() {
        let values = vec![
            json!({"summary": "valid facts"}),
            json!([1, 2, 3]), // not an object
            json!({"summary": "also valid"}),
        ];
        let err = ingest_many(NodeType::Facts, "p", values, &permissive_config()).unwrap_err();
        match err {
            IngestError::Batch { errors } => {
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].0, 1); // index 1 failed
                assert!(matches!(errors[0].1, IngestError::NotAnObject));
            }
            other => panic!("expected Batch error, got: {other}"),
        }
    }

    #[test]
    fn test_ingest_many_all_fail_returns_batch_with_all_errors() {
        let values = vec![json!([1, 2]), json!([3, 4])];
        let err = ingest_many(NodeType::Facts, "p", values, &permissive_config()).unwrap_err();
        match err {
            IngestError::Batch { errors } => {
                assert_eq!(errors.len(), 2);
            }
            other => panic!("expected Batch error, got: {other}"),
        }
    }

    #[test]
    fn test_ingest_many_empty_input_returns_empty_vec() {
        let nodes = ingest_many(NodeType::Facts, "p", vec![], &permissive_config()).unwrap();
        assert!(nodes.is_empty());
    }
}
