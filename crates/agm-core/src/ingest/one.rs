//! `ingest_one`: ingest a single JSON object into a `Node`.

use serde_json::Value;

use crate::model::fields::NodeType;
use crate::model::node::Node;

use super::from_json::{
    decision_from_json, facts_from_json, orchestration_from_json, rules_from_json,
    ticket_from_json, workflow_from_json,
};
use super::schema_check::run_schema_check;
use super::{IngestConfig, IngestError};

// ---------------------------------------------------------------------------
// Id resolution (D6)
// ---------------------------------------------------------------------------

/// Resolves the node id following D6 priority:
/// JSON `"node"` field → JSON `"id"` field → caller-provided `id_arg`.
/// Returns `IngestError::MissingId` if all sources are empty.
fn resolve_id<'a>(v: &'a Value, id_arg: &'a str) -> Result<&'a str, IngestError> {
    if let Some(s) = v
        .get("node")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return Ok(s);
    }
    if let Some(s) = v
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return Ok(s);
    }
    if !id_arg.is_empty() {
        return Ok(id_arg);
    }
    Err(IngestError::MissingId)
}

// ---------------------------------------------------------------------------
// ingest_one
// ---------------------------------------------------------------------------

/// Ingest a single JSON object into a `Node`.
///
/// The pipeline (per D3):
/// 1. Assert `v.is_object()` → `IngestError::NotAnObject`.
/// 2. Optionally run JSON Schema check.
/// 3. Resolve id (D6).
/// 4. Dispatch to `*_from_json` helper; build with `Permissive` (avoids
///    spurious single-node cross-ref errors).
/// 5. Optionally normalize via a scratch `AgmFile`.
///
/// Full Standard/Strict validation is left to the caller, which assembles
/// the complete `AgmFile` before validating.
#[must_use = "check the Result for IngestError"]
pub fn ingest_one(
    node_type: NodeType,
    id: &str,
    value: Value,
    config: &IngestConfig,
) -> Result<Node, IngestError> {
    if !value.is_object() {
        return Err(IngestError::NotAnObject);
    }

    // Step 2: optional schema check
    if config.schema_check {
        run_schema_check(&node_type, &value)?;
    }

    // Step 3: id resolution
    let resolved_id = resolve_id(&value, id)?;
    let resolved_id = resolved_id.to_owned();

    // Step 4: build unchecked — full Standard/Strict validation is deferred to
    // the caller after the complete AgmFile is assembled.
    let node = dispatch_build(&node_type, &resolved_id, &value)?;

    // Step 5: optional normalize pass
    let node = if config.normalize {
        normalize_node(node)?
    } else {
        node
    };

    Ok(node)
}

// ---------------------------------------------------------------------------
// Per-type dispatch
// ---------------------------------------------------------------------------

fn dispatch_build(node_type: &NodeType, id: &str, v: &Value) -> Result<Node, IngestError> {
    // Use build_unchecked so that synthesized IDs (e.g. `prefix.0`) and
    // cross-ref fields do not trigger V021/V004 here.  Full Standard/Strict
    // validation is deferred to the caller after the complete AgmFile is built.
    match node_type {
        NodeType::Ticket => ticket_from_json(id, v)?
            .build_unchecked()
            .map_err(Into::into),
        NodeType::Workflow => workflow_from_json(id, v)?
            .build_unchecked()
            .map_err(Into::into),
        NodeType::Orchestration => orchestration_from_json(id, v)?
            .build_unchecked()
            .map_err(Into::into),
        NodeType::Facts => facts_from_json(id, v)?
            .build_unchecked()
            .map_err(Into::into),
        NodeType::Rules => rules_from_json(id, v)?
            .build_unchecked()
            .map_err(Into::into),
        NodeType::Decision => decision_from_json(id, v)?
            .build_unchecked()
            .map_err(Into::into),
        NodeType::Custom(_) => Err(IngestError::SchemaCheck(
            "custom types have no schema".to_owned(),
        )),
        // For node types without dedicated builders, build a generic Node directly.
        _ => build_generic(node_type, id, v),
    }
}

/// Builds a Node for types that don't have a dedicated builder (entity, exception,
/// example, glossary, anti_pattern). Constructs directly — no validation, matching
/// the `build_unchecked` contract used by builder types above.
fn build_generic(node_type: &NodeType, id: &str, v: &Value) -> Result<Node, IngestError> {
    use crate::model::fields::FieldValue;
    use crate::model::node::Node;

    let mut node = Node {
        id: id.to_owned(),
        node_type: node_type.clone(),
        ..Default::default()
    };

    if let Some(s) = v.get("summary").and_then(|v| v.as_str()) {
        node.summary = s.to_owned();
    }

    // Populate all remaining fields from JSON by routing through extra_fields
    if let Some(obj) = v.as_object() {
        const SKIP: &[&str] = &["node", "id", "type", "summary"];
        for (key, val) in obj {
            if !SKIP.contains(&key.as_str()) {
                let fv = match val {
                    serde_json::Value::String(s) => FieldValue::Scalar(s.clone()),
                    serde_json::Value::Array(arr) => FieldValue::List(
                        arr.iter()
                            .filter_map(|i| i.as_str().map(str::to_owned))
                            .collect(),
                    ),
                    other => FieldValue::Block(other.to_string()),
                };
                node.extra_fields.insert(key.clone(), fv);
            }
        }
    }

    Ok(node)
}

// ---------------------------------------------------------------------------
// Normalize pass
// ---------------------------------------------------------------------------

fn normalize_node(node: Node) -> Result<Node, IngestError> {
    use crate::model::file::{AgmFile, Header};
    use crate::normalize::{NormalizeConfig, normalize_ast};

    let mut file = AgmFile {
        header: Header {
            agm: "1.0".to_owned(),
            package: "scratch.ingest".to_owned(),
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
        nodes: vec![node],
    };

    let config = NormalizeConfig::default();
    let _ = normalize_ast(&mut file, &config);

    // Extract the single node back
    let node = file.nodes.remove(0);
    Ok(node)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::IngestConfig;
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
    fn test_ingest_one_rejects_non_object() {
        let v = json!([1, 2, 3]);
        let err = ingest_one(NodeType::Ticket, "t.x", v, &permissive_config()).unwrap_err();
        assert!(matches!(err, IngestError::NotAnObject));
    }

    #[test]
    fn test_ingest_one_ticket_minimal_succeeds() {
        let v = json!({
            "summary": "add login",
            "title": "Add Login",
            "description": "Implement login.",
            "priority": "high"
        });
        let node = ingest_one(NodeType::Ticket, "t.login", v, &permissive_config()).unwrap();
        assert_eq!(node.id, "t.login");
    }

    #[test]
    fn test_ingest_one_uses_json_node_field_as_id() {
        let v = json!({
            "node": "t.from-json",
            "summary": "s",
            "title": "T",
            "description": "d",
            "priority": "low"
        });
        let node = ingest_one(NodeType::Ticket, "t.fallback", v, &permissive_config()).unwrap();
        assert_eq!(node.id, "t.from-json");
    }

    #[test]
    fn test_ingest_one_uses_json_id_field_as_id() {
        let v = json!({
            "id": "t.from-id",
            "summary": "s"
        });
        let node = ingest_one(NodeType::Facts, "t.fallback", v, &permissive_config()).unwrap();
        assert_eq!(node.id, "t.from-id");
    }

    #[test]
    fn test_ingest_one_missing_id_returns_error() {
        let v = json!({"summary": "s"});
        let err = ingest_one(NodeType::Facts, "", v, &permissive_config()).unwrap_err();
        assert!(matches!(err, IngestError::MissingId));
    }

    #[test]
    fn test_ingest_one_custom_type_returns_error() {
        let v = json!({"summary": "s"});
        let err = ingest_one(
            NodeType::Custom("widget".to_owned()),
            "t.x",
            v,
            &permissive_config(),
        )
        .unwrap_err();
        assert!(matches!(err, IngestError::SchemaCheck(_)));
    }

    #[test]
    fn test_ingest_one_schema_check_rejects_bad_priority() {
        let v = json!({
            "summary": "s",
            "title": "T",
            "description": "d",
            "priority": "urgent"
        });
        let config = IngestConfig {
            schema_check: true,
            ..permissive_config()
        };
        let err = ingest_one(NodeType::Ticket, "t.x", v, &config).unwrap_err();
        assert!(matches!(err, IngestError::SchemaCheck(_)));
    }

    #[test]
    fn test_ingest_one_workflow_succeeds() {
        let v = json!({
            "summary": "authenticate user",
            "steps": ["resolve tenant", "redirect"]
        });
        let node = ingest_one(NodeType::Workflow, "auth.login", v, &permissive_config()).unwrap();
        assert_eq!(node.id, "auth.login");
        assert_eq!(node.steps.as_deref().unwrap().len(), 2);
    }

    #[test]
    fn test_ingest_one_normalize_pass_runs_without_error() {
        let v = json!({
            "summary": "auth constraints",
            "items": ["sessions expire"]
        });
        let config = IngestConfig {
            normalize: true,
            schema_check: false,
            enforcement: EnforcementLevel::Permissive,
        };
        let node = ingest_one(NodeType::Facts, "auth.facts", v, &config).unwrap();
        assert_eq!(node.id, "auth.facts");
    }
}
