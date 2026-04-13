//! Structured field parsers: converts typed sub-fields into model types.
//!
//! All parsers follow the same convention:
//!   - Accept `&[Line]`, `&mut usize` (pos), `&mut Vec<AgmError>`
//!   - On entry, `*pos` points to the first line AFTER the `FieldStart` line
//!   - Field content ends when a line's indent < base_indent (non-blank),
//!     or a `NodeDeclaration` is encountered, or EOF

use std::collections::BTreeMap;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::code::{CodeAction, CodeBlock};
use crate::model::context::{AgentContext, FileRange, LoadFile};
use crate::model::file::{LoadProfile, TokenEstimate};
use crate::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};
use crate::model::orchestration::{ParallelGroup, Strategy};
use crate::model::verify::VerifyCheck;

use super::lexer::{Line, LineKind};

// ---------------------------------------------------------------------------
// Internal SubFieldValue enum
// ---------------------------------------------------------------------------

/// Internal representation of a sub-field value before it is converted to
/// a typed model value.
enum SubFieldValue {
    Scalar(String),
    List(Vec<String>),
    PipeBody(String),
}

// ---------------------------------------------------------------------------
// Indentation helpers
// ---------------------------------------------------------------------------

/// Scans forward from `pos` (skipping blanks) and returns the indent of the
/// first non-blank line that is not a `NodeDeclaration`.
///
/// Returns `None` if no such line exists.
fn detect_base_indent(lines: &[Line], pos: usize) -> Option<usize> {
    let mut i = pos;
    while i < lines.len() {
        match &lines[i].kind {
            LineKind::Blank => {
                i += 1;
            }
            LineKind::NodeDeclaration(_) => return None,
            _ => return Some(lines[i].indent),
        }
    }
    None
}

/// Returns `true` if the line at `pos` is still within the structured field
/// body (i.e., its indent is >= `base_indent`, or it is blank).
fn is_within_field(lines: &[Line], pos: usize, base_indent: usize) -> bool {
    if pos >= lines.len() {
        return false;
    }
    match &lines[pos].kind {
        LineKind::Blank => true,
        LineKind::NodeDeclaration(_) => false,
        _ => lines[pos].indent >= base_indent,
    }
}

/// Advances `*pos` past any blank lines.
fn skip_blanks(lines: &[Line], pos: &mut usize) {
    while *pos < lines.len() && matches!(&lines[*pos].kind, LineKind::Blank) {
        *pos += 1;
    }
}

/// Parses a key: value pair from a raw string (e.g. `"  action: create"`).
/// Returns `(key, value)` or `None` if the string is not a valid kv pair.
fn parse_kv_from_text(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let colon_pos = trimmed.find(':')?;
    let key = trimmed[..colon_pos].trim();
    let value = trimmed[colon_pos + 1..].trim();
    if key.is_empty() {
        return None;
    }
    Some((key.to_owned(), value.to_owned()))
}

// ---------------------------------------------------------------------------
// collect_pipe_body
// ---------------------------------------------------------------------------

/// Collects body text after a `BodyMarker` line (`body: |`).
///
/// Strips the base indent from each line, preserving relative indentation.
/// Trailing blank lines are removed.
fn collect_pipe_body(lines: &[Line], pos: &mut usize) -> String {
    // Detect body indent from the first non-blank line.
    let body_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => return String::new(),
    };

    let mut parts: Vec<String> = Vec::new();

    while *pos < lines.len() {
        match &lines[*pos].kind {
            LineKind::NodeDeclaration(_) => break,
            LineKind::Blank => {
                // Peek ahead: keep blank if more body follows.
                let mut lookahead = *pos + 1;
                while lookahead < lines.len() && matches!(&lines[lookahead].kind, LineKind::Blank) {
                    lookahead += 1;
                }
                let more_body = lookahead < lines.len()
                    && !matches!(&lines[lookahead].kind, LineKind::NodeDeclaration(_))
                    && lines[lookahead].indent >= body_indent;

                if more_body {
                    parts.push(String::new());
                    *pos += 1;
                } else {
                    break;
                }
            }
            _ => {
                if lines[*pos].indent < body_indent {
                    break;
                }
                let raw = &lines[*pos].raw;
                let stripped = if raw.len() >= body_indent {
                    raw[body_indent..].to_owned()
                } else {
                    raw.trim_start().to_owned()
                };
                parts.push(stripped);
                *pos += 1;
            }
        }
    }

    // Trim trailing blank entries.
    while parts.last().is_some_and(|s: &String| s.is_empty()) {
        parts.pop();
    }
    parts.join("\n")
}

// ---------------------------------------------------------------------------
// collect_sub_fields
// ---------------------------------------------------------------------------

/// Reads the sub-fields of a single structured object entry.
///
/// On entry, `*pos` should point to the first line of the sub-field block
/// (i.e., the line immediately after a list-item intro or after a
/// `FieldStart` for a single-object field). `base_indent` is the expected
/// minimum indent for lines inside this object.
///
/// Returns the collected sub-fields as `Vec<(key, SubFieldValue)>`.
fn collect_sub_fields(
    lines: &[Line],
    pos: &mut usize,
    base_indent: usize,
) -> Vec<(String, SubFieldValue)> {
    let mut fields: Vec<(String, SubFieldValue)> = Vec::new();

    while *pos < lines.len() {
        match &lines[*pos].kind {
            LineKind::Blank => {
                // Peek: stay in block only if indented content follows.
                let mut lookahead = *pos + 1;
                while lookahead < lines.len() && matches!(&lines[lookahead].kind, LineKind::Blank) {
                    lookahead += 1;
                }
                let continues = lookahead < lines.len()
                    && !matches!(&lines[lookahead].kind, LineKind::NodeDeclaration(_))
                    && lines[lookahead].indent >= base_indent;

                if continues {
                    *pos += 1;
                } else {
                    break;
                }
            }
            LineKind::NodeDeclaration(_) => break,
            _ if lines[*pos].indent < base_indent => break,
            LineKind::ScalarField(key, value) => {
                fields.push((key.clone(), SubFieldValue::Scalar(value.clone())));
                *pos += 1;
            }
            LineKind::InlineListField(key, items) => {
                fields.push((key.clone(), SubFieldValue::List(items.clone())));
                *pos += 1;
            }
            LineKind::BodyMarker => {
                // body: | — collect pipe body.
                *pos += 1; // advance past BodyMarker
                let body = collect_pipe_body(lines, pos);
                fields.push(("body".to_owned(), SubFieldValue::PipeBody(body)));
            }
            LineKind::FieldStart(key) => {
                let key = key.clone();
                let field_line_indent = lines[*pos].indent;
                *pos += 1; // advance past FieldStart

                // Determine what follows: list of objects, list of scalars, or block.
                skip_blanks(lines, pos);

                if *pos >= lines.len() {
                    // Empty field.
                    fields.push((key, SubFieldValue::Scalar(String::new())));
                    continue;
                }

                let next_indent = lines[*pos].indent;

                match &lines[*pos].kind {
                    LineKind::ListItem(_) => {
                        // Collect list items.
                        let mut items = Vec::new();
                        while *pos < lines.len() {
                            match &lines[*pos].kind {
                                LineKind::ListItem(v) => {
                                    items.push(v.clone());
                                    *pos += 1;
                                }
                                LineKind::Blank => {
                                    let mut la = *pos + 1;
                                    while la < lines.len()
                                        && matches!(&lines[la].kind, LineKind::Blank)
                                    {
                                        la += 1;
                                    }
                                    if la < lines.len()
                                        && matches!(&lines[la].kind, LineKind::ListItem(_))
                                        && lines[la].indent > field_line_indent
                                    {
                                        *pos += 1;
                                    } else {
                                        break;
                                    }
                                }
                                _ => break,
                            }
                        }
                        fields.push((key, SubFieldValue::List(items)));
                    }
                    _ if next_indent > field_line_indent => {
                        // Nested scalar block or nested objects.
                        // Read lines as a block string.
                        let body_indent = next_indent;
                        let mut body_parts: Vec<String> = Vec::new();
                        while *pos < lines.len() {
                            match &lines[*pos].kind {
                                LineKind::Blank => {
                                    let mut la = *pos + 1;
                                    while la < lines.len()
                                        && matches!(&lines[la].kind, LineKind::Blank)
                                    {
                                        la += 1;
                                    }
                                    let more = la < lines.len()
                                        && !matches!(&lines[la].kind, LineKind::NodeDeclaration(_))
                                        && lines[la].indent >= body_indent;
                                    if more {
                                        body_parts.push(String::new());
                                        *pos += 1;
                                    } else {
                                        break;
                                    }
                                }
                                LineKind::NodeDeclaration(_) => break,
                                _ => {
                                    if lines[*pos].indent < body_indent {
                                        break;
                                    }
                                    let raw = &lines[*pos].raw;
                                    let stripped = if raw.len() >= body_indent {
                                        raw[body_indent..].to_owned()
                                    } else {
                                        raw.trim_start().to_owned()
                                    };
                                    body_parts.push(stripped);
                                    *pos += 1;
                                }
                            }
                        }
                        while body_parts.last().is_some_and(|s: &String| s.is_empty()) {
                            body_parts.pop();
                        }
                        fields.push((key, SubFieldValue::Scalar(body_parts.join("\n"))));
                    }
                    _ => {
                        // Nothing follows at the right indent.
                        fields.push((key, SubFieldValue::Scalar(String::new())));
                    }
                }
            }
            LineKind::ListItem(_) | LineKind::IndentedLine(_) => {
                // Unexpected at this level — stop.
                break;
            }
            _ => {
                // Anything else stops the sub-field collection.
                break;
            }
        }
    }

    fields
}

// ---------------------------------------------------------------------------
// Helper: extract scalar from sub-field map
// ---------------------------------------------------------------------------

fn get_scalar<'a>(fields: &'a [(String, SubFieldValue)], key: &str) -> Option<&'a str> {
    for (k, v) in fields {
        if k == key {
            if let SubFieldValue::Scalar(s) = v {
                return Some(s.as_str());
            }
            if let SubFieldValue::PipeBody(s) = v {
                return Some(s.as_str());
            }
        }
    }
    None
}

fn get_list<'a>(fields: &'a [(String, SubFieldValue)], key: &str) -> Option<&'a [String]> {
    for (k, v) in fields {
        if k == key {
            if let SubFieldValue::List(items) = v {
                return Some(items.as_slice());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// build_code_block_from_fields
// ---------------------------------------------------------------------------

fn build_code_block_from_fields(
    fields: &[(String, SubFieldValue)],
    errors: &mut Vec<AgmError>,
    line_number: usize,
) -> CodeBlock {
    let action_str = get_scalar(fields, "action").unwrap_or("");
    let action = if action_str.is_empty() {
        errors.push(AgmError::new(
            ErrorCode::V008,
            "Code block missing required field: `action`",
            ErrorLocation::new(None, Some(line_number), None),
        ));
        CodeAction::Full // fallback
    } else {
        match action_str.parse::<CodeAction>() {
            Ok(a) => a,
            Err(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Invalid `action` value in code block: {action_str:?}"),
                    ErrorLocation::new(None, Some(line_number), None),
                ));
                CodeAction::Full
            }
        }
    };

    let body = get_scalar(fields, "body").unwrap_or("").to_owned();
    if body.is_empty() {
        errors.push(AgmError::new(
            ErrorCode::V008,
            "Code block missing required field: `body`",
            ErrorLocation::new(None, Some(line_number), None),
        ));
    }

    let lang = get_scalar(fields, "lang")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned());
    let target = get_scalar(fields, "target")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned());
    let anchor = get_scalar(fields, "anchor")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned());
    let old = get_scalar(fields, "old")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned());

    CodeBlock {
        lang,
        target,
        action,
        body,
        anchor,
        old,
    }
}

// ---------------------------------------------------------------------------
// Phase 1: parse_code_block
// ---------------------------------------------------------------------------

/// Parses a single `code:` block into a `CodeBlock`.
///
/// On entry, `*pos` points to the first line after the `FieldStart("code")`.
pub(crate) fn parse_code_block(
    lines: &[Line],
    pos: &mut usize,
    errors: &mut Vec<AgmError>,
) -> CodeBlock {
    let line_number = if *pos > 0 {
        lines.get(*pos - 1).map_or(0, |l| l.number)
    } else {
        0
    };

    let base_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => {
            errors.push(AgmError::new(
                ErrorCode::V008,
                "Code block missing required field: `action`",
                ErrorLocation::new(None, Some(line_number), None),
            ));
            errors.push(AgmError::new(
                ErrorCode::V008,
                "Code block missing required field: `body`",
                ErrorLocation::new(None, Some(line_number), None),
            ));
            return CodeBlock {
                lang: None,
                target: None,
                action: CodeAction::Full,
                body: String::new(),
                anchor: None,
                old: None,
            };
        }
    };

    let fields = collect_sub_fields(lines, pos, base_indent);
    build_code_block_from_fields(&fields, errors, line_number)
}

// ---------------------------------------------------------------------------
// Phase 2: parse_code_blocks
// ---------------------------------------------------------------------------

/// Parses a `code_blocks:` list into a `Vec<CodeBlock>`.
///
/// On entry, `*pos` points to the first line after the `FieldStart("code_blocks")`.
pub(crate) fn parse_code_blocks(
    lines: &[Line],
    pos: &mut usize,
    errors: &mut Vec<AgmError>,
) -> Vec<CodeBlock> {
    let base_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => return Vec::new(),
    };

    let mut blocks = Vec::new();

    while is_within_field(lines, *pos, base_indent) {
        skip_blanks(lines, pos);
        if !is_within_field(lines, *pos, base_indent) {
            break;
        }

        match &lines[*pos].kind {
            LineKind::ListItem(text) => {
                let line_number = lines[*pos].number;
                let text = text.clone();
                *pos += 1;

                // The list item text may contain inline kv pairs like "action: create"
                // but in practice list-item objects have sub-fields on subsequent lines.
                let mut fields: Vec<(String, SubFieldValue)> = Vec::new();

                // Parse inline kv if any on the dash line.
                if !text.is_empty() {
                    if let Some((k, v)) = parse_kv_from_text(&text) {
                        fields.push((k, SubFieldValue::Scalar(v)));
                    }
                }

                // Determine indent of sub-fields (they should be indented more than the dash).
                let sub_indent = detect_base_indent(lines, *pos);
                if let Some(si) = sub_indent {
                    if si > base_indent {
                        let mut sub = collect_sub_fields(lines, pos, si);
                        fields.append(&mut sub);
                    }
                }

                blocks.push(build_code_block_from_fields(&fields, errors, line_number));
            }
            _ => break,
        }
    }

    blocks
}

// ---------------------------------------------------------------------------
// Phase 2: parse_verify
// ---------------------------------------------------------------------------

/// Parses a `verify:` list into a `Vec<VerifyCheck>`.
///
/// On entry, `*pos` points to the first line after the `FieldStart("verify")`.
pub(crate) fn parse_verify(
    lines: &[Line],
    pos: &mut usize,
    errors: &mut Vec<AgmError>,
) -> Vec<VerifyCheck> {
    let base_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => return Vec::new(),
    };

    let mut checks = Vec::new();

    while is_within_field(lines, *pos, base_indent) {
        skip_blanks(lines, pos);
        if !is_within_field(lines, *pos, base_indent) {
            break;
        }

        match &lines[*pos].kind {
            LineKind::ListItem(text) => {
                let line_number = lines[*pos].number;
                let text = text.clone();
                *pos += 1;

                let mut fields: Vec<(String, SubFieldValue)> = Vec::new();

                // Parse inline kv on the dash line.
                if !text.is_empty() {
                    if let Some((k, v)) = parse_kv_from_text(&text) {
                        fields.push((k, SubFieldValue::Scalar(v)));
                    }
                }

                // Sub-fields on subsequent lines.
                let sub_indent = detect_base_indent(lines, *pos);
                if let Some(si) = sub_indent {
                    if si > base_indent {
                        let mut sub = collect_sub_fields(lines, pos, si);
                        fields.append(&mut sub);
                    }
                }

                let check = build_verify_check_from_fields(&fields, errors, line_number);
                if let Some(c) = check {
                    checks.push(c);
                }
            }
            _ => break,
        }
    }

    checks
}

fn build_verify_check_from_fields(
    fields: &[(String, SubFieldValue)],
    errors: &mut Vec<AgmError>,
    line_number: usize,
) -> Option<VerifyCheck> {
    let type_val = match get_scalar(fields, "type") {
        Some(t) if !t.is_empty() => t,
        _ => {
            errors.push(AgmError::new(
                ErrorCode::V009,
                "Verify entry missing required field: `type`",
                ErrorLocation::new(None, Some(line_number), None),
            ));
            return None;
        }
    };

    match type_val {
        "command" => {
            let run = match get_scalar(fields, "run") {
                Some(r) if !r.is_empty() => r.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `run`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            let expect = get_scalar(fields, "expect")
                .filter(|s| !s.is_empty())
                .map(|s| s.to_owned());
            Some(VerifyCheck::Command { run, expect })
        }
        "file_exists" => {
            let file = match get_scalar(fields, "file") {
                Some(f) if !f.is_empty() => f.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `file`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            Some(VerifyCheck::FileExists { file })
        }
        "file_contains" => {
            let file = match get_scalar(fields, "file") {
                Some(f) if !f.is_empty() => f.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `file`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            let pattern = match get_scalar(fields, "pattern") {
                Some(p) if !p.is_empty() => p.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `pattern`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            Some(VerifyCheck::FileContains { file, pattern })
        }
        "file_not_contains" => {
            let file = match get_scalar(fields, "file") {
                Some(f) if !f.is_empty() => f.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `file`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            let pattern = match get_scalar(fields, "pattern") {
                Some(p) if !p.is_empty() => p.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `pattern`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            Some(VerifyCheck::FileNotContains { file, pattern })
        }
        "node_status" => {
            let node = match get_scalar(fields, "node") {
                Some(n) if !n.is_empty() => n.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `node`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            let status = match get_scalar(fields, "status") {
                Some(s) if !s.is_empty() => s.to_owned(),
                _ => {
                    errors.push(AgmError::new(
                        ErrorCode::V009,
                        "Verify entry missing required field: `status`",
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    return None;
                }
            };
            Some(VerifyCheck::NodeStatus { node, status })
        }
        unknown => {
            errors.push(AgmError::new(
                ErrorCode::P003,
                format!("Unknown verify type: {unknown:?}"),
                ErrorLocation::new(None, Some(line_number), None),
            ));
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 3: parse_file_range helper
// ---------------------------------------------------------------------------

/// Parses a file range string such as `"full"`, `"1-50"`, or `"function:name"`.
fn parse_file_range(s: &str) -> FileRange {
    if s == "full" {
        return FileRange::Full;
    }
    if let Some(name) = s.strip_prefix("function:") {
        return FileRange::Function(name.trim().to_owned());
    }
    // Try "start-end" numeric range.
    if let Some(dash_pos) = s.find('-') {
        let start_str = &s[..dash_pos];
        let end_str = &s[dash_pos + 1..];
        if let (Ok(start), Ok(end)) = (
            start_str.trim().parse::<u64>(),
            end_str.trim().parse::<u64>(),
        ) {
            return FileRange::Lines(start, end);
        }
    }
    // Fallback.
    FileRange::Full
}

// ---------------------------------------------------------------------------
// Phase 3: parse_load_files_list helper
// ---------------------------------------------------------------------------

/// Parses a list of load_files entries from collected sub-fields.
fn parse_load_files_list(
    lines: &[Line],
    pos: &mut usize,
    base_indent: usize,
    errors: &mut Vec<AgmError>,
) -> Vec<LoadFile> {
    let mut files = Vec::new();

    while is_within_field(lines, *pos, base_indent) {
        skip_blanks(lines, pos);
        if !is_within_field(lines, *pos, base_indent) {
            break;
        }

        match &lines[*pos].kind {
            LineKind::ListItem(text) => {
                let line_number = lines[*pos].number;
                let text = text.clone();
                *pos += 1;

                let mut fields: Vec<(String, SubFieldValue)> = Vec::new();

                if !text.is_empty() {
                    if let Some((k, v)) = parse_kv_from_text(&text) {
                        fields.push((k, SubFieldValue::Scalar(v)));
                    }
                }

                let sub_indent = detect_base_indent(lines, *pos);
                if let Some(si) = sub_indent {
                    if si > base_indent {
                        let mut sub = collect_sub_fields(lines, pos, si);
                        fields.append(&mut sub);
                    }
                }

                let path = match get_scalar(&fields, "path") {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        errors.push(AgmError::new(
                            ErrorCode::P003,
                            "load_files entry missing required field: `path`",
                            ErrorLocation::new(None, Some(line_number), None),
                        ));
                        continue;
                    }
                };

                let range = get_scalar(&fields, "range")
                    .map(parse_file_range)
                    .unwrap_or(FileRange::Full);

                files.push(LoadFile { path, range });
            }
            _ => break,
        }
    }

    files
}

// ---------------------------------------------------------------------------
// Phase 3: parse_agent_context
// ---------------------------------------------------------------------------

/// Parses an `agent_context:` block into an `AgentContext`.
///
/// On entry, `*pos` points to the first line after the `FieldStart("agent_context")`.
pub(crate) fn parse_agent_context(
    lines: &[Line],
    pos: &mut usize,
    errors: &mut Vec<AgmError>,
) -> AgentContext {
    let base_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => {
            return AgentContext {
                load_nodes: None,
                load_files: None,
                system_hint: None,
                max_tokens: None,
                load_memory: None,
            };
        }
    };

    let mut load_nodes: Option<Vec<String>> = None;
    let mut load_files: Option<Vec<LoadFile>> = None;
    let mut system_hint: Option<String> = None;
    let mut max_tokens: Option<u64> = None;
    let mut load_memory: Option<Vec<String>> = None;

    while is_within_field(lines, *pos, base_indent) {
        skip_blanks(lines, pos);
        if !is_within_field(lines, *pos, base_indent) {
            break;
        }

        match &lines[*pos].kind.clone() {
            LineKind::ScalarField(key, value) if lines[*pos].indent == base_indent => {
                match key.as_str() {
                    "system_hint" => system_hint = Some(value.clone()),
                    "max_tokens" => {
                        if let Ok(n) = value.parse::<u64>() {
                            max_tokens = Some(n);
                        } else {
                            errors.push(AgmError::new(
                                ErrorCode::P003,
                                format!("Invalid `max_tokens` value: {value:?}"),
                                ErrorLocation::new(None, Some(lines[*pos].number), None),
                            ));
                        }
                    }
                    _ => {}
                }
                *pos += 1;
            }
            LineKind::InlineListField(key, items) if lines[*pos].indent == base_indent => {
                match key.as_str() {
                    "load_nodes" => load_nodes = Some(items.clone()),
                    "load_memory" => load_memory = Some(items.clone()),
                    _ => {}
                }
                *pos += 1;
            }
            LineKind::FieldStart(key) if lines[*pos].indent == base_indent => {
                let key = key.clone();
                *pos += 1;
                match key.as_str() {
                    "load_nodes" => {
                        // Indented list of node IDs.
                        let sub_indent = detect_base_indent(lines, *pos);
                        if let Some(si) = sub_indent {
                            if si > base_indent {
                                let mut items = Vec::new();
                                while is_within_field(lines, *pos, si) {
                                    skip_blanks(lines, pos);
                                    if !is_within_field(lines, *pos, si) {
                                        break;
                                    }
                                    if let LineKind::ListItem(v) = &lines[*pos].kind {
                                        items.push(v.clone());
                                        *pos += 1;
                                    } else {
                                        break;
                                    }
                                }
                                load_nodes = Some(items);
                            }
                        }
                    }
                    "load_memory" => {
                        let sub_indent = detect_base_indent(lines, *pos);
                        if let Some(si) = sub_indent {
                            if si > base_indent {
                                let mut items = Vec::new();
                                while is_within_field(lines, *pos, si) {
                                    skip_blanks(lines, pos);
                                    if !is_within_field(lines, *pos, si) {
                                        break;
                                    }
                                    if let LineKind::ListItem(v) = &lines[*pos].kind {
                                        items.push(v.clone());
                                        *pos += 1;
                                    } else {
                                        break;
                                    }
                                }
                                load_memory = Some(items);
                            }
                        }
                    }
                    "load_files" => {
                        let sub_indent = detect_base_indent(lines, *pos);
                        if let Some(si) = sub_indent {
                            if si > base_indent {
                                let fl = parse_load_files_list(lines, pos, si, errors);
                                if !fl.is_empty() {
                                    load_files = Some(fl);
                                }
                            }
                        }
                    }
                    _ => {
                        // Unknown field — skip its body.
                        let sub_indent = detect_base_indent(lines, *pos);
                        if let Some(si) = sub_indent {
                            if si > base_indent {
                                while is_within_field(lines, *pos, si) {
                                    skip_blanks(lines, pos);
                                    if !is_within_field(lines, *pos, si) {
                                        break;
                                    }
                                    *pos += 1;
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                *pos += 1;
            }
        }
    }

    AgentContext {
        load_nodes,
        load_files,
        system_hint,
        max_tokens,
        load_memory,
    }
}

// ---------------------------------------------------------------------------
// Phase 3: parse_parallel_groups
// ---------------------------------------------------------------------------

/// Parses a `parallel_groups:` list into a `Vec<ParallelGroup>`.
///
/// On entry, `*pos` points to the first line after the `FieldStart("parallel_groups")`.
pub(crate) fn parse_parallel_groups(
    lines: &[Line],
    pos: &mut usize,
    errors: &mut Vec<AgmError>,
) -> Vec<ParallelGroup> {
    let base_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => return Vec::new(),
    };

    let mut groups = Vec::new();

    while is_within_field(lines, *pos, base_indent) {
        skip_blanks(lines, pos);
        if !is_within_field(lines, *pos, base_indent) {
            break;
        }

        match &lines[*pos].kind {
            LineKind::ListItem(text) => {
                let line_number = lines[*pos].number;
                let text = text.clone();
                *pos += 1;

                let mut fields: Vec<(String, SubFieldValue)> = Vec::new();

                if !text.is_empty() {
                    if let Some((k, v)) = parse_kv_from_text(&text) {
                        fields.push((k, SubFieldValue::Scalar(v)));
                    }
                }

                let sub_indent = detect_base_indent(lines, *pos);
                if let Some(si) = sub_indent {
                    if si > base_indent {
                        let mut sub = collect_sub_fields(lines, pos, si);
                        fields.append(&mut sub);
                    }
                }

                let group = match get_scalar(&fields, "group") {
                    Some(g) if !g.is_empty() => g.to_owned(),
                    _ => {
                        errors.push(AgmError::new(
                            ErrorCode::P003,
                            "parallel_groups entry missing required field: `group`",
                            ErrorLocation::new(None, Some(line_number), None),
                        ));
                        continue;
                    }
                };

                let nodes = get_list(&fields, "nodes")
                    .map(|s| s.to_vec())
                    .unwrap_or_default();

                let strategy_str = get_scalar(&fields, "strategy").unwrap_or("sequential");
                let strategy = strategy_str.parse::<Strategy>().unwrap_or_else(|_| {
                    errors.push(AgmError::new(
                        ErrorCode::P003,
                        format!("Invalid `strategy` value: {strategy_str:?}"),
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    Strategy::Sequential
                });

                let requires = get_list(&fields, "requires").map(|s| s.to_vec());

                let max_concurrency =
                    get_scalar(&fields, "max_concurrency").and_then(|s| s.parse::<u32>().ok());

                groups.push(ParallelGroup {
                    group,
                    nodes,
                    strategy,
                    requires,
                    max_concurrency,
                });
            }
            _ => break,
        }
    }

    groups
}

// ---------------------------------------------------------------------------
// Phase 4: parse_load_profiles
// ---------------------------------------------------------------------------

/// Parses a `load_profiles:` block into a `BTreeMap<String, LoadProfile>`.
///
/// On entry, `*pos` points to the first line after the `FieldStart("load_profiles")`.
pub(crate) fn parse_load_profiles(
    lines: &[Line],
    pos: &mut usize,
    errors: &mut Vec<AgmError>,
) -> BTreeMap<String, LoadProfile> {
    let base_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => return BTreeMap::new(),
    };

    let mut profiles = BTreeMap::new();

    while is_within_field(lines, *pos, base_indent) {
        skip_blanks(lines, pos);
        if !is_within_field(lines, *pos, base_indent) {
            break;
        }

        // Each profile starts with a `FieldStart(name)` at base_indent.
        match &lines[*pos].kind.clone() {
            LineKind::FieldStart(name) if lines[*pos].indent == base_indent => {
                let name = name.clone();
                *pos += 1;

                let sub_indent = match detect_base_indent(lines, *pos) {
                    Some(si) if si > base_indent => si,
                    _ => continue,
                };

                let sub_fields = collect_sub_fields(lines, pos, sub_indent);

                let filter = get_scalar(&sub_fields, "filter").unwrap_or("").to_owned();

                if filter.is_empty() {
                    errors.push(AgmError::new(
                        ErrorCode::P003,
                        format!("load_profile {name:?} missing required field: `filter`"),
                        ErrorLocation::new(None, None, None),
                    ));
                }

                let estimated_tokens = get_scalar(&sub_fields, "estimated_tokens")
                    .and_then(|s| s.parse::<TokenEstimate>().ok());

                profiles.insert(
                    name,
                    LoadProfile {
                        filter,
                        estimated_tokens,
                    },
                );
            }
            _ => {
                *pos += 1;
            }
        }
    }

    profiles
}

// ---------------------------------------------------------------------------
// Phase 4: parse_memory
// ---------------------------------------------------------------------------

/// Parses a `memory:` list into a `Vec<MemoryEntry>`.
///
/// On entry, `*pos` points to the first line after the `FieldStart("memory")`.
pub(crate) fn parse_memory(
    lines: &[Line],
    pos: &mut usize,
    errors: &mut Vec<AgmError>,
) -> Vec<MemoryEntry> {
    let base_indent = match detect_base_indent(lines, *pos) {
        Some(i) => i,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    while is_within_field(lines, *pos, base_indent) {
        skip_blanks(lines, pos);
        if !is_within_field(lines, *pos, base_indent) {
            break;
        }

        match &lines[*pos].kind {
            LineKind::ListItem(text) => {
                let line_number = lines[*pos].number;
                let text = text.clone();
                *pos += 1;

                let mut fields: Vec<(String, SubFieldValue)> = Vec::new();

                if !text.is_empty() {
                    if let Some((k, v)) = parse_kv_from_text(&text) {
                        fields.push((k, SubFieldValue::Scalar(v)));
                    }
                }

                let sub_indent = detect_base_indent(lines, *pos);
                if let Some(si) = sub_indent {
                    if si > base_indent {
                        let mut sub = collect_sub_fields(lines, pos, si);
                        fields.append(&mut sub);
                    }
                }

                let key = match get_scalar(&fields, "key") {
                    Some(k) if !k.is_empty() => k.to_owned(),
                    _ => {
                        errors.push(AgmError::new(
                            ErrorCode::P003,
                            "memory entry missing required field: `key`",
                            ErrorLocation::new(None, Some(line_number), None),
                        ));
                        continue;
                    }
                };

                let topic = match get_scalar(&fields, "topic") {
                    Some(t) if !t.is_empty() => t.to_owned(),
                    _ => {
                        errors.push(AgmError::new(
                            ErrorCode::P003,
                            "memory entry missing required field: `topic`",
                            ErrorLocation::new(None, Some(line_number), None),
                        ));
                        continue;
                    }
                };

                let action_str = get_scalar(&fields, "action").unwrap_or("");
                let action = match action_str.parse::<MemoryAction>() {
                    Ok(a) => a,
                    Err(_) => {
                        errors.push(AgmError::new(
                            ErrorCode::P003,
                            format!("memory entry missing or invalid `action`: {action_str:?}"),
                            ErrorLocation::new(None, Some(line_number), None),
                        ));
                        continue;
                    }
                };

                let value = get_scalar(&fields, "value")
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_owned());

                let scope =
                    get_scalar(&fields, "scope").and_then(|s| s.parse::<MemoryScope>().ok());

                let ttl = get_scalar(&fields, "ttl").and_then(|s| s.parse::<MemoryTtl>().ok());

                let query = get_scalar(&fields, "query")
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_owned());

                let max_results =
                    get_scalar(&fields, "max_results").and_then(|s| s.parse::<u32>().ok());

                entries.push(MemoryEntry {
                    key,
                    topic,
                    action,
                    value,
                    scope,
                    ttl,
                    query,
                    max_results,
                });
            }
            _ => break,
        }
    }

    entries
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::lexer::lex;

    /// Test utility: lex input and call the given parser starting at pos=0.
    fn parse_structured<F, T>(input: &str, parser: F) -> (T, Vec<AgmError>)
    where
        F: FnOnce(&[Line], &mut usize, &mut Vec<AgmError>) -> T,
    {
        let lines = lex(input).expect("lex failed");
        let mut pos = 0;
        let mut errors = Vec::new();
        let result = parser(&lines, &mut pos, &mut errors);
        (result, errors)
    }

    // -----------------------------------------------------------------------
    // A: parse_code_block tests (A1-A7)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_code_block_minimal_action_and_body_returns_code_block() {
        // A1
        let input = "  action: create\n  body: |
    fn main() {}
";
        let (cb, errors) = parse_structured(input, parse_code_block);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(cb.action, CodeAction::Create);
        assert_eq!(cb.body, "fn main() {}");
    }

    #[test]
    fn test_parse_code_block_all_fields_returns_full_code_block() {
        // A2
        let input = "  lang: rust\n  target: src/main.rs\n  action: append\n  body: |\n    fn foo() {}\n  anchor: // anchor\n";
        let (cb, errors) = parse_structured(input, parse_code_block);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(cb.lang.as_deref(), Some("rust"));
        assert_eq!(cb.target.as_deref(), Some("src/main.rs"));
        assert_eq!(cb.action, CodeAction::Append);
        assert_eq!(cb.body, "fn foo() {}");
        assert_eq!(cb.anchor.as_deref(), Some("// anchor"));
    }

    #[test]
    fn test_parse_code_block_missing_action_emits_v008_and_uses_fallback() {
        // A3
        let input = "  body: |\n    some code\n";
        let (cb, errors) = parse_structured(input, parse_code_block);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::V008),
            "expected V008"
        );
        assert_eq!(cb.action, CodeAction::Full);
        assert_eq!(cb.body, "some code");
    }

    #[test]
    fn test_parse_code_block_missing_body_emits_v008() {
        // A4
        let input = "  action: create\n";
        let (_cb, errors) = parse_structured(input, parse_code_block);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::V008),
            "expected V008 for missing body"
        );
    }

    #[test]
    fn test_parse_code_block_invalid_action_emits_p003_and_uses_fallback() {
        // A5
        let input = "  action: overwrite\n  body: |\n    code\n";
        let (cb, errors) = parse_structured(input, parse_code_block);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::P003),
            "expected P003"
        );
        assert_eq!(cb.action, CodeAction::Full);
    }

    #[test]
    fn test_parse_code_block_body_scalar_value_parsed_correctly() {
        // A6 - body: some text (not pipe)
        let input = "  action: full\n  body: inline body text\n";
        let (cb, errors) = parse_structured(input, parse_code_block);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(cb.body, "inline body text");
    }

    #[test]
    fn test_parse_code_block_with_old_field_returns_old() {
        // A7 - `old:` is a scalar field (only `body: |` triggers BodyMarker)
        let input = "  action: replace\n  body: |\n    new code\n  old: fn old_impl() {}\n";
        let (cb, errors) = parse_structured(input, parse_code_block);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(cb.action, CodeAction::Replace);
        assert_eq!(cb.old.as_deref(), Some("fn old_impl() {}"));
    }

    // -----------------------------------------------------------------------
    // B: parse_code_blocks tests (B8-B12)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_code_blocks_single_item_returns_one_block() {
        // B8
        let input = "  - action: create\n    body: |\n      fn main() {}\n";
        let (blocks, errors) = parse_structured(input, parse_code_blocks);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].action, CodeAction::Create);
    }

    #[test]
    fn test_parse_code_blocks_multiple_items_returns_all() {
        // B9
        let input = "  - action: create\n    body: |\n      fn a() {}\n  - action: append\n    body: |\n      fn b() {}\n";
        let (blocks, errors) = parse_structured(input, parse_code_blocks);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].action, CodeAction::Create);
        assert_eq!(blocks[1].action, CodeAction::Append);
    }

    #[test]
    fn test_parse_code_blocks_empty_returns_empty_vec() {
        // B10
        let input = "";
        let (blocks, errors) = parse_structured(input, parse_code_blocks);
        assert!(errors.is_empty());
        assert_eq!(blocks.len(), 0);
    }

    #[test]
    fn test_parse_code_blocks_item_missing_action_emits_v008() {
        // B11
        let input = "  - body: |\n      some code\n";
        let (_blocks, errors) = parse_structured(input, parse_code_blocks);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::V008),
            "expected V008"
        );
    }

    #[test]
    fn test_parse_code_blocks_stops_at_unindented_content() {
        // B12
        let input = "  - action: create\n    body: inline\nsummary: something\n";
        let (blocks, _errors) = parse_structured(input, parse_code_blocks);
        assert_eq!(blocks.len(), 1);
    }

    // -----------------------------------------------------------------------
    // C: parse_verify tests (C13-C20)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_verify_command_inline_returns_command_check() {
        // C13
        let input = "  - type: command\n    run: cargo check\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(checks.len(), 1);
        assert!(matches!(checks[0], VerifyCheck::Command { .. }));
    }

    #[test]
    fn test_parse_verify_command_with_expect_returns_check() {
        // C14
        let input = "  - type: command\n    run: cargo test\n    expect: exit_code_0\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        if let VerifyCheck::Command { run, expect } = &checks[0] {
            assert_eq!(run, "cargo test");
            assert_eq!(expect.as_deref(), Some("exit_code_0"));
        } else {
            panic!("expected Command check");
        }
    }

    #[test]
    fn test_parse_verify_file_exists_returns_file_exists_check() {
        // C15
        let input = "  - type: file_exists\n    file: src/main.rs\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(matches!(checks[0], VerifyCheck::FileExists { .. }));
    }

    #[test]
    fn test_parse_verify_file_contains_returns_file_contains_check() {
        // C16
        let input = "  - type: file_contains\n    file: src/lib.rs\n    pattern: fn main\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(matches!(checks[0], VerifyCheck::FileContains { .. }));
    }

    #[test]
    fn test_parse_verify_file_not_contains_returns_correct_check() {
        // C17
        let input = "  - type: file_not_contains\n    file: src/lib.rs\n    pattern: unsafe\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(matches!(checks[0], VerifyCheck::FileNotContains { .. }));
    }

    #[test]
    fn test_parse_verify_node_status_returns_node_status_check() {
        // C18
        let input = "  - type: node_status\n    node: auth.login\n    status: completed\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        if let VerifyCheck::NodeStatus { node, status } = &checks[0] {
            assert_eq!(node, "auth.login");
            assert_eq!(status, "completed");
        } else {
            panic!("expected NodeStatus check");
        }
    }

    #[test]
    fn test_parse_verify_missing_type_emits_v009_and_skips_entry() {
        // C19
        let input = "  - run: cargo check\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::V009),
            "expected V009"
        );
        assert_eq!(checks.len(), 0);
    }

    #[test]
    fn test_parse_verify_multiple_checks_returns_all() {
        // C20
        let input = "  - type: command\n    run: cargo check\n  - type: file_exists\n    file: Cargo.toml\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(checks.len(), 2);
    }

    // -----------------------------------------------------------------------
    // D: parse_agent_context tests (D21-D25)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_agent_context_system_hint_only() {
        // D21
        let input = "  system_hint: Rust project\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(ctx.system_hint.as_deref(), Some("Rust project"));
    }

    #[test]
    fn test_parse_agent_context_load_nodes_inline_list() {
        // D22
        let input = "  load_nodes: [auth.login, auth.session]\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let nodes = ctx.load_nodes.as_deref().unwrap();
        assert_eq!(nodes, &["auth.login", "auth.session"]);
    }

    #[test]
    fn test_parse_agent_context_max_tokens_parsed_as_u64() {
        // D23
        let input = "  max_tokens: 4000\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(ctx.max_tokens, Some(4000));
    }

    #[test]
    fn test_parse_agent_context_invalid_max_tokens_emits_p003() {
        // D24
        let input = "  max_tokens: not_a_number\n";
        let (_ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::P003),
            "expected P003"
        );
    }

    #[test]
    fn test_parse_agent_context_full_returns_all_fields() {
        // D25
        let input = "  system_hint: Rust project\n  max_tokens: 4000\n  load_nodes: [auth.login]\n  load_memory: [rust.repo]\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(ctx.system_hint.as_deref(), Some("Rust project"));
        assert_eq!(ctx.max_tokens, Some(4000));
        assert!(ctx.load_nodes.is_some());
        assert!(ctx.load_memory.is_some());
    }

    // -----------------------------------------------------------------------
    // E: parse_parallel_groups tests (E26-E29)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_parallel_groups_single_group_returns_one_group() {
        // E26
        let input =
            "  - group: 1-schema\n    nodes: [migration.schema]\n    strategy: sequential\n";
        let (groups, errors) = parse_structured(input, parse_parallel_groups);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group, "1-schema");
        assert_eq!(groups[0].strategy, Strategy::Sequential);
    }

    #[test]
    fn test_parse_parallel_groups_multiple_groups_returns_all() {
        // E27
        let input = "  - group: g1\n    nodes: [n.one]\n    strategy: sequential\n  - group: g2\n    nodes: [n.two]\n    strategy: parallel\n";
        let (groups, errors) = parse_structured(input, parse_parallel_groups);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_parse_parallel_groups_missing_group_field_emits_p003() {
        // E28
        let input = "  - nodes: [n.one]\n    strategy: sequential\n";
        let (_groups, errors) = parse_structured(input, parse_parallel_groups);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::P003),
            "expected P003"
        );
    }

    #[test]
    fn test_parse_parallel_groups_with_requires_and_max_concurrency() {
        // E29
        let input = "  - group: g1\n    nodes: [n.one, n.two]\n    strategy: parallel\n    requires: [g0]\n    max_concurrency: 4\n";
        let (groups, errors) = parse_structured(input, parse_parallel_groups);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(groups[0].requires.as_deref(), Some(&["g0".to_owned()][..]));
        assert_eq!(groups[0].max_concurrency, Some(4));
    }

    // -----------------------------------------------------------------------
    // F: parse_load_profiles tests (F30-F33)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_load_profiles_single_profile_returns_one_entry() {
        // F30
        let input = "  summary:\n    filter: type in [facts]\n";
        let (profiles, errors) = parse_structured(input, parse_load_profiles);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(profiles.contains_key("summary"));
        assert_eq!(profiles["summary"].filter, "type in [facts]");
    }

    #[test]
    fn test_parse_load_profiles_multiple_profiles_returns_all() {
        // F31
        let input = "  summary:\n    filter: type in [facts]\n  operational:\n    filter: priority in [critical]\n";
        let (profiles, errors) = parse_structured(input, parse_load_profiles);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(profiles.len(), 2);
        assert!(profiles.contains_key("summary"));
        assert!(profiles.contains_key("operational"));
    }

    #[test]
    fn test_parse_load_profiles_with_estimated_tokens() {
        // F32
        let input = "  summary:\n    filter: type in [facts]\n    estimated_tokens: 1200\n";
        let (profiles, errors) = parse_structured(input, parse_load_profiles);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(
            profiles["summary"].estimated_tokens,
            Some(TokenEstimate::Count(1200))
        );
    }

    #[test]
    fn test_parse_load_profiles_missing_filter_emits_p003() {
        // F33
        let input = "  summary:\n    estimated_tokens: 1200\n";
        let (_profiles, errors) = parse_structured(input, parse_load_profiles);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::P003),
            "expected P003 for missing filter"
        );
    }

    // -----------------------------------------------------------------------
    // G: parse_memory tests (G34-G38)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_memory_upsert_entry_returns_memory_entry() {
        // G34
        let input = "  - key: repo.pattern\n    topic: rust.repository\n    action: upsert\n    value: row_to_column uses get()\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "repo.pattern");
        assert_eq!(entries[0].action, MemoryAction::Upsert);
    }

    #[test]
    fn test_parse_memory_multiple_entries_returns_all() {
        // G35
        let input = "  - key: k1\n    topic: t1\n    action: get\n  - key: k2\n    topic: t2\n    action: list\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_parse_memory_missing_key_emits_p003_and_skips_entry() {
        // G36
        let input = "  - topic: t1\n    action: get\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::P003),
            "expected P003"
        );
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_memory_invalid_action_emits_p003_and_skips_entry() {
        // G37
        let input = "  - key: k1\n    topic: t1\n    action: invalid_action\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::P003),
            "expected P003"
        );
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_memory_with_scope_and_ttl() {
        // G38
        let input = "  - key: k1\n    topic: t1\n    action: upsert\n    scope: project\n    ttl: permanent\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(entries[0].scope, Some(MemoryScope::Project));
        assert_eq!(entries[0].ttl, Some(MemoryTtl::Permanent));
    }

    // -----------------------------------------------------------------------
    // H: edge case / error-path coverage
    // -----------------------------------------------------------------------

    // --- H1. parse_kv_from_text via parse_code_blocks inline-kv path ------
    #[test]
    fn test_parse_code_blocks_dash_with_inline_action_kv() {
        // List items whose dash line carries the first kv pair, body on
        // continuation lines. Covers the `parse_kv_from_text` + inline-kv
        // merge path inside `parse_code_blocks`.
        let input = "  - action: create\n    body: |\n      fn a() {}\n";
        let (blocks, errors) = parse_structured(input, parse_code_blocks);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].action, CodeAction::Create);
    }

    // --- H2. parse_code_block on empty input (detect_base_indent → None) --
    #[test]
    fn test_parse_code_block_empty_input_emits_two_v008_errors() {
        let input = "";
        let (cb, errors) = parse_structured(input, parse_code_block);
        let v008_count = errors.iter().filter(|e| e.code == ErrorCode::V008).count();
        assert_eq!(
            v008_count, 2,
            "expected 2 V008 errors (missing action + body), got {v008_count}"
        );
        assert_eq!(cb.action, CodeAction::Full);
        assert!(cb.body.is_empty());
    }

    // --- H3. parse_code_blocks on empty input (detect_base_indent → None) -
    #[test]
    fn test_parse_code_blocks_no_content_returns_empty_vec() {
        let input = "";
        let (blocks, errors) = parse_structured(input, parse_code_blocks);
        assert!(errors.is_empty());
        assert!(blocks.is_empty());
    }

    // --- H4. parse_verify on empty input (detect_base_indent → None) ------
    #[test]
    fn test_parse_verify_no_content_returns_empty_vec() {
        let input = "";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.is_empty());
        assert!(checks.is_empty());
    }

    // --- H5. parse_verify command missing `run` emits V009 ----------------
    #[test]
    fn test_parse_verify_command_missing_run_emits_v009() {
        let input = "  - type: command\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
        assert!(checks.is_empty());
    }

    // --- H6. parse_verify file_exists missing `file` emits V009 -----------
    #[test]
    fn test_parse_verify_file_exists_missing_file_emits_v009() {
        let input = "  - type: file_exists\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
        assert!(checks.is_empty());
    }

    // --- H7. parse_verify file_contains missing `file` or `pattern` -------
    #[test]
    fn test_parse_verify_file_contains_missing_file_emits_v009() {
        let input = "  - type: file_contains\n    pattern: foo\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
        assert!(checks.is_empty());
    }

    #[test]
    fn test_parse_verify_file_contains_missing_pattern_emits_v009() {
        let input = "  - type: file_contains\n    file: src/lib.rs\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
        assert!(checks.is_empty());
    }

    // --- H8. parse_verify file_not_contains missing fields ---------------
    #[test]
    fn test_parse_verify_file_not_contains_missing_file_emits_v009() {
        let input = "  - type: file_not_contains\n    pattern: unsafe\n";
        let (_checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
    }

    #[test]
    fn test_parse_verify_file_not_contains_missing_pattern_emits_v009() {
        let input = "  - type: file_not_contains\n    file: src/lib.rs\n";
        let (_checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
    }

    // --- H9. parse_verify node_status missing fields ---------------------
    #[test]
    fn test_parse_verify_node_status_missing_node_emits_v009() {
        let input = "  - type: node_status\n    status: completed\n";
        let (_checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
    }

    #[test]
    fn test_parse_verify_node_status_missing_status_emits_v009() {
        let input = "  - type: node_status\n    node: auth.login\n";
        let (_checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::V009));
    }

    // --- H10. parse_verify unknown type emits P003 -----------------------
    #[test]
    fn test_parse_verify_unknown_type_emits_p003() {
        let input = "  - type: magic\n    foo: bar\n";
        let (checks, errors) = parse_structured(input, parse_verify);
        assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
        assert!(checks.is_empty());
    }

    // --- H11. parse_agent_context load_files (block list) ----------------
    #[test]
    fn test_parse_agent_context_load_files_block_list_all_range_variants() {
        let input = "  load_files:\n    - path: src/main.rs\n      range: full\n    - path: src/util.rs\n      range: 1-50\n    - path: src/other.rs\n      range: function: do_work\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let files = ctx.load_files.as_deref().unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].range, FileRange::Full);
        assert_eq!(files[1].range, FileRange::Lines(1, 50));
        assert_eq!(files[2].range, FileRange::Function("do_work".to_owned()));
    }

    // --- H12. parse_agent_context load_files missing path emits P003 -----
    #[test]
    fn test_parse_agent_context_load_files_missing_path_emits_p003() {
        let input = "  load_files:\n    - range: full\n";
        let (_ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
    }

    // --- H13. parse_agent_context load_nodes as block list ---------------
    #[test]
    fn test_parse_agent_context_load_nodes_block_list() {
        let input = "  load_nodes:\n    - auth.login\n    - auth.session\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let nodes = ctx.load_nodes.as_deref().unwrap();
        assert_eq!(nodes, &["auth.login", "auth.session"]);
    }

    // --- H14. parse_agent_context load_memory as block list --------------
    #[test]
    fn test_parse_agent_context_load_memory_block_list() {
        let input = "  load_memory:\n    - topic.one\n    - topic.two\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let mem = ctx.load_memory.as_deref().unwrap();
        assert_eq!(mem, &["topic.one", "topic.two"]);
    }

    // --- H15. parse_agent_context unknown indented field is skipped -----
    #[test]
    fn test_parse_agent_context_unknown_field_skipped_without_error() {
        let input = "  system_hint: hello\n  unknown_extension:\n    - some\n    - thing\n  max_tokens: 100\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(ctx.system_hint.as_deref(), Some("hello"));
        assert_eq!(ctx.max_tokens, Some(100));
    }

    // --- H16. parse_agent_context empty input returns all-None ----------
    #[test]
    fn test_parse_agent_context_empty_input_returns_all_none() {
        let input = "";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty());
        assert!(ctx.system_hint.is_none());
        assert!(ctx.max_tokens.is_none());
        assert!(ctx.load_nodes.is_none());
        assert!(ctx.load_files.is_none());
        assert!(ctx.load_memory.is_none());
    }

    // --- H17. parse_parallel_groups invalid strategy emits P003 ---------
    #[test]
    fn test_parse_parallel_groups_invalid_strategy_emits_p003() {
        let input = "  - group: g1\n    nodes: [n.a]\n    strategy: bogus\n";
        let (groups, errors) = parse_structured(input, parse_parallel_groups);
        assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
        // Group still builds with fallback strategy.
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].strategy, Strategy::Sequential);
    }

    // --- H18. parse_parallel_groups empty input returns empty -----------
    #[test]
    fn test_parse_parallel_groups_empty_input_returns_empty() {
        let input = "";
        let (groups, errors) = parse_structured(input, parse_parallel_groups);
        assert!(errors.is_empty());
        assert!(groups.is_empty());
    }

    // --- H19. parse_load_profiles empty input returns empty -------------
    #[test]
    fn test_parse_load_profiles_empty_input_returns_empty() {
        let input = "";
        let (profiles, errors) = parse_structured(input, parse_load_profiles);
        assert!(errors.is_empty());
        assert!(profiles.is_empty());
    }

    // --- H20. parse_memory empty input returns empty --------------------
    #[test]
    fn test_parse_memory_empty_input_returns_empty() {
        let input = "";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(errors.is_empty());
        assert!(entries.is_empty());
    }

    // --- H21. parse_memory missing topic emits P003 --------------------
    #[test]
    fn test_parse_memory_missing_topic_emits_p003_and_skips() {
        let input = "  - key: k1\n    action: get\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
        assert!(entries.is_empty());
    }

    // --- H22. parse_memory entry with query + max_results + value -------
    #[test]
    fn test_parse_memory_with_query_max_results_and_value() {
        let input = "  - key: k1\n    topic: t1\n    action: search\n    value: some value\n    query: pattern\n    max_results: 5\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(entries[0].value.as_deref(), Some("some value"));
        assert_eq!(entries[0].query.as_deref(), Some("pattern"));
        assert_eq!(entries[0].max_results, Some(5));
    }

    // --- H23. parse_memory with ttl duration and session scope ----------
    #[test]
    fn test_parse_memory_session_scope_and_duration_ttl() {
        let input = "  - key: sess.k\n    topic: t\n    action: upsert\n    scope: session\n    ttl: duration:P1D\n";
        let (entries, errors) = parse_structured(input, parse_memory);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(entries[0].scope, Some(MemoryScope::Session));
        assert_eq!(entries[0].ttl, Some(MemoryTtl::Duration("P1D".to_owned())));
    }

    // --- H24. parse_file_range all branches (via parse_load_files_list) -
    #[test]
    fn test_parse_file_range_helper_all_branches() {
        // Exercise the parse_file_range helper end-to-end through load_files:
        // "full", numeric range "10-20", "function: name", invalid "abc".
        let input = "  load_files:\n    - path: a.rs\n      range: full\n    - path: b.rs\n      range: 10-20\n    - path: c.rs\n      range: function: work\n    - path: d.rs\n      range: abc\n";
        let (ctx, errors) = parse_structured(input, parse_agent_context);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let files = ctx.load_files.as_deref().unwrap();
        assert_eq!(files[0].range, FileRange::Full);
        assert_eq!(files[1].range, FileRange::Lines(10, 20));
        assert_eq!(files[2].range, FileRange::Function("work".to_owned()));
        // Invalid falls back to Full.
        assert_eq!(files[3].range, FileRange::Full);
    }

    // --- H25. parse_code_block with pipe body that has blank line -------
    #[test]
    fn test_parse_code_block_pipe_body_preserves_internal_blank_line() {
        // A blank line in the middle of a pipe body should be preserved if
        // more body follows (collect_pipe_body lookahead path).
        let input = "  action: create\n  body: |\n    line one\n\n    line three\n";
        let (cb, errors) = parse_structured(input, parse_code_block);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(cb.body, "line one\n\nline three");
    }
}
