//! Node parser: reads a single AGM node declaration and its fields.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::execution::ExecutionStatus;
use crate::model::fields::{
    Confidence, FieldValue, NodeStatus, NodeType, Priority, Span, Stability,
};
use crate::model::node::Node;

use super::fields::{
    FieldTracker, collect_structured_raw, is_structured_field, parse_block, parse_indented_list,
    skip_field_body,
};
use super::lexer::{Line, LineKind};
use super::structured::{
    parse_agent_context, parse_code_block, parse_code_blocks, parse_memory, parse_parallel_groups,
    parse_verify,
};

// ---------------------------------------------------------------------------
// Node ID validation
// ---------------------------------------------------------------------------

static NODE_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_]*([.\-][a-z][a-z0-9_]*)*$").unwrap());

// ---------------------------------------------------------------------------
// default_node
// ---------------------------------------------------------------------------

/// Creates a `Node` with all fields at their defaults.
///
/// `node_type` is set to `NodeType::Facts` as a placeholder;
/// `summary` is set to an empty string. The validator will catch missing fields.
fn default_node(id: String, start_line: usize) -> Node {
    Node {
        id,
        node_type: NodeType::Facts,
        summary: String::new(),
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
        span: Span::new(start_line, start_line),
    }
}

// ---------------------------------------------------------------------------
// parse_node
// ---------------------------------------------------------------------------

/// Parses a single node starting at the `NodeDeclaration` line at `*pos`.
///
/// Advances `pos` past all lines belonging to this node.
pub fn parse_node(lines: &[Line], pos: &mut usize, errors: &mut Vec<AgmError>) -> Node {
    // Extract ID from NodeDeclaration.
    let (id, declaration_line) = match &lines[*pos].kind {
        LineKind::NodeDeclaration(id) => (id.clone(), lines[*pos].number),
        _ => {
            // Should never happen — caller guarantees this.
            return default_node(String::new(), lines[*pos].number);
        }
    };

    // Validate node ID.
    if id.is_empty() {
        errors.push(AgmError::new(
            ErrorCode::P002,
            "Node declaration missing ID",
            ErrorLocation::new(None, Some(declaration_line), None),
        ));
    } else if !NODE_ID_RE.is_match(&id) {
        errors.push(AgmError::new(
            ErrorCode::P002,
            format!("Invalid node ID: {id:?} (must match [a-z][a-z0-9]*([.\\-][a-z][a-z0-9]*)*)"),
            ErrorLocation::new(None, Some(declaration_line), None),
        ));
    }

    let mut node = default_node(id, declaration_line);
    *pos += 1; // advance past NodeDeclaration

    let mut tracker = FieldTracker::new();
    let mut last_line = declaration_line;

    while *pos < lines.len() {
        match &lines[*pos].kind.clone() {
            LineKind::NodeDeclaration(_) => break,

            LineKind::Blank | LineKind::Comment | LineKind::TestExpectHeader(_) => {
                last_line = lines[*pos].number;
                *pos += 1;
            }

            LineKind::ScalarField(key, value) => {
                let key = key.clone();
                let value = value.clone();
                let line_number = lines[*pos].number;
                last_line = line_number;

                if tracker.track(&key) {
                    errors.push(AgmError::new(
                        ErrorCode::P006,
                        format!("Duplicate field `{key}` in node `{}`", node.id),
                        ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                    ));
                    *pos += 1;
                    continue;
                }

                assign_scalar_field(&mut node, &key, &value, line_number, errors);
                *pos += 1;
            }

            LineKind::InlineListField(key, items) => {
                let key = key.clone();
                let items = items.clone();
                let line_number = lines[*pos].number;
                last_line = line_number;

                if tracker.track(&key) {
                    errors.push(AgmError::new(
                        ErrorCode::P006,
                        format!("Duplicate field `{key}` in node `{}`", node.id),
                        ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                    ));
                    *pos += 1;
                    continue;
                }

                assign_list_field(&mut node, &key, items);
                *pos += 1;
            }

            LineKind::FieldStart(key) => {
                let key = key.clone();
                let line_number = lines[*pos].number;
                last_line = line_number;

                if tracker.track(&key) {
                    errors.push(AgmError::new(
                        ErrorCode::P006,
                        format!("Duplicate field `{key}` in node `{}`", node.id),
                        ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                    ));
                    *pos += 1;
                    skip_field_body(lines, pos);
                    continue;
                }

                *pos += 1; // advance past FieldStart

                if is_structured_field(&key) {
                    match key.as_str() {
                        "code" => {
                            node.code = Some(parse_code_block(lines, pos, errors));
                        }
                        "code_blocks" => {
                            node.code_blocks = Some(parse_code_blocks(lines, pos, errors));
                        }
                        "verify" => {
                            node.verify = Some(parse_verify(lines, pos, errors));
                        }
                        "agent_context" => {
                            node.agent_context = Some(parse_agent_context(lines, pos, errors));
                        }
                        "parallel_groups" => {
                            node.parallel_groups = Some(parse_parallel_groups(lines, pos, errors));
                        }
                        "memory" => {
                            node.memory = Some(parse_memory(lines, pos, errors));
                        }
                        _ => {
                            // Unknown structured field — collect raw for forward compatibility.
                            let raw = collect_structured_raw(lines, pos);
                            node.extra_fields.insert(key, FieldValue::Block(raw));
                        }
                    }
                } else {
                    // Peek: list or block? Skip over comment lines before deciding.
                    let peek_pos = {
                        let mut p = *pos;
                        while p < lines.len()
                            && matches!(
                                &lines[p].kind,
                                LineKind::Comment | LineKind::TestExpectHeader(_)
                            )
                        {
                            p += 1;
                        }
                        p
                    };
                    let is_list = peek_pos < lines.len()
                        && matches!(&lines[peek_pos].kind, LineKind::ListItem(_));

                    if is_list {
                        let items = parse_indented_list(lines, pos);
                        assign_list_field(&mut node, &key, items);
                    } else {
                        let text = parse_block(lines, pos);
                        assign_block_field(&mut node, &key, text);
                    }
                }
            }

            LineKind::BodyMarker => {
                let line_number = lines[*pos].number;
                last_line = line_number;

                let is_dup = tracker.track("body");
                *pos += 1; // advance past BodyMarker

                let text = parse_block(lines, pos);

                if is_dup {
                    // Already have body — stash in extra_fields.
                    node.extra_fields
                        .insert("body".to_owned(), FieldValue::Block(text));
                } else {
                    // Assign to detail (canonical body destination).
                    if node.detail.is_none() {
                        node.detail = Some(text);
                    } else {
                        node.extra_fields
                            .insert("body".to_owned(), FieldValue::Block(text));
                    }
                }
            }

            LineKind::ListItem(_) | LineKind::IndentedLine(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!(
                        "Unexpected indentation at line {} in node `{}`",
                        lines[*pos].number, node.id
                    ),
                    ErrorLocation::new(None, Some(lines[*pos].number), Some(node.id.clone())),
                ));
                last_line = lines[*pos].number;
                *pos += 1;
            }
        }
    }

    node.span.end_line = last_line;
    node
}

// ---------------------------------------------------------------------------
// assign_scalar_field
// ---------------------------------------------------------------------------

fn assign_scalar_field(
    node: &mut Node,
    key: &str,
    value: &str,
    line_number: usize,
    errors: &mut Vec<AgmError>,
) {
    match key {
        "type" => {
            node.node_type = value.parse().unwrap(); // infallible — returns Custom
        }
        "summary" => {
            node.summary = value.to_owned();
        }
        "priority" => match value.parse::<Priority>() {
            Ok(p) => node.priority = Some(p),
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Invalid `priority` value: {value:?}"),
                    ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                ));
            }
        },
        "stability" => match value.parse::<Stability>() {
            Ok(s) => node.stability = Some(s),
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Invalid `stability` value: {value:?}"),
                    ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                ));
            }
        },
        "confidence" => match value.parse::<Confidence>() {
            Ok(c) => node.confidence = Some(c),
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Invalid `confidence` value: {value:?}"),
                    ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                ));
            }
        },
        "status" => match value.parse::<NodeStatus>() {
            Ok(s) => node.status = Some(s),
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Invalid `status` value: {value:?}"),
                    ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                ));
            }
        },
        "detail" => node.detail = Some(value.to_owned()),
        "examples" => node.examples = Some(value.to_owned()),
        "notes" => node.notes = Some(value.to_owned()),
        "target" => node.target = Some(value.to_owned()),
        "applies_when" => node.applies_when = Some(value.to_owned()),
        "valid_from" => node.valid_from = Some(value.to_owned()),
        "valid_until" => node.valid_until = Some(value.to_owned()),
        "execution_status" => match value.parse::<ExecutionStatus>() {
            Ok(es) => node.execution_status = Some(es),
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Invalid `execution_status` value: {value:?}"),
                    ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                ));
            }
        },
        "executed_by" => node.executed_by = Some(value.to_owned()),
        "executed_at" => node.executed_at = Some(value.to_owned()),
        "execution_log" => node.execution_log = Some(value.to_owned()),
        "retry_count" => match value.parse::<u32>() {
            Ok(n) => node.retry_count = Some(n),
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Invalid `retry_count` value: {value:?}"),
                    ErrorLocation::new(None, Some(line_number), Some(node.id.clone())),
                ));
            }
        },
        _ => {
            node.extra_fields
                .insert(key.to_owned(), FieldValue::Scalar(value.to_owned()));
        }
    }
}

// ---------------------------------------------------------------------------
// assign_list_field
// ---------------------------------------------------------------------------

fn assign_list_field(node: &mut Node, key: &str, items: Vec<String>) {
    match key {
        "depends" => node.depends = Some(items),
        "related_to" => node.related_to = Some(items),
        "replaces" => node.replaces = Some(items),
        "conflicts" => node.conflicts = Some(items),
        "see_also" => node.see_also = Some(items),
        "items" => node.items = Some(items),
        "steps" => node.steps = Some(items),
        "fields" => node.fields = Some(items),
        "input" => node.input = Some(items),
        "output" => node.output = Some(items),
        "rationale" => node.rationale = Some(items),
        "tradeoffs" => node.tradeoffs = Some(items),
        "resolution" => node.resolution = Some(items),
        "scope" => node.scope = Some(items),
        "tags" => node.tags = Some(items),
        "aliases" => node.aliases = Some(items),
        "keywords" => node.keywords = Some(items),
        _ => {
            node.extra_fields
                .insert(key.to_owned(), FieldValue::List(items));
        }
    }
}

// ---------------------------------------------------------------------------
// assign_block_field
// ---------------------------------------------------------------------------

fn assign_block_field(node: &mut Node, key: &str, text: String) {
    match key {
        "detail" => node.detail = Some(text),
        "examples" => node.examples = Some(text),
        "notes" => node.notes = Some(text),
        "execution_log" => node.execution_log = Some(text),
        "applies_when" => node.applies_when = Some(text),
        _ => {
            node.extra_fields
                .insert(key.to_owned(), FieldValue::Block(text));
        }
    }
}
