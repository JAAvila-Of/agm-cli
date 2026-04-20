//! Per-type `*_from_json` helpers.
//!
//! Each function:
//! 1. Walks the JSON object, routing every recognized field to the corresponding
//!    builder setter.
//! 2. Parses enum-valued string fields via `FromStr`; returns `IngestError::SchemaCheck`
//!    on failure.
//! 3. Routes unknown fields through `.extra(key, FieldValue)`.
//! 4. Skips the identity fields `"node"`, `"id"`, `"type"` — those come from
//!    caller arguments, not the JSON body.

use serde_json::Value;

use crate::builder::{
    DecisionBuilder, FactsBuilder, MemoryEntryBuilder, OrchestrationBuilder, RulesBuilder,
    TicketBuilder, WorkflowBuilder,
};
use crate::model::code::{CodeAction, CodeBlock};
use crate::model::context::{AgentContext, FileRange, LoadFile};
use crate::model::fields::{FieldValue, Priority, SddPhase, Stability, TicketAction};
use crate::model::memory::{MemoryAction, MemoryScope, MemoryTtl};
use crate::model::orchestration::{ParallelGroup, Strategy};
use crate::model::verify::VerifyCheck;

use super::IngestError;

// ---------------------------------------------------------------------------
// Identity fields — skipped by all builders
// ---------------------------------------------------------------------------

const IDENTITY_FIELDS: &[&str] = &["node", "id", "type"];

fn is_identity(key: &str) -> bool {
    IDENTITY_FIELDS.contains(&key)
}

// ---------------------------------------------------------------------------
// Helper: extract a string from a JSON Value
// ---------------------------------------------------------------------------

fn str_val<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|v| v.as_str())
}

fn string_list(v: &Value, key: &str) -> Option<Vec<String>> {
    v.get(key).and_then(|arr| {
        arr.as_array().map(|items| {
            items
                .iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
    })
}

/// Convert a serde_json::Value to a FieldValue for extra_fields.
fn value_to_field_value(v: &Value) -> FieldValue {
    match v {
        Value::String(s) => FieldValue::Scalar(s.clone()),
        Value::Array(arr) => FieldValue::List(
            arr.iter()
                .filter_map(|i| i.as_str().map(str::to_owned))
                .collect(),
        ),
        other => FieldValue::Block(other.to_string()),
    }
}

/// Parse enum `T` from a string field; map errors to `IngestError::SchemaCheck`.
macro_rules! parse_enum {
    ($v:expr, $key:expr, $T:ty) => {{
        let s = $v;
        s.parse::<$T>()
            .map_err(|e| IngestError::SchemaCheck(format!("field `{}`: {}", $key, e)))
    }};
}

// ---------------------------------------------------------------------------
// Sub-helpers: nested structures
// ---------------------------------------------------------------------------

fn parse_code_block(obj: &Value) -> Result<CodeBlock, IngestError> {
    let action_str = obj.get("action").and_then(|v| v.as_str()).unwrap_or("full");
    let action = parse_enum!(action_str, "action", CodeAction)?;
    let body = obj
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let lang = obj.get("lang").and_then(|v| v.as_str()).map(str::to_owned);
    let target = obj
        .get("target")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let anchor = obj
        .get("anchor")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let old = obj.get("old").and_then(|v| v.as_str()).map(str::to_owned);
    Ok(CodeBlock {
        lang,
        target,
        action,
        body,
        anchor,
        old,
    })
}

fn parse_verify_check(obj: &Value) -> Result<VerifyCheck, IngestError> {
    let type_str = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("command");
    match type_str {
        "command" => {
            let run = obj
                .get("run")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let expect = obj
                .get("expect")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            Ok(VerifyCheck::Command { run, expect })
        }
        "file_exists" => {
            let file = obj
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            Ok(VerifyCheck::FileExists { file })
        }
        "file_contains" => {
            let file = obj
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let pattern = obj
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            Ok(VerifyCheck::FileContains { file, pattern })
        }
        "file_not_contains" => {
            let file = obj
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let pattern = obj
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            Ok(VerifyCheck::FileNotContains { file, pattern })
        }
        "node_status" => {
            let node = obj
                .get("node")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let status = obj
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            Ok(VerifyCheck::NodeStatus { node, status })
        }
        other => Ok(VerifyCheck::Command {
            run: format!("# unknown verify type: {other}"),
            expect: None,
        }),
    }
}

fn parse_agent_context(v: &Value) -> AgentContext {
    let load_nodes = string_list(v, "load_nodes");
    let load_files = v.get("load_files").and_then(|arr| {
        arr.as_array().map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let path = item.get("path")?.as_str()?.to_owned();
                    let range = item
                        .get("range")
                        .map(|r| match r.as_str() {
                            Some("full") => FileRange::Full,
                            Some(s) if s.starts_with("function:") => FileRange::Function(
                                s.trim_start_matches("function:").trim().to_owned(),
                            ),
                            _ => FileRange::Full,
                        })
                        .unwrap_or(FileRange::Full);
                    Some(LoadFile { path, range })
                })
                .collect::<Vec<_>>()
        })
    });
    let system_hint = v
        .get("system_hint")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let max_tokens = v.get("max_tokens").and_then(|v| v.as_u64());
    let load_memory = string_list(v, "load_memory");
    AgentContext {
        load_nodes,
        load_files,
        system_hint,
        max_tokens,
        load_memory,
    }
}

fn parse_parallel_group(v: &Value) -> Result<ParallelGroup, IngestError> {
    let group = v
        .get("group")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let nodes = string_list(v, "nodes").unwrap_or_default();
    let strategy_str = v
        .get("strategy")
        .and_then(|v| v.as_str())
        .unwrap_or("sequential");
    let strategy = parse_enum!(strategy_str, "strategy", Strategy)?;
    let requires = string_list(v, "requires");
    let max_concurrency = v
        .get("max_concurrency")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);
    Ok(ParallelGroup {
        group,
        nodes,
        strategy,
        requires,
        max_concurrency,
    })
}

// ---------------------------------------------------------------------------
// Common fields helper: applies universal fields to any builder that has them
// ---------------------------------------------------------------------------

/// Parsed form of universal fields shared by every node builder type.
/// Only includes fields that are actually applied via builder setters.
/// Fields applied directly (notes, detail) or lacking builder setters
/// (confidence, status) are handled inline in each *_from_json function.
#[derive(Default)]
struct UniversalFields {
    pub stability: Option<Stability>,
    pub depends: Option<Vec<String>>,
    pub related_to: Option<Vec<String>>,
    pub replaces: Option<Vec<String>>,
    pub conflicts: Option<Vec<String>>,
    pub see_also: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

fn extract_universal(v: &Value) -> Result<UniversalFields, IngestError> {
    let stability = v
        .get("stability")
        .and_then(|v| v.as_str())
        .map(|s| parse_enum!(s, "stability", Stability))
        .transpose()?;
    let depends = string_list(v, "depends");
    let related_to = string_list(v, "related_to");
    let replaces = string_list(v, "replaces");
    let conflicts = string_list(v, "conflicts");
    let see_also = string_list(v, "see_also");
    let tags = string_list(v, "tags");
    Ok(UniversalFields {
        stability,
        depends,
        related_to,
        replaces,
        conflicts,
        see_also,
        tags,
    })
}

// ---------------------------------------------------------------------------
// ticket_from_json
// ---------------------------------------------------------------------------

/// Known fields for ticket nodes.
const TICKET_KNOWN: &[&str] = &[
    "node",
    "id",
    "type",
    "summary",
    "title",
    "description",
    "priority",
    "action",
    "sdd_phase",
    "labels",
    "prompt",
    "assignee",
    "ticket_id",
    "stability",
    "confidence",
    "status",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "tags",
    "notes",
    "detail",
    "code_blocks",
    "agent_context",
];

pub fn ticket_from_json(id: &str, v: &Value) -> Result<TicketBuilder, IngestError> {
    if !v.is_object() {
        return Err(IngestError::NotAnObject);
    }

    let mut b = TicketBuilder::new(id);

    if let Some(s) = str_val(v, "summary") {
        b = b.summary(s);
    }
    if let Some(s) = str_val(v, "title") {
        b = b.title(s);
    }
    if let Some(s) = str_val(v, "description") {
        b = b.description(s);
    }
    if let Some(s) = str_val(v, "priority") {
        b = b.priority(parse_enum!(s, "priority", Priority)?);
    }
    if let Some(s) = str_val(v, "action") {
        b = b.action(parse_enum!(s, "action", TicketAction)?);
    }
    if let Some(s) = str_val(v, "sdd_phase") {
        b = b.sdd_phase(parse_enum!(s, "sdd_phase", SddPhase)?);
    }
    if let Some(labels) = string_list(v, "labels") {
        b = b.labels(labels);
    }
    if let Some(s) = str_val(v, "prompt") {
        b = b.prompt(s);
    }
    if let Some(s) = str_val(v, "assignee") {
        b = b.assignee(s);
    }
    if let Some(s) = str_val(v, "ticket_id") {
        b = b.ticket_id(s);
    }
    if let Some(s) = str_val(v, "notes") {
        b = b.notes(s);
    }
    if let Some(s) = str_val(v, "detail") {
        b = b.detail(s);
    }

    let uni = extract_universal(v)?;
    if let Some(s) = uni.stability {
        b = b.stability(s);
    }
    if let Some(deps) = uni.depends {
        b = b.depends(deps);
    }
    if let Some(rel) = uni.related_to {
        b = b.related_to(rel);
    }
    if let Some(rep) = uni.replaces {
        b = b.replaces(rep);
    }
    if let Some(con) = uni.conflicts {
        b = b.conflicts(con);
    }
    if let Some(sa) = uni.see_also {
        b = b.see_also(sa);
    }
    if let Some(t) = uni.tags {
        b = b.tags(t);
    }

    // code_blocks
    if let Some(arr) = v.get("code_blocks").and_then(|v| v.as_array()) {
        let blocks: Result<Vec<CodeBlock>, _> = arr.iter().map(parse_code_block).collect();
        b = b.code_blocks(blocks?);
    }

    // agent_context
    if let Some(ctx_val) = v.get("agent_context") {
        b = b.agent_context(parse_agent_context(ctx_val));
    }

    // extra_fields: all unrecognized keys
    if let Some(obj) = v.as_object() {
        for (key, val) in obj {
            if !TICKET_KNOWN.contains(&key.as_str()) && !is_identity(key) {
                b = b.extra(key.clone(), value_to_field_value(val));
            }
        }
    }

    Ok(b)
}

// ---------------------------------------------------------------------------
// workflow_from_json
// ---------------------------------------------------------------------------

const WORKFLOW_KNOWN: &[&str] = &[
    "node",
    "id",
    "type",
    "summary",
    "steps",
    "input",
    "output",
    "code",
    "code_blocks",
    "verify",
    "agent_context",
    "target",
    "priority",
    "stability",
    "confidence",
    "status",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "tags",
    "notes",
    "detail",
];

pub fn workflow_from_json(id: &str, v: &Value) -> Result<WorkflowBuilder, IngestError> {
    if !v.is_object() {
        return Err(IngestError::NotAnObject);
    }

    let mut b = WorkflowBuilder::new(id);

    if let Some(s) = str_val(v, "summary") {
        b = b.summary(s);
    }
    if let Some(steps) = string_list(v, "steps") {
        b = b.steps(steps);
    }
    if let Some(input) = string_list(v, "input") {
        b = b.input(input);
    }
    if let Some(output) = string_list(v, "output") {
        b = b.output(output);
    }
    if let Some(s) = str_val(v, "target") {
        b = b.target(s);
    }
    if let Some(s) = str_val(v, "notes") {
        b = b.notes(s);
    }
    if let Some(s) = str_val(v, "detail") {
        b = b.detail(s);
    }

    let uni = extract_universal(v)?;
    if let Some(p) = v.get("priority").and_then(|v| v.as_str()) {
        b = b.priority(parse_enum!(p, "priority", Priority)?);
    }
    if let Some(s) = uni.stability {
        b = b.stability(s);
    }
    if let Some(deps) = uni.depends {
        b = b.depends(deps);
    }
    if let Some(rel) = uni.related_to {
        b = b.related_to(rel);
    }
    if let Some(rep) = uni.replaces {
        b = b.replaces(rep);
    }
    if let Some(con) = uni.conflicts {
        b = b.conflicts(con);
    }
    if let Some(sa) = uni.see_also {
        b = b.see_also(sa);
    }
    if let Some(t) = uni.tags {
        b = b.tags(t);
    }

    // code_blocks
    if let Some(arr) = v.get("code_blocks").and_then(|v| v.as_array()) {
        let blocks: Result<Vec<CodeBlock>, _> = arr.iter().map(parse_code_block).collect();
        b = b.code_blocks(blocks?);
    }

    // verify
    if let Some(arr) = v.get("verify").and_then(|v| v.as_array()) {
        let checks: Result<Vec<VerifyCheck>, _> = arr.iter().map(parse_verify_check).collect();
        b = b.verify(checks?);
    }

    // agent_context
    if let Some(ctx_val) = v.get("agent_context") {
        b = b.agent_context(parse_agent_context(ctx_val));
    }

    // extra_fields
    if let Some(obj) = v.as_object() {
        for (key, val) in obj {
            if !WORKFLOW_KNOWN.contains(&key.as_str()) && !is_identity(key) {
                b = b.extra(key.clone(), value_to_field_value(val));
            }
        }
    }

    Ok(b)
}

// ---------------------------------------------------------------------------
// orchestration_from_json
// ---------------------------------------------------------------------------

const ORCHESTRATION_KNOWN: &[&str] = &[
    "node",
    "id",
    "type",
    "summary",
    "parallel_groups",
    "detail",
    "notes",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "tags",
    "stability",
    "confidence",
    "status",
];

pub fn orchestration_from_json(id: &str, v: &Value) -> Result<OrchestrationBuilder, IngestError> {
    if !v.is_object() {
        return Err(IngestError::NotAnObject);
    }

    let mut b = OrchestrationBuilder::new(id);

    if let Some(s) = str_val(v, "summary") {
        b = b.summary(s);
    }
    if let Some(s) = str_val(v, "detail") {
        b = b.detail(s);
    }
    if let Some(s) = str_val(v, "notes") {
        b = b.notes(s);
    }

    // parallel_groups
    if let Some(arr) = v.get("parallel_groups").and_then(|v| v.as_array()) {
        let groups: Result<Vec<ParallelGroup>, _> = arr.iter().map(parse_parallel_group).collect();
        b = b.parallel_groups(groups?);
    }

    let uni = extract_universal(v)?;
    if let Some(s) = uni.stability {
        b = b.stability(s);
    }
    if let Some(deps) = uni.depends {
        b = b.depends(deps);
    }
    if let Some(rel) = uni.related_to {
        b = b.related_to(rel);
    }
    if let Some(rep) = uni.replaces {
        b = b.replaces(rep);
    }
    if let Some(con) = uni.conflicts {
        b = b.conflicts(con);
    }
    if let Some(sa) = uni.see_also {
        b = b.see_also(sa);
    }
    if let Some(t) = uni.tags {
        b = b.tags(t);
    }

    // extra_fields
    if let Some(obj) = v.as_object() {
        for (key, val) in obj {
            if !ORCHESTRATION_KNOWN.contains(&key.as_str()) && !is_identity(key) {
                b = b.extra(key.clone(), value_to_field_value(val));
            }
        }
    }

    Ok(b)
}

// ---------------------------------------------------------------------------
// facts_from_json
// ---------------------------------------------------------------------------

const FACTS_KNOWN: &[&str] = &[
    "node",
    "id",
    "type",
    "summary",
    "items",
    "detail",
    "stability",
    "notes",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "tags",
    "confidence",
    "status",
];

pub fn facts_from_json(id: &str, v: &Value) -> Result<FactsBuilder, IngestError> {
    if !v.is_object() {
        return Err(IngestError::NotAnObject);
    }

    let mut b = FactsBuilder::new(id);

    if let Some(s) = str_val(v, "summary") {
        b = b.summary(s);
    }
    if let Some(items) = string_list(v, "items") {
        b = b.items(items);
    }
    if let Some(s) = str_val(v, "detail") {
        b = b.detail(s);
    }
    if let Some(s) = str_val(v, "notes") {
        b = b.notes(s);
    }

    let uni = extract_universal(v)?;
    if let Some(s) = uni.stability {
        b = b.stability(s);
    }
    if let Some(deps) = uni.depends {
        b = b.depends(deps);
    }
    if let Some(rel) = uni.related_to {
        b = b.related_to(rel);
    }
    if let Some(rep) = uni.replaces {
        b = b.replaces(rep);
    }
    if let Some(con) = uni.conflicts {
        b = b.conflicts(con);
    }
    if let Some(sa) = uni.see_also {
        b = b.see_also(sa);
    }
    if let Some(t) = uni.tags {
        b = b.tags(t);
    }

    // extra_fields
    if let Some(obj) = v.as_object() {
        for (key, val) in obj {
            if !FACTS_KNOWN.contains(&key.as_str()) && !is_identity(key) {
                b = b.extra(key.clone(), value_to_field_value(val));
            }
        }
    }

    Ok(b)
}

// ---------------------------------------------------------------------------
// rules_from_json
// ---------------------------------------------------------------------------

const RULES_KNOWN: &[&str] = &[
    "node",
    "id",
    "type",
    "summary",
    "items",
    "detail",
    "stability",
    "notes",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "tags",
    "confidence",
    "status",
];

pub fn rules_from_json(id: &str, v: &Value) -> Result<RulesBuilder, IngestError> {
    if !v.is_object() {
        return Err(IngestError::NotAnObject);
    }

    let mut b = RulesBuilder::new(id);

    if let Some(s) = str_val(v, "summary") {
        b = b.summary(s);
    }
    if let Some(items) = string_list(v, "items") {
        b = b.items(items);
    }
    if let Some(s) = str_val(v, "detail") {
        b = b.detail(s);
    }
    if let Some(s) = str_val(v, "notes") {
        b = b.notes(s);
    }

    let uni = extract_universal(v)?;
    if let Some(s) = uni.stability {
        b = b.stability(s);
    }
    if let Some(deps) = uni.depends {
        b = b.depends(deps);
    }
    if let Some(rel) = uni.related_to {
        b = b.related_to(rel);
    }
    if let Some(rep) = uni.replaces {
        b = b.replaces(rep);
    }
    if let Some(con) = uni.conflicts {
        b = b.conflicts(con);
    }
    if let Some(sa) = uni.see_also {
        b = b.see_also(sa);
    }
    if let Some(t) = uni.tags {
        b = b.tags(t);
    }

    // extra_fields
    if let Some(obj) = v.as_object() {
        for (key, val) in obj {
            if !RULES_KNOWN.contains(&key.as_str()) && !is_identity(key) {
                b = b.extra(key.clone(), value_to_field_value(val));
            }
        }
    }

    Ok(b)
}

// ---------------------------------------------------------------------------
// decision_from_json
// ---------------------------------------------------------------------------

const DECISION_KNOWN: &[&str] = &[
    "node",
    "id",
    "type",
    "summary",
    "rationale",
    "tradeoffs",
    "resolution",
    "detail",
    "stability",
    "notes",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "tags",
    "confidence",
    "status",
];

pub fn decision_from_json(id: &str, v: &Value) -> Result<DecisionBuilder, IngestError> {
    if !v.is_object() {
        return Err(IngestError::NotAnObject);
    }

    let mut b = DecisionBuilder::new(id);

    if let Some(s) = str_val(v, "summary") {
        b = b.summary(s);
    }
    if let Some(items) = string_list(v, "rationale") {
        b = b.rationale(items);
    }
    if let Some(items) = string_list(v, "tradeoffs") {
        b = b.tradeoffs(items);
    }
    if let Some(items) = string_list(v, "resolution") {
        b = b.resolution(items);
    }
    if let Some(s) = str_val(v, "detail") {
        b = b.detail(s);
    }
    if let Some(s) = str_val(v, "notes") {
        b = b.notes(s);
    }

    let uni = extract_universal(v)?;
    if let Some(s) = uni.stability {
        b = b.stability(s);
    }
    if let Some(deps) = uni.depends {
        b = b.depends(deps);
    }
    if let Some(rel) = uni.related_to {
        b = b.related_to(rel);
    }
    if let Some(rep) = uni.replaces {
        b = b.replaces(rep);
    }
    if let Some(con) = uni.conflicts {
        b = b.conflicts(con);
    }
    if let Some(sa) = uni.see_also {
        b = b.see_also(sa);
    }
    if let Some(t) = uni.tags {
        b = b.tags(t);
    }

    // extra_fields
    if let Some(obj) = v.as_object() {
        for (key, val) in obj {
            if !DECISION_KNOWN.contains(&key.as_str()) && !is_identity(key) {
                b = b.extra(key.clone(), value_to_field_value(val));
            }
        }
    }

    Ok(b)
}

// ---------------------------------------------------------------------------
// memory_entry_from_json
// ---------------------------------------------------------------------------

/// Returns a `MemoryEntryBuilder` pre-populated from a JSON object.
///
/// This function handles ingest of a single memory entry as a standalone
/// operation. In practice, memory entries are embedded in nodes; this helper
/// exists for completeness and for future CLI extensions.
pub fn memory_entry_from_json(_id: &str, v: &Value) -> Result<MemoryEntryBuilder, IngestError> {
    if !v.is_object() {
        return Err(IngestError::NotAnObject);
    }

    let key = str_val(v, "key").unwrap_or("").to_owned();
    let topic = str_val(v, "topic").unwrap_or("").to_owned();
    let action_str = str_val(v, "action").unwrap_or("upsert");
    let action = parse_enum!(action_str, "action", MemoryAction)?;

    let mut b = MemoryEntryBuilder::new(key, topic, action);

    if let Some(s) = str_val(v, "value") {
        b = b.value(s);
    }
    if let Some(s) = str_val(v, "scope") {
        b = b.scope(parse_enum!(s, "scope", MemoryScope)?);
    }
    if let Some(s) = str_val(v, "ttl") {
        b = b.ttl(parse_enum!(s, "ttl", MemoryTtl)?);
    }
    if let Some(s) = str_val(v, "query") {
        b = b.query(s);
    }
    if let Some(n) = v.get("max_results").and_then(|v| v.as_u64()) {
        b = b.max_results(n as u32);
    }

    // extra_fields: all unrecognized keys
    const KNOWN: &[&str] = &[
        "key",
        "topic",
        "action",
        "value",
        "scope",
        "ttl",
        "query",
        "max_results",
    ];
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if !KNOWN.contains(&k.as_str()) {
                b = b.extra(k.clone(), value_to_field_value(val));
            }
        }
    }

    Ok(b)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- ticket ---

    #[test]
    fn test_ticket_from_json_minimal() {
        let v = json!({
            "summary": "add login",
            "title": "Add Login",
            "description": "Implement login.",
            "priority": "high"
        });
        let b = ticket_from_json("t.login", &v).unwrap();
        let node = b.build().unwrap();
        assert_eq!(node.id, "t.login");
        assert_eq!(node.summary, "add login");
        assert_eq!(node.priority, Some(Priority::High));
    }

    #[test]
    fn test_ticket_from_json_with_labels() {
        let v = json!({
            "summary": "s",
            "title": "T",
            "description": "d",
            "priority": "normal",
            "labels": ["auth", "security"]
        });
        let node = ticket_from_json("t.x", &v).unwrap().build().unwrap();
        assert_eq!(node.labels.as_deref().unwrap().len(), 2);
    }

    #[test]
    fn test_ticket_from_json_bad_priority_returns_err() {
        let v = json!({"summary": "s", "priority": "urgent"});
        let err = ticket_from_json("t.x", &v).unwrap_err();
        assert!(matches!(err, IngestError::SchemaCheck(_)));
        assert!(err.to_string().contains("priority"));
    }

    #[test]
    fn test_ticket_from_json_unknown_field_goes_to_extra() {
        let v = json!({
            "summary": "s",
            "title": "T",
            "description": "d",
            "priority": "low",
            "custom_field": "custom_value"
        });
        let node = ticket_from_json("t.x", &v).unwrap().build().unwrap();
        assert!(node.extra_fields.contains_key("custom_field"));
    }

    #[test]
    fn test_ticket_from_json_skips_identity_fields() {
        let v = json!({
            "node": "t.x",
            "id": "t.x",
            "type": "ticket",
            "summary": "s",
            "title": "T",
            "description": "d",
            "priority": "normal"
        });
        // should not put node/id/type into extra_fields
        let node = ticket_from_json("t.x", &v).unwrap().build().unwrap();
        assert!(!node.extra_fields.contains_key("node"));
        assert!(!node.extra_fields.contains_key("id"));
        assert!(!node.extra_fields.contains_key("type"));
    }

    #[test]
    fn test_ticket_from_json_non_object_returns_err() {
        let v = json!([1, 2, 3]);
        assert!(matches!(
            ticket_from_json("t.x", &v).unwrap_err(),
            IngestError::NotAnObject
        ));
    }

    // --- workflow ---

    #[test]
    fn test_workflow_from_json_with_code_blocks() {
        let v = json!({
            "summary": "authenticate user",
            "steps": ["resolve tenant", "redirect"],
            "code_blocks": [
                {"action": "create", "body": "fn auth() {}", "lang": "rust", "target": "src/auth.rs"}
            ]
        });
        let node = workflow_from_json("auth.login", &v)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(node.steps.as_deref().unwrap().len(), 2);
        assert_eq!(node.code_blocks.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_workflow_from_json_unknown_field_goes_to_extra() {
        let v = json!({
            "summary": "authenticate",
            "unknown_field": "value"
        });
        let node = workflow_from_json("auth.login", &v)
            .unwrap()
            .build()
            .unwrap();
        assert!(node.extra_fields.contains_key("unknown_field"));
    }

    // --- orchestration ---

    #[test]
    fn test_orchestration_from_json_with_groups() {
        let v = json!({
            "summary": "deploy pipeline",
            "parallel_groups": [
                {
                    "group": "1-schema",
                    "nodes": ["deploy.orchestration"],
                    "strategy": "sequential"
                }
            ]
        });
        let node = orchestration_from_json("deploy.orchestration", &v)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(node.parallel_groups.as_ref().unwrap().len(), 1);
    }

    // --- facts ---

    #[test]
    fn test_facts_from_json_minimal() {
        let v = json!({
            "summary": "auth constraints",
            "items": ["sessions expire after 24h", "MFA required"]
        });
        let node = facts_from_json("auth.constraints", &v)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(node.items.as_deref().unwrap().len(), 2);
    }

    #[test]
    fn test_facts_from_json_unknown_field_goes_to_extra() {
        let v = json!({
            "summary": "auth constraints",
            "extra_meta": "value"
        });
        let node = facts_from_json("auth.constraints", &v)
            .unwrap()
            .build()
            .unwrap();
        assert!(node.extra_fields.contains_key("extra_meta"));
    }

    // --- rules ---

    #[test]
    fn test_rules_from_json_minimal() {
        let v = json!({
            "summary": "auth rules",
            "items": ["require HTTPS"]
        });
        let node = rules_from_json("auth.rules", &v).unwrap().build().unwrap();
        assert_eq!(node.items.as_deref().unwrap().len(), 1);
    }

    #[test]
    fn test_rules_from_json_unknown_field_goes_to_extra() {
        let v = json!({
            "summary": "auth rules",
            "items": ["require HTTPS"],
            "custom_info": "value"
        });
        let node = rules_from_json("auth.rules", &v).unwrap().build().unwrap();
        assert!(node.extra_fields.contains_key("custom_info"));
    }

    // --- decision ---

    #[test]
    fn test_decision_from_json_minimal() {
        let v = json!({
            "summary": "chose PostgreSQL",
            "rationale": ["ACID guarantees required"]
        });
        let node = decision_from_json("arch.db-choice", &v)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(node.rationale.as_deref().unwrap().len(), 1);
    }

    #[test]
    fn test_decision_from_json_with_tradeoffs_and_resolution() {
        let v = json!({
            "summary": "chose PostgreSQL",
            "rationale": ["ACID required"],
            "tradeoffs": ["complex scaling"],
            "resolution": ["use PostgreSQL 16"]
        });
        let node = decision_from_json("arch.db-choice", &v)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(node.tradeoffs.as_deref().unwrap().len(), 1);
        assert_eq!(node.resolution.as_deref().unwrap().len(), 1);
    }

    #[test]
    fn test_decision_from_json_bad_stability_returns_err() {
        let v = json!({
            "summary": "s",
            "rationale": ["r"],
            "stability": "unknown_stability"
        });
        let err = decision_from_json("arch.db-choice", &v).unwrap_err();
        assert!(matches!(err, IngestError::SchemaCheck(_)));
    }

    #[test]
    fn test_decision_from_json_unknown_field_goes_to_extra() {
        let v = json!({
            "summary": "s",
            "rationale": ["r"],
            "custom_data": "value"
        });
        let node = decision_from_json("arch.db-choice", &v)
            .unwrap()
            .build()
            .unwrap();
        assert!(node.extra_fields.contains_key("custom_data"));
    }

    // --- memory_entry ---

    #[test]
    fn test_memory_entry_from_json_minimal() {
        let v = json!({
            "key": "repo.pattern",
            "topic": "rust.repository",
            "action": "upsert",
            "value": "row_to_column uses get()"
        });
        let entry = memory_entry_from_json("ignored", &v)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(entry.key, "repo.pattern");
        assert_eq!(entry.topic, "rust.repository");
        assert_eq!(entry.action, MemoryAction::Upsert);
        assert_eq!(entry.value.as_deref(), Some("row_to_column uses get()"));
    }

    #[test]
    fn test_memory_entry_from_json_bad_action_returns_err() {
        let v = json!({"key": "k", "topic": "t", "action": "unknown_action"});
        let err = memory_entry_from_json("ignored", &v).unwrap_err();
        assert!(matches!(err, IngestError::SchemaCheck(_)));
    }

    #[test]
    fn test_memory_entry_from_json_unknown_field_goes_to_extra() {
        let v = json!({
            "key": "k",
            "topic": "t",
            "action": "get",
            "custom_field": "custom_value"
        });
        let entry = memory_entry_from_json("ignored", &v)
            .unwrap()
            .build()
            .unwrap();
        assert!(entry.extra_fields.contains_key("custom_field"));
    }
}
