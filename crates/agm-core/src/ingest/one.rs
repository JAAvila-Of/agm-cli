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
// Enrichment pass
// ---------------------------------------------------------------------------

/// Pre-process a raw JSON object before schema validation and builder dispatch.
///
/// Three injections are applied unconditionally:
/// 1. `"type"` — injected from the `node_type` CLI arg if absent in JSON.
///    If JSON already has `"type"` and it differs from `node_type`, returns
///    `IngestError::SchemaCheck` with a descriptive mismatch message.
/// 2. `"node"` — injected from the resolved `id` arg if absent in JSON.
///    JSON `"node"` or `"id"` fields take priority per the D6 rule that runs
///    *after* enrichment; injecting here satisfies the schema validator.
/// 3. `"summary"` — synthesized from `"title"` when absent. This preserves
///    LLM ergonomics: tool callers only need to supply a human-readable
///    `title`; the `summary` field (required by AGM) is derived automatically.
///
/// The enriched `Value` is used for both schema check **and** as the input to
/// `*_from_json` builders, so produced `Node` values always carry synthesized
/// fields consistently.
pub fn enrich_for_ingest(
    node_type: &NodeType,
    id: &str,
    mut v: Value,
) -> Result<Value, IngestError> {
    let type_wire = node_type.to_string();

    // 1. Inject or validate "type"
    match v.get("type").and_then(|t| t.as_str()) {
        Some(existing) if existing != type_wire => {
            return Err(IngestError::SchemaCheck(format!(
                "type mismatch: JSON says `{existing}` but ingest was called for `{type_wire}`"
            )));
        }
        None => {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("type".to_owned(), Value::String(type_wire));
            }
        }
        Some(_) => {} // already matches — leave it
    }

    // 2. Inject "node" if neither "node" nor "id" is present.
    // This satisfies the schema validator; D6 id resolution still runs after
    // enrichment and will prefer JSON "node" > JSON "id" > caller arg.
    // We must NOT inject "node" when JSON already has "id" — that would
    // shadow D6's "id" fallback (priority: node > id > caller arg).
    let has_node_or_id = v.get("node").and_then(|n| n.as_str()).is_some()
        || v.get("id").and_then(|n| n.as_str()).is_some();
    if !has_node_or_id {
        if let Some(obj) = v.as_object_mut() {
            obj.insert("node".to_owned(), Value::String(id.to_owned()));
        }
    }

    // 3. Synthesize "summary" from "title" when absent
    let has_summary = v
        .get("summary")
        .and_then(|s| s.as_str())
        .is_some_and(|s| !s.is_empty());
    if !has_summary {
        if let Some(title) = v.get("title").and_then(|t| t.as_str()).map(str::to_owned) {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("summary".to_owned(), Value::String(title));
            }
        }
    }

    Ok(v)
}

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

    // Step 1b: enrichment — inject type/node from CLI args; synthesize summary
    // from title. This runs unconditionally before schema check so the validator
    // sees a fully-formed object even when the caller omits AGM identity fields.
    let value = enrich_for_ingest(&node_type, id, value)?;

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

    // Prefer explicit "summary"; fall back to "title" (defense in depth).
    let summary = v
        .get("summary")
        .and_then(|v| v.as_str())
        .or_else(|| v.get("title").and_then(|v| v.as_str()));
    if let Some(s) = summary {
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

    // -----------------------------------------------------------------------
    // Enrichment tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_ingest_one_synthesizes_summary_from_title_when_missing() {
        // The plan §3 example: no "summary", no "type", no "node" in JSON.
        let v = json!({
            "title": "x",
            "description": "d",
            "priority": "high"
        });
        let config = IngestConfig {
            schema_check: true,
            ..permissive_config()
        };
        let node = ingest_one(NodeType::Ticket, "t.x", v, &config).unwrap();
        assert_eq!(node.summary, "x");
    }

    #[test]
    fn test_ingest_one_explicit_summary_wins_over_title() {
        let v = json!({
            "summary": "explicit summary",
            "title": "Title That Should Not Win",
            "description": "d",
            "priority": "normal"
        });
        let config = IngestConfig {
            schema_check: true,
            ..permissive_config()
        };
        let node = ingest_one(NodeType::Ticket, "t.x", v, &config).unwrap();
        assert_eq!(node.summary, "explicit summary");
    }

    #[test]
    fn test_ingest_one_type_mismatch_returns_schema_check_error() {
        // JSON says "workflow" but CLI arg is Ticket — must return a clear error.
        let v = json!({
            "type": "workflow",
            "title": "T",
            "description": "d",
            "priority": "normal"
        });
        let err = ingest_one(NodeType::Ticket, "t.x", v, &permissive_config()).unwrap_err();
        match err {
            IngestError::SchemaCheck(msg) => {
                assert!(
                    msg.contains("type mismatch"),
                    "expected 'type mismatch' in error, got: {msg}"
                );
                assert!(
                    msg.contains("workflow"),
                    "error must mention conflicting type"
                );
                assert!(msg.contains("ticket"), "error must mention expected type");
            }
            other => panic!("expected SchemaCheck, got: {other}"),
        }
    }

    #[test]
    fn test_enrich_for_ingest_injects_type_and_node_and_summary() {
        let v = json!({
            "title": "My Title",
            "description": "desc"
        });
        let enriched = enrich_for_ingest(&NodeType::Facts, "pkg.facts", v).unwrap();
        assert_eq!(enriched["type"].as_str(), Some("facts"));
        assert_eq!(enriched["node"].as_str(), Some("pkg.facts"));
        assert_eq!(enriched["summary"].as_str(), Some("My Title"));
    }

    #[test]
    fn test_enrich_for_ingest_preserves_existing_node_field() {
        let v = json!({"node": "explicit.id", "summary": "s"});
        let enriched = enrich_for_ingest(&NodeType::Facts, "fallback.id", v).unwrap();
        assert_eq!(enriched["node"].as_str(), Some("explicit.id"));
    }

    #[test]
    fn test_enrich_for_ingest_does_not_inject_node_when_id_present() {
        // JSON has "id" but no "node". Enrichment must not inject "node",
        // so D6 can pick up "id" → "t.from-id" as the resolved id.
        let v = json!({"id": "t.from-id", "summary": "s"});
        let enriched = enrich_for_ingest(&NodeType::Facts, "fallback.id", v).unwrap();
        assert!(
            enriched.get("node").is_none(),
            "node must not be injected when 'id' is present"
        );
    }

    #[test]
    fn test_enrich_for_ingest_type_match_does_not_error() {
        let v = json!({"type": "ticket", "summary": "s"});
        let enriched = enrich_for_ingest(&NodeType::Ticket, "t.x", v).unwrap();
        assert_eq!(enriched["type"].as_str(), Some("ticket"));
    }
}
