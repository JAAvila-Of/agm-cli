//! JSON canonical form renderer and reverse conversion (spec S37).
//!
//! `agm_to_json` — forward conversion: `AgmFile` -> `serde_json::Value`
//! `json_to_agm` — reverse conversion: `serde_json::Value` -> `AgmFile`

use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;
use std::str::FromStr;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::model::code::CodeBlock;
use crate::model::context::AgentContext;
use crate::model::execution::ExecutionStatus;
use crate::model::fields::{
    Confidence, FieldValue, NodeStatus, NodeType, Priority, Span, Stability,
};
use crate::model::file::{AgmFile, Header, LoadProfile, TokenEstimate};
use crate::model::imports::ImportEntry;
use crate::model::memory::MemoryEntry;
use crate::model::node::Node;
use crate::model::orchestration::ParallelGroup;
use crate::model::verify::VerifyCheck;

use super::RenderError;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Converts an `AgmFile` to the spec S37 JSON canonical form.
///
/// The returned value uses type coercions:
/// - `agm` is an integer (not a string)
/// - `imports` are strings in `"pkg@constraint"` format
/// - Boolean/number-like scalars in `extra_fields` are coerced
#[must_use]
pub fn agm_to_json(file: &AgmFile) -> Value {
    let mut map = Map::new();

    // Header
    map.insert("agm".into(), convert_agm_version(&file.header.agm));
    map.insert("package".into(), Value::String(file.header.package.clone()));
    map.insert("version".into(), Value::String(file.header.version.clone()));

    insert_opt_str(&mut map, "title", &file.header.title);
    insert_opt_str(&mut map, "owner", &file.header.owner);

    if let Some(ref imports) = file.header.imports {
        map.insert("imports".into(), convert_imports(imports));
    }

    insert_opt_str(&mut map, "default_load", &file.header.default_load);
    insert_opt_str(&mut map, "description", &file.header.description);

    if let Some(ref tags) = file.header.tags {
        map.insert("tags".into(), str_list_to_value(tags));
    }

    insert_opt_str(&mut map, "status", &file.header.status);

    if let Some(ref profiles) = file.header.load_profiles {
        let obj: Map<String, Value> = profiles
            .iter()
            .map(|(k, v)| (k.clone(), load_profile_to_json(v)))
            .collect();
        map.insert("load_profiles".into(), Value::Object(obj));
    }

    insert_opt_str(&mut map, "target_runtime", &file.header.target_runtime);

    // Nodes
    let nodes: Vec<Value> = file.nodes.iter().map(node_to_json).collect();
    map.insert("nodes".into(), Value::Array(nodes));

    Value::Object(map)
}

/// Renders the canonical JSON form as a pretty-printed string.
#[must_use]
pub fn render_json_canonical(file: &AgmFile) -> String {
    serde_json::to_string_pretty(&agm_to_json(file))
        .expect("canonical Value is always serializable")
}

/// Converts a spec S37 JSON value back into an `AgmFile`.
///
/// Returns an error if required fields are missing or have invalid types.
pub fn json_to_agm(json: &Value) -> Result<AgmFile, RenderError> {
    let obj = json.as_object().ok_or_else(|| RenderError::InvalidType {
        field: "<root>".into(),
        expected: "object".into(),
        actual: json_type_name(json).into(),
    })?;

    // Header
    let agm = get_agm_string(obj)?;
    let package = get_required_str(obj, "package")?;
    let version = get_required_str(obj, "version")?;
    let title = get_opt_str(obj, "title")?;
    let owner = get_opt_str(obj, "owner")?;

    let imports = if let Some(v) = obj.get("imports") {
        Some(parse_imports(v)?)
    } else {
        None
    };

    let default_load = get_opt_str(obj, "default_load")?;
    let description = get_opt_str(obj, "description")?;
    let tags = get_opt_str_list(obj, "tags")?;
    let status = get_opt_str(obj, "status")?;

    let load_profiles = if let Some(v) = obj.get("load_profiles") {
        Some(parse_load_profiles(v)?)
    } else {
        None
    };

    let target_runtime = get_opt_str(obj, "target_runtime")?;

    let header = Header {
        agm,
        package,
        version,
        title,
        owner,
        imports,
        default_load,
        description,
        tags,
        status,
        load_profiles,
        target_runtime,
    };

    // Nodes
    let nodes_val = obj.get("nodes").ok_or_else(|| RenderError::MissingField {
        field: "nodes".into(),
    })?;
    let nodes_arr = nodes_val
        .as_array()
        .ok_or_else(|| RenderError::InvalidType {
            field: "nodes".into(),
            expected: "array".into(),
            actual: json_type_name(nodes_val).into(),
        })?;

    let nodes: Vec<Node> = nodes_arr
        .iter()
        .enumerate()
        .map(|(i, v)| json_to_node(v, i))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AgmFile { header, nodes })
}

// ---------------------------------------------------------------------------
// Known node field names (used to separate known fields from extra_fields)
// ---------------------------------------------------------------------------

fn known_node_fields() -> HashSet<&'static str> {
    [
        "node",
        "type",
        "summary",
        "priority",
        "stability",
        "confidence",
        "status",
        "depends",
        "related_to",
        "replaces",
        "conflicts",
        "see_also",
        "items",
        "steps",
        "fields",
        "input",
        "output",
        "detail",
        "rationale",
        "tradeoffs",
        "resolution",
        "examples",
        "notes",
        "code",
        "code_blocks",
        "verify",
        "agent_context",
        "target",
        "execution_status",
        "executed_by",
        "executed_at",
        "execution_log",
        "retry_count",
        "parallel_groups",
        "memory",
        "scope",
        "applies_when",
        "valid_from",
        "valid_until",
        "tags",
        "aliases",
        "keywords",
    ]
    .into_iter()
    .collect()
}

// ---------------------------------------------------------------------------
// Forward conversion helpers
// ---------------------------------------------------------------------------

fn convert_agm_version(s: &str) -> Value {
    if let Ok(n) = s.parse::<u64>() {
        return Value::Number(n.into());
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Number((f as u64).into());
    }
    Value::String(s.to_owned())
}

fn convert_imports(imports: &[ImportEntry]) -> Value {
    let arr: Vec<Value> = imports
        .iter()
        .map(|e| Value::String(e.to_string()))
        .collect();
    Value::Array(arr)
}

fn load_profile_to_json(lp: &LoadProfile) -> Value {
    let mut map = Map::new();
    map.insert("filter".into(), Value::String(lp.filter.clone()));
    if let Some(ref est) = lp.estimated_tokens {
        let v = match est {
            TokenEstimate::Count(n) => Value::Number((*n).into()),
            TokenEstimate::Variable => Value::String("variable".into()),
        };
        map.insert("estimated_tokens".into(), v);
    }
    Value::Object(map)
}

fn node_to_json(node: &Node) -> Value {
    let mut map = Map::new();

    // Identity
    map.insert("node".into(), Value::String(node.id.clone()));
    map.insert("type".into(), Value::String(node.node_type.to_string()));
    map.insert("summary".into(), Value::String(node.summary.clone()));

    // Control
    insert_opt_display(&mut map, "priority", &node.priority);
    insert_opt_display(&mut map, "stability", &node.stability);
    insert_opt_display(&mut map, "confidence", &node.confidence);
    insert_opt_display(&mut map, "status", &node.status);

    // Relationships
    insert_opt_list(&mut map, "depends", &node.depends);
    insert_opt_list(&mut map, "related_to", &node.related_to);
    insert_opt_list(&mut map, "replaces", &node.replaces);
    insert_opt_list(&mut map, "conflicts", &node.conflicts);
    insert_opt_list(&mut map, "see_also", &node.see_also);

    // I/O
    insert_opt_list(&mut map, "input", &node.input);
    insert_opt_list(&mut map, "output", &node.output);

    // Operational
    insert_opt_list(&mut map, "items", &node.items);
    insert_opt_list(&mut map, "steps", &node.steps);
    insert_opt_list(&mut map, "fields", &node.fields);

    // Executable
    insert_opt_serde(&mut map, "code", &node.code);
    insert_opt_serde_vec(&mut map, "code_blocks", &node.code_blocks);
    insert_opt_serde_vec(&mut map, "verify", &node.verify);
    insert_opt_serde(&mut map, "agent_context", &node.agent_context);
    insert_opt_str(&mut map, "target", &node.target);

    // Memory
    insert_opt_serde_vec(&mut map, "memory", &node.memory);

    // Execution state
    insert_opt_display(&mut map, "execution_status", &node.execution_status);
    insert_opt_str(&mut map, "executed_by", &node.executed_by);
    insert_opt_str(&mut map, "executed_at", &node.executed_at);
    insert_opt_str(&mut map, "execution_log", &node.execution_log);
    insert_opt_u32(&mut map, "retry_count", &node.retry_count);

    // Orchestration
    insert_opt_serde_vec(&mut map, "parallel_groups", &node.parallel_groups);

    // Explanatory
    insert_opt_str(&mut map, "detail", &node.detail);
    insert_opt_list(&mut map, "rationale", &node.rationale);
    insert_opt_list(&mut map, "tradeoffs", &node.tradeoffs);
    insert_opt_list(&mut map, "resolution", &node.resolution);
    insert_opt_str(&mut map, "examples", &node.examples);
    insert_opt_str(&mut map, "notes", &node.notes);

    // Context
    insert_opt_list(&mut map, "scope", &node.scope);
    insert_opt_str(&mut map, "applies_when", &node.applies_when);
    insert_opt_str(&mut map, "valid_from", &node.valid_from);
    insert_opt_str(&mut map, "valid_until", &node.valid_until);
    insert_opt_list(&mut map, "tags", &node.tags);
    insert_opt_list(&mut map, "aliases", &node.aliases);
    insert_opt_list(&mut map, "keywords", &node.keywords);

    // Extension (extra_fields) — with scalar coercion
    for (k, v) in &node.extra_fields {
        let json_val = match v {
            FieldValue::Scalar(s) => coerce_scalar(s),
            FieldValue::List(items) => str_list_to_value(items),
            FieldValue::Block(s) => Value::String(s.clone()),
        };
        map.insert(k.clone(), json_val);
    }

    Value::Object(map)
}

// ---------------------------------------------------------------------------
// Forward helper functions
// ---------------------------------------------------------------------------

fn insert_opt_str(map: &mut Map<String, Value>, key: &str, val: &Option<String>) {
    if let Some(s) = val {
        map.insert(key.into(), Value::String(s.clone()));
    }
}

fn insert_opt_list(map: &mut Map<String, Value>, key: &str, val: &Option<Vec<String>>) {
    if let Some(v) = val {
        map.insert(key.into(), str_list_to_value(v));
    }
}

fn insert_opt_display<T: Display>(map: &mut Map<String, Value>, key: &str, val: &Option<T>) {
    if let Some(v) = val {
        map.insert(key.into(), Value::String(v.to_string()));
    }
}

fn insert_opt_serde<T: Serialize>(map: &mut Map<String, Value>, key: &str, val: &Option<T>) {
    if let Some(v) = val {
        if let Ok(json_val) = serde_json::to_value(v) {
            map.insert(key.into(), json_val);
        }
    }
}

fn insert_opt_serde_vec<T: Serialize>(
    map: &mut Map<String, Value>,
    key: &str,
    val: &Option<Vec<T>>,
) {
    if let Some(v) = val {
        if let Ok(json_val) = serde_json::to_value(v) {
            map.insert(key.into(), json_val);
        }
    }
}

fn insert_opt_u32(map: &mut Map<String, Value>, key: &str, val: &Option<u32>) {
    if let Some(n) = val {
        map.insert(key.into(), Value::Number((*n).into()));
    }
}

fn str_list_to_value(items: &[String]) -> Value {
    Value::Array(items.iter().map(|s| Value::String(s.clone())).collect())
}

fn coerce_scalar(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                Value::Number(n.into())
            } else {
                Value::String(s.to_owned())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reverse conversion helpers
// ---------------------------------------------------------------------------

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn get_agm_string(obj: &Map<String, Value>) -> Result<String, RenderError> {
    let v = obj.get("agm").ok_or_else(|| RenderError::MissingField {
        field: "agm".into(),
    })?;
    match v {
        Value::Number(n) => {
            // Integer agm values (e.g. 1) must be formatted as MAJOR.MINOR (e.g. "1.0")
            // to satisfy the parser's format requirement.
            if let Some(i) = n.as_u64() {
                if n.as_f64().is_some_and(|f| f == i as f64) {
                    return Ok(format!("{i}.0"));
                }
            }
            Ok(n.to_string())
        }
        Value::String(s) => Ok(s.clone()),
        other => Err(RenderError::InvalidType {
            field: "agm".into(),
            expected: "number or string".into(),
            actual: json_type_name(other).into(),
        }),
    }
}

fn get_required_str(obj: &Map<String, Value>, key: &str) -> Result<String, RenderError> {
    let v = obj
        .get(key)
        .ok_or_else(|| RenderError::MissingField { field: key.into() })?;
    v.as_str()
        .map(|s| s.to_owned())
        .ok_or_else(|| RenderError::InvalidType {
            field: key.into(),
            expected: "string".into(),
            actual: json_type_name(v).into(),
        })
}

fn get_opt_str(obj: &Map<String, Value>, key: &str) -> Result<Option<String>, RenderError> {
    match obj.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(RenderError::InvalidType {
            field: key.into(),
            expected: "string".into(),
            actual: json_type_name(other).into(),
        }),
    }
}

fn get_opt_str_list(
    obj: &Map<String, Value>,
    key: &str,
) -> Result<Option<Vec<String>>, RenderError> {
    match obj.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(Value::Array(arr)) => {
            let mut result = Vec::with_capacity(arr.len());
            for (i, item) in arr.iter().enumerate() {
                match item {
                    Value::String(s) => result.push(s.clone()),
                    other => {
                        return Err(RenderError::InvalidType {
                            field: format!("{key}[{i}]"),
                            expected: "string".into(),
                            actual: json_type_name(other).into(),
                        });
                    }
                }
            }
            Ok(Some(result))
        }
        Some(other) => Err(RenderError::InvalidType {
            field: key.into(),
            expected: "array".into(),
            actual: json_type_name(other).into(),
        }),
    }
}

fn get_opt_enum<T: FromStr>(obj: &Map<String, Value>, key: &str) -> Result<Option<T>, RenderError>
where
    T::Err: Display,
{
    match obj.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            s.parse::<T>()
                .map(Some)
                .map_err(|e| RenderError::InvalidEnumValue {
                    field: key.into(),
                    value: format!("{s}: {e}"),
                })
        }
        Some(other) => Err(RenderError::InvalidType {
            field: key.into(),
            expected: "string".into(),
            actual: json_type_name(other).into(),
        }),
    }
}

fn get_opt_u32(obj: &Map<String, Value>, key: &str) -> Result<Option<u32>, RenderError> {
    match obj.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .map(Some)
            .ok_or_else(|| RenderError::InvalidType {
                field: key.into(),
                expected: "u32".into(),
                actual: "number out of range".into(),
            }),
        Some(other) => Err(RenderError::InvalidType {
            field: key.into(),
            expected: "number".into(),
            actual: json_type_name(other).into(),
        }),
    }
}

fn parse_imports(v: &Value) -> Result<Vec<ImportEntry>, RenderError> {
    let arr = v.as_array().ok_or_else(|| RenderError::InvalidType {
        field: "imports".into(),
        expected: "array".into(),
        actual: json_type_name(v).into(),
    })?;

    let mut result = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let s = item.as_str().ok_or_else(|| RenderError::InvalidType {
            field: format!("imports[{i}]"),
            expected: "string".into(),
            actual: json_type_name(item).into(),
        })?;
        let entry = s
            .parse::<ImportEntry>()
            .map_err(|e| RenderError::JsonError(e.to_string()))?;
        result.push(entry);
    }
    Ok(result)
}

fn parse_load_profiles(v: &Value) -> Result<BTreeMap<String, LoadProfile>, RenderError> {
    let obj = v.as_object().ok_or_else(|| RenderError::InvalidType {
        field: "load_profiles".into(),
        expected: "object".into(),
        actual: json_type_name(v).into(),
    })?;

    let mut result = BTreeMap::new();
    for (k, val) in obj {
        let lp: LoadProfile = serde_json::from_value(val.clone())
            .map_err(|e| RenderError::JsonError(e.to_string()))?;
        result.insert(k.clone(), lp);
    }
    Ok(result)
}

fn json_to_node(v: &Value, index: usize) -> Result<Node, RenderError> {
    let obj = v.as_object().ok_or_else(|| RenderError::InvalidType {
        field: format!("nodes[{index}]"),
        expected: "object".into(),
        actual: json_type_name(v).into(),
    })?;

    let id = get_required_str(obj, "node")?;
    let type_str = get_required_str(obj, "type")?;
    let node_type = type_str
        .parse::<NodeType>()
        .expect("NodeType::from_str is infallible");
    let summary = get_required_str(obj, "summary")?;

    let priority = get_opt_enum::<Priority>(obj, "priority")?;
    let stability = get_opt_enum::<Stability>(obj, "stability")?;
    let confidence = get_opt_enum::<Confidence>(obj, "confidence")?;
    let status = get_opt_enum::<NodeStatus>(obj, "status")?;

    let depends = get_opt_str_list(obj, "depends")?;
    let related_to = get_opt_str_list(obj, "related_to")?;
    let replaces = get_opt_str_list(obj, "replaces")?;
    let conflicts = get_opt_str_list(obj, "conflicts")?;
    let see_also = get_opt_str_list(obj, "see_also")?;

    let items = get_opt_str_list(obj, "items")?;
    let steps = get_opt_str_list(obj, "steps")?;
    let fields = get_opt_str_list(obj, "fields")?;
    let input = get_opt_str_list(obj, "input")?;
    let output = get_opt_str_list(obj, "output")?;

    let detail = get_opt_str(obj, "detail")?;
    let rationale = get_opt_str_list(obj, "rationale")?;
    let tradeoffs = get_opt_str_list(obj, "tradeoffs")?;
    let resolution = get_opt_str_list(obj, "resolution")?;
    let examples = get_opt_str(obj, "examples")?;
    let notes = get_opt_str(obj, "notes")?;

    let code = parse_opt_serde::<CodeBlock>(obj, "code")?;
    let code_blocks = parse_opt_serde_vec::<CodeBlock>(obj, "code_blocks")?;
    let verify = parse_opt_serde_vec::<VerifyCheck>(obj, "verify")?;
    let agent_context = parse_opt_serde::<AgentContext>(obj, "agent_context")?;
    let target = get_opt_str(obj, "target")?;

    let execution_status = get_opt_enum::<ExecutionStatus>(obj, "execution_status")?;
    let executed_by = get_opt_str(obj, "executed_by")?;
    let executed_at = get_opt_str(obj, "executed_at")?;
    let execution_log = get_opt_str(obj, "execution_log")?;
    let retry_count = get_opt_u32(obj, "retry_count")?;

    let parallel_groups = parse_opt_serde_vec::<ParallelGroup>(obj, "parallel_groups")?;
    let memory = parse_opt_serde_vec::<MemoryEntry>(obj, "memory")?;

    let scope = get_opt_str_list(obj, "scope")?;
    let applies_when = get_opt_str(obj, "applies_when")?;
    let valid_from = get_opt_str(obj, "valid_from")?;
    let valid_until = get_opt_str(obj, "valid_until")?;
    let tags = get_opt_str_list(obj, "tags")?;
    let aliases = get_opt_str_list(obj, "aliases")?;
    let keywords = get_opt_str_list(obj, "keywords")?;

    // Extra fields: any key not in the known set
    let known = known_node_fields();
    let mut extra_fields = BTreeMap::new();
    for (k, val) in obj {
        if !known.contains(k.as_str()) {
            let fv = match val {
                Value::Bool(b) => FieldValue::Scalar(b.to_string()),
                Value::Number(n) => FieldValue::Scalar(n.to_string()),
                Value::String(s) => FieldValue::Scalar(s.clone()),
                Value::Array(arr) => {
                    let items: Vec<String> = arr
                        .iter()
                        .map(|item| match item {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect();
                    FieldValue::List(items)
                }
                other => FieldValue::Scalar(other.to_string()),
            };
            extra_fields.insert(k.clone(), fv);
        }
    }

    Ok(Node {
        id,
        node_type,
        summary,
        priority,
        stability,
        confidence,
        status,
        depends,
        related_to,
        replaces,
        conflicts,
        see_also,
        items,
        steps,
        fields,
        input,
        output,
        detail,
        rationale,
        tradeoffs,
        resolution,
        examples,
        notes,
        code,
        code_blocks,
        verify,
        agent_context,
        target,
        execution_status,
        executed_by,
        executed_at,
        execution_log,
        retry_count,
        parallel_groups,
        memory,
        scope,
        applies_when,
        valid_from,
        valid_until,
        tags,
        aliases,
        keywords,
        extra_fields,
        span: Span::default(),
        // Ticket fields: parsed via serde from extra_fields above (v1.2.0)
        ..Default::default()
    })
}

fn parse_opt_serde<T: serde::de::DeserializeOwned>(
    obj: &Map<String, Value>,
    key: &str,
) -> Result<Option<T>, RenderError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value::<T>(v.clone())
            .map(Some)
            .map_err(|e| RenderError::JsonError(e.to_string())),
    }
}

fn parse_opt_serde_vec<T: serde::de::DeserializeOwned>(
    obj: &Map<String, Value>,
    key: &str,
) -> Result<Option<Vec<T>>, RenderError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value::<Vec<T>>(v.clone())
            .map(Some)
            .map_err(|e| RenderError::JsonError(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::code::{CodeAction, CodeBlock};
    use crate::model::memory::{MemoryAction, MemoryEntry};
    use crate::model::verify::VerifyCheck;

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
                ..Default::default()
            }],
        }
    }

    #[test]
    fn test_agm_to_json_agm_field_is_integer() {
        let file = minimal_file();
        let json = agm_to_json(&file);
        let agm_val = &json["agm"];
        assert!(agm_val.is_number(), "expected number, got: {agm_val:?}");
        assert_eq!(agm_val, &Value::Number(1u64.into()));
    }

    #[test]
    fn test_agm_to_json_agm_float_string_truncates_to_integer() {
        let mut file = minimal_file();
        file.header.agm = "1.0".to_owned();
        let json = agm_to_json(&file);
        assert_eq!(json["agm"], Value::Number(1u64.into()));
    }

    #[test]
    fn test_agm_to_json_imports_as_strings() {
        let mut file = minimal_file();
        file.header.imports = Some(vec![
            ImportEntry::new("shared.security".into(), Some("^1.0.0".into())),
            ImportEntry::new("core.utils".into(), None),
        ]);
        let json = agm_to_json(&file);
        let imports = json["imports"].as_array().unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0], Value::String("shared.security@^1.0.0".into()));
        assert_eq!(imports[1], Value::String("core.utils".into()));
    }

    #[test]
    fn test_agm_to_json_omits_none_fields() {
        let file = minimal_file();
        let json = agm_to_json(&file);
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("title"));
        assert!(!obj.contains_key("owner"));
        assert!(!obj.contains_key("imports"));
        assert!(!obj.contains_key("description"));
        let node_obj = &json["nodes"][0];
        assert!(!node_obj.as_object().unwrap().contains_key("priority"));
        assert!(!node_obj.as_object().unwrap().contains_key("depends"));
    }

    #[test]
    fn test_agm_to_json_extra_fields_coerced() {
        let mut file = minimal_file();
        file.nodes[0]
            .extra_fields
            .insert("is_active".into(), FieldValue::Scalar("true".into()));
        file.nodes[0]
            .extra_fields
            .insert("count".into(), FieldValue::Scalar("42".into()));
        file.nodes[0]
            .extra_fields
            .insert("label".into(), FieldValue::Scalar("hello".into()));
        let json = agm_to_json(&file);
        let node = &json["nodes"][0];
        assert_eq!(node["is_active"], Value::Bool(true));
        assert_eq!(node["count"], Value::Number(42i64.into()));
        assert_eq!(node["label"], Value::String("hello".into()));
    }

    #[test]
    fn test_agm_to_json_code_block_as_object() {
        let mut file = minimal_file();
        file.nodes[0].code = Some(CodeBlock {
            lang: Some("rust".into()),
            target: Some("src/main.rs".into()),
            action: CodeAction::Create,
            body: "fn main() {}".into(),
            anchor: None,
            old: None,
        });
        let json = agm_to_json(&file);
        let code = &json["nodes"][0]["code"];
        assert!(code.is_object());
        assert_eq!(code["lang"], Value::String("rust".into()));
        assert_eq!(code["action"], Value::String("create".into()));
    }

    #[test]
    fn test_agm_to_json_verify_checks_as_array() {
        let mut file = minimal_file();
        file.nodes[0].verify = Some(vec![VerifyCheck::Command {
            run: "cargo check".into(),
            expect: Some("exit_code_0".into()),
        }]);
        let json = agm_to_json(&file);
        let verify = &json["nodes"][0]["verify"];
        assert!(verify.is_array());
        assert_eq!(verify[0]["type"], Value::String("command".into()));
        assert_eq!(verify[0]["run"], Value::String("cargo check".into()));
    }

    #[test]
    fn test_agm_to_json_memory_as_array() {
        let mut file = minimal_file();
        file.nodes[0].memory = Some(vec![MemoryEntry {
            key: "repo.pattern".into(),
            topic: "rust.repository".into(),
            action: MemoryAction::Upsert,
            value: Some("row_to_column uses get()".into()),
            scope: None,
            ttl: None,
            query: None,
            max_results: None,
        }]);
        let json = agm_to_json(&file);
        let mem = &json["nodes"][0]["memory"];
        assert!(mem.is_array());
        assert_eq!(mem[0]["key"], Value::String("repo.pattern".into()));
        assert_eq!(mem[0]["action"], Value::String("upsert".into()));
    }

    #[test]
    fn test_json_to_agm_minimal_succeeds() {
        let json = serde_json::json!({
            "agm": 1,
            "package": "test.minimal",
            "version": "0.1.0",
            "nodes": [
                {
                    "node": "test.node",
                    "type": "facts",
                    "summary": "a minimal test node"
                }
            ]
        });
        let file = json_to_agm(&json).unwrap();
        assert_eq!(file.header.package, "test.minimal");
        assert_eq!(file.nodes.len(), 1);
        assert_eq!(file.nodes[0].id, "test.node");
    }

    #[test]
    fn test_json_to_agm_missing_package_returns_error() {
        let json = serde_json::json!({
            "agm": 1,
            "version": "0.1.0",
            "nodes": []
        });
        let err = json_to_agm(&json).unwrap_err();
        assert!(matches!(err, RenderError::MissingField { field } if field == "package"));
    }

    #[test]
    fn test_json_to_agm_agm_integer_to_string() {
        let json = serde_json::json!({
            "agm": 1,
            "package": "test.pkg",
            "version": "0.1.0",
            "nodes": [{"node": "n", "type": "facts", "summary": "s"}]
        });
        let file = json_to_agm(&json).unwrap();
        // Integer agm values are normalized to MAJOR.MINOR format for parser compatibility.
        assert_eq!(file.header.agm, "1.0");
    }

    #[test]
    fn test_json_to_agm_imports_parsed() {
        let json = serde_json::json!({
            "agm": 1,
            "package": "test.pkg",
            "version": "0.1.0",
            "imports": ["shared.security@^1.0.0", "core.utils"],
            "nodes": [{"node": "n", "type": "facts", "summary": "s"}]
        });
        let file = json_to_agm(&json).unwrap();
        let imports = file.header.imports.unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].package, "shared.security");
        assert_eq!(imports[0].version_constraint.as_deref(), Some("^1.0.0"));
        assert_eq!(imports[1].package, "core.utils");
    }

    #[test]
    fn test_json_to_agm_missing_nodes_returns_error() {
        let json = serde_json::json!({
            "agm": 1,
            "package": "test.pkg",
            "version": "0.1.0"
        });
        let err = json_to_agm(&json).unwrap_err();
        assert!(matches!(err, RenderError::MissingField { field } if field == "nodes"));
    }

    #[test]
    fn test_json_to_agm_roundtrip_preserves_data() {
        let file = minimal_file();
        let json = agm_to_json(&file);
        let restored = json_to_agm(&json).unwrap();
        assert_eq!(restored.header.package, file.header.package);
        assert_eq!(restored.nodes[0].id, file.nodes[0].id);
        assert_eq!(restored.nodes[0].summary, file.nodes[0].summary);
    }

    #[test]
    fn test_agm_to_json_nodes_always_present() {
        let mut file = minimal_file();
        file.nodes = vec![];
        let json = agm_to_json(&file);
        // nodes key should exist
        assert!(json.as_object().unwrap().contains_key("nodes"));
        assert_eq!(json["nodes"], Value::Array(vec![]));
    }
}
