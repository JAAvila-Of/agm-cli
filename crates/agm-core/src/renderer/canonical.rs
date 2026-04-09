//! Canonical AGM text emitter (spec S19.6).
//!
//! Re-emits an `AgmFile` as AGM text in the canonical field order.
//! The output is a valid AGM file that, when parsed, produces a
//! semantically identical AST. Comments are not preserved.

use std::collections::BTreeMap;

use crate::model::code::CodeBlock;
use crate::model::context::AgentContext;
use crate::model::fields::FieldValue;
use crate::model::file::{AgmFile, LoadProfile, TokenEstimate};
use crate::model::imports::ImportEntry;
use crate::model::memory::MemoryEntry;
use crate::model::node::Node;
use crate::model::orchestration::ParallelGroup;
use crate::model::verify::VerifyCheck;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Re-emits the `AgmFile` as AGM text in canonical field order.
///
/// The output is a valid AGM file that, when parsed, produces a
/// semantically identical AST. Comments are not preserved.
#[must_use]
pub fn render_canonical(file: &AgmFile) -> String {
    let mut buf = String::new();

    // Header
    emit_scalar(&mut buf, "agm", &file.header.agm);
    emit_scalar(&mut buf, "package", &file.header.package);
    emit_scalar(&mut buf, "version", &file.header.version);
    emit_opt_scalar(&mut buf, "title", &file.header.title);
    emit_opt_scalar(&mut buf, "owner", &file.header.owner);

    if let Some(ref imports) = file.header.imports {
        emit_imports(&mut buf, imports);
    }

    emit_opt_scalar(&mut buf, "default_load", &file.header.default_load);
    emit_opt_block_or_scalar(&mut buf, "description", &file.header.description);
    emit_opt_list(&mut buf, "tags", &file.header.tags);
    emit_opt_scalar(&mut buf, "status", &file.header.status);

    if let Some(ref profiles) = file.header.load_profiles {
        emit_load_profiles(&mut buf, profiles);
    }

    emit_opt_scalar(&mut buf, "target_runtime", &file.header.target_runtime);

    // Nodes
    for node in &file.nodes {
        buf.push('\n');
        emit_node(&mut buf, node);
    }

    // Ensure single trailing newline
    if !buf.ends_with('\n') {
        buf.push('\n');
    }

    buf
}

// ---------------------------------------------------------------------------
// Node emission
// ---------------------------------------------------------------------------

fn emit_node(buf: &mut String, node: &Node) {
    buf.push_str(&format!("node {}\n", node.id));

    // Group 1: Identity
    emit_scalar(buf, "type", &node.node_type.to_string());

    // Group 2: Control
    emit_opt_enum(buf, "status", &node.status);
    emit_opt_enum(buf, "stability", &node.stability);
    emit_opt_enum(buf, "priority", &node.priority);
    emit_opt_enum(buf, "confidence", &node.confidence);

    // Group 3: Relations
    emit_opt_list(buf, "depends", &node.depends);
    emit_opt_list(buf, "related_to", &node.related_to);
    emit_opt_list(buf, "replaces", &node.replaces);
    emit_opt_list(buf, "conflicts", &node.conflicts);
    emit_opt_list(buf, "see_also", &node.see_also);

    // Group 4: I/O
    emit_opt_list(buf, "input", &node.input);
    emit_opt_list(buf, "output", &node.output);

    // Group 5: Summary
    emit_scalar(buf, "summary", &node.summary);

    // Group 6: Operational
    emit_opt_list(buf, "items", &node.items);
    emit_opt_list(buf, "steps", &node.steps);
    emit_opt_list(buf, "fields", &node.fields);

    // Group 7: Executable
    if let Some(ref cb) = node.code {
        emit_code_block(buf, "code", cb);
    }
    if let Some(ref blocks) = node.code_blocks {
        emit_code_blocks(buf, blocks);
    }
    if let Some(ref checks) = node.verify {
        emit_verify_checks(buf, checks);
    }
    if let Some(ref ctx) = node.agent_context {
        emit_agent_context(buf, ctx);
    }
    emit_opt_scalar(buf, "target", &node.target);

    // Group 8: Memory
    if let Some(ref entries) = node.memory {
        emit_memory_entries(buf, entries);
    }

    // Group 9: Execution
    emit_opt_enum(buf, "execution_status", &node.execution_status);
    emit_opt_scalar(buf, "executed_by", &node.executed_by);
    emit_opt_scalar(buf, "executed_at", &node.executed_at);
    emit_opt_block_or_scalar(buf, "execution_log", &node.execution_log);
    if let Some(n) = node.retry_count {
        emit_scalar(buf, "retry_count", &n.to_string());
    }

    // Group 10: Orchestration
    if let Some(ref groups) = node.parallel_groups {
        emit_parallel_groups(buf, groups);
    }

    // Group 11: Explanatory
    emit_opt_block_or_scalar(buf, "detail", &node.detail);
    emit_opt_list(buf, "rationale", &node.rationale);
    emit_opt_list(buf, "tradeoffs", &node.tradeoffs);
    emit_opt_list(buf, "resolution", &node.resolution);
    emit_opt_block_or_scalar(buf, "examples", &node.examples);
    emit_opt_block_or_scalar(buf, "notes", &node.notes);

    // Group 12: Context
    emit_opt_list(buf, "scope", &node.scope);
    emit_opt_block_or_scalar(buf, "applies_when", &node.applies_when);
    emit_opt_scalar(buf, "valid_from", &node.valid_from);
    emit_opt_scalar(buf, "valid_until", &node.valid_until);
    emit_opt_list(buf, "tags", &node.tags);
    emit_opt_list(buf, "aliases", &node.aliases);
    emit_opt_list(buf, "keywords", &node.keywords);

    // Group 13: Extension (sorted by key)
    for (k, v) in &node.extra_fields {
        match v {
            FieldValue::Scalar(s) => emit_scalar(buf, k, s),
            FieldValue::Block(s) => emit_block_or_scalar(buf, k, s),
            FieldValue::List(items) => {
                emit_list_raw(buf, k, items.iter().map(String::as_str).collect());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Primitive emission helpers
// ---------------------------------------------------------------------------

fn emit_scalar(buf: &mut String, key: &str, val: &str) {
    buf.push_str(key);
    buf.push_str(": ");
    buf.push_str(val);
    buf.push('\n');
}

fn emit_opt_scalar(buf: &mut String, key: &str, val: &Option<String>) {
    if let Some(s) = val {
        emit_scalar(buf, key, s);
    }
}

fn emit_opt_enum<T: std::fmt::Display>(buf: &mut String, key: &str, val: &Option<T>) {
    if let Some(v) = val {
        emit_scalar(buf, key, &v.to_string());
    }
}

fn emit_block_or_scalar(buf: &mut String, key: &str, val: &str) {
    if val.contains('\n') {
        buf.push_str(key);
        buf.push_str(":\n");
        for line in val.lines() {
            buf.push_str("  ");
            buf.push_str(line);
            buf.push('\n');
        }
    } else {
        emit_scalar(buf, key, val);
    }
}

fn emit_opt_block_or_scalar(buf: &mut String, key: &str, val: &Option<String>) {
    if let Some(s) = val {
        emit_block_or_scalar(buf, key, s);
    }
}

fn emit_list_raw(buf: &mut String, key: &str, items: Vec<&str>) {
    if items.is_empty() {
        return;
    }

    let use_inline = items.len() <= 3
        && items
            .iter()
            .all(|s| !s.contains(',') && !s.contains('[') && !s.contains(']'));

    if use_inline {
        buf.push_str(key);
        buf.push_str(": [");
        buf.push_str(&items.join(", "));
        buf.push_str("]\n");
    } else {
        buf.push_str(key);
        buf.push_str(":\n");
        for item in items {
            buf.push_str("  - ");
            buf.push_str(item);
            buf.push('\n');
        }
    }
}

fn emit_opt_list(buf: &mut String, key: &str, val: &Option<Vec<String>>) {
    if let Some(items) = val {
        if items.is_empty() {
            return;
        }
        let refs: Vec<&str> = items.iter().map(String::as_str).collect();
        emit_list_raw(buf, key, refs);
    }
}

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

fn emit_imports(buf: &mut String, imports: &[ImportEntry]) {
    if imports.is_empty() {
        return;
    }
    let items: Vec<String> = imports.iter().map(|e| e.to_string()).collect();
    let refs: Vec<&str> = items.iter().map(String::as_str).collect();
    // Imports always use indented list (they tend to have @ chars which may cause issues)
    buf.push_str("imports:\n");
    for item in refs {
        buf.push_str("  - ");
        buf.push_str(item);
        buf.push('\n');
    }
}

// ---------------------------------------------------------------------------
// Load profiles
// ---------------------------------------------------------------------------

fn emit_load_profiles(buf: &mut String, profiles: &BTreeMap<String, LoadProfile>) {
    if profiles.is_empty() {
        return;
    }
    buf.push_str("load_profiles:\n");
    for (name, lp) in profiles {
        buf.push_str("  ");
        buf.push_str(name);
        buf.push_str(":\n");
        buf.push_str("    filter: ");
        buf.push_str(&lp.filter);
        buf.push('\n');
        if let Some(ref est) = lp.estimated_tokens {
            buf.push_str("    estimated_tokens: ");
            let s = match est {
                TokenEstimate::Count(n) => n.to_string(),
                TokenEstimate::Variable => "variable".into(),
            };
            buf.push_str(&s);
            buf.push('\n');
        }
    }
}

// ---------------------------------------------------------------------------
// Structured field emitters
// ---------------------------------------------------------------------------

fn emit_code_block(buf: &mut String, prefix: &str, cb: &CodeBlock) {
    buf.push_str(prefix);
    buf.push_str(":\n");
    if let Some(ref lang) = cb.lang {
        buf.push_str("  lang: ");
        buf.push_str(lang);
        buf.push('\n');
    }
    if let Some(ref target) = cb.target {
        buf.push_str("  target: ");
        buf.push_str(target);
        buf.push('\n');
    }
    buf.push_str("  action: ");
    buf.push_str(&cb.action.to_string());
    buf.push('\n');
    // body as block
    buf.push_str("  body:\n");
    for line in cb.body.lines() {
        buf.push_str("    ");
        buf.push_str(line);
        buf.push('\n');
    }
    if cb.body.is_empty() {
        buf.push_str("    \n");
    }
    if let Some(ref anchor) = cb.anchor {
        buf.push_str("  anchor: ");
        buf.push_str(anchor);
        buf.push('\n');
    }
    if let Some(ref old) = cb.old {
        buf.push_str("  old:\n");
        for line in old.lines() {
            buf.push_str("    ");
            buf.push_str(line);
            buf.push('\n');
        }
    }
}

fn emit_code_blocks(buf: &mut String, blocks: &[CodeBlock]) {
    if blocks.is_empty() {
        return;
    }
    buf.push_str("code_blocks:\n");
    for cb in blocks {
        buf.push_str("  - ");
        if let Some(ref lang) = cb.lang {
            buf.push_str("lang: ");
            buf.push_str(lang);
            buf.push('\n');
        } else {
            buf.push_str("action: ");
            buf.push_str(&cb.action.to_string());
            buf.push('\n');
        }
        if let Some(ref lang) = cb.lang {
            let _ = lang; // already emitted
            // emit remaining fields with 4-space indent
            if let Some(ref target) = cb.target {
                buf.push_str("    target: ");
                buf.push_str(target);
                buf.push('\n');
            }
            buf.push_str("    action: ");
            buf.push_str(&cb.action.to_string());
            buf.push('\n');
        }
        buf.push_str("    body:\n");
        for line in cb.body.lines() {
            buf.push_str("      ");
            buf.push_str(line);
            buf.push('\n');
        }
        if let Some(ref anchor) = cb.anchor {
            buf.push_str("    anchor: ");
            buf.push_str(anchor);
            buf.push('\n');
        }
        if let Some(ref old) = cb.old {
            buf.push_str("    old:\n");
            for line in old.lines() {
                buf.push_str("      ");
                buf.push_str(line);
                buf.push('\n');
            }
        }
    }
}

fn emit_verify_checks(buf: &mut String, checks: &[VerifyCheck]) {
    if checks.is_empty() {
        return;
    }
    buf.push_str("verify:\n");
    for check in checks {
        match check {
            VerifyCheck::Command { run, expect } => {
                buf.push_str("  - type: command\n");
                buf.push_str("    run: ");
                buf.push_str(run);
                buf.push('\n');
                if let Some(exp) = expect {
                    buf.push_str("    expect: ");
                    buf.push_str(exp);
                    buf.push('\n');
                }
            }
            VerifyCheck::FileExists { file } => {
                buf.push_str("  - type: file_exists\n");
                buf.push_str("    file: ");
                buf.push_str(file);
                buf.push('\n');
            }
            VerifyCheck::FileContains { file, pattern } => {
                buf.push_str("  - type: file_contains\n");
                buf.push_str("    file: ");
                buf.push_str(file);
                buf.push('\n');
                buf.push_str("    pattern: ");
                buf.push_str(pattern);
                buf.push('\n');
            }
            VerifyCheck::FileNotContains { file, pattern } => {
                buf.push_str("  - type: file_not_contains\n");
                buf.push_str("    file: ");
                buf.push_str(file);
                buf.push('\n');
                buf.push_str("    pattern: ");
                buf.push_str(pattern);
                buf.push('\n');
            }
            VerifyCheck::NodeStatus { node, status } => {
                buf.push_str("  - type: node_status\n");
                buf.push_str("    node: ");
                buf.push_str(node);
                buf.push('\n');
                buf.push_str("    status: ");
                buf.push_str(status);
                buf.push('\n');
            }
        }
    }
}

fn emit_agent_context(buf: &mut String, ctx: &AgentContext) {
    buf.push_str("agent_context:\n");
    if let Some(ref nodes) = ctx.load_nodes {
        let refs: Vec<&str> = nodes.iter().map(String::as_str).collect();
        let use_inline = refs.len() <= 3
            && refs
                .iter()
                .all(|s| !s.contains(',') && !s.contains('[') && !s.contains(']'));
        if use_inline {
            buf.push_str("  load_nodes: [");
            buf.push_str(&refs.join(", "));
            buf.push_str("]\n");
        } else {
            buf.push_str("  load_nodes:\n");
            for n in refs {
                buf.push_str("    - ");
                buf.push_str(n);
                buf.push('\n');
            }
        }
    }
    if let Some(ref files) = ctx.load_files {
        buf.push_str("  load_files:\n");
        for lf in files {
            buf.push_str("    - path: ");
            buf.push_str(&lf.path);
            buf.push('\n');
            let range_str = match &lf.range {
                crate::model::context::FileRange::Full => "full".to_owned(),
                crate::model::context::FileRange::Lines(s, e) => format!("{s}-{e}"),
                crate::model::context::FileRange::Function(name) => {
                    format!("function: {name}")
                }
            };
            buf.push_str("      range: ");
            buf.push_str(&range_str);
            buf.push('\n');
        }
    }
    if let Some(ref hint) = ctx.system_hint {
        emit_block_or_scalar_indented(buf, "  system_hint", hint);
    }
    if let Some(max) = ctx.max_tokens {
        buf.push_str("  max_tokens: ");
        buf.push_str(&max.to_string());
        buf.push('\n');
    }
    if let Some(ref mem) = ctx.load_memory {
        let refs: Vec<&str> = mem.iter().map(String::as_str).collect();
        let use_inline = refs.len() <= 3
            && refs
                .iter()
                .all(|s| !s.contains(',') && !s.contains('[') && !s.contains(']'));
        if use_inline {
            buf.push_str("  load_memory: [");
            buf.push_str(&refs.join(", "));
            buf.push_str("]\n");
        } else {
            buf.push_str("  load_memory:\n");
            for n in refs {
                buf.push_str("    - ");
                buf.push_str(n);
                buf.push('\n');
            }
        }
    }
}

fn emit_block_or_scalar_indented(buf: &mut String, prefixed_key: &str, val: &str) {
    if val.contains('\n') {
        buf.push_str(prefixed_key);
        buf.push_str(":\n");
        let indent = "    "; // 4-space for nested
        for line in val.lines() {
            buf.push_str(indent);
            buf.push_str(line);
            buf.push('\n');
        }
    } else {
        buf.push_str(prefixed_key);
        buf.push_str(": ");
        buf.push_str(val);
        buf.push('\n');
    }
}

fn emit_memory_entries(buf: &mut String, entries: &[MemoryEntry]) {
    if entries.is_empty() {
        return;
    }
    buf.push_str("memory:\n");
    for entry in entries {
        buf.push_str("  - key: ");
        buf.push_str(&entry.key);
        buf.push('\n');
        buf.push_str("    topic: ");
        buf.push_str(&entry.topic);
        buf.push('\n');
        buf.push_str("    action: ");
        buf.push_str(&entry.action.to_string());
        buf.push('\n');
        if let Some(ref v) = entry.value {
            buf.push_str("    value: ");
            buf.push_str(v);
            buf.push('\n');
        }
        if let Some(ref s) = entry.scope {
            buf.push_str("    scope: ");
            buf.push_str(&s.to_string());
            buf.push('\n');
        }
        if let Some(ref t) = entry.ttl {
            buf.push_str("    ttl: ");
            buf.push_str(&t.to_string());
            buf.push('\n');
        }
        if let Some(ref q) = entry.query {
            buf.push_str("    query: ");
            buf.push_str(q);
            buf.push('\n');
        }
        if let Some(max) = entry.max_results {
            buf.push_str("    max_results: ");
            buf.push_str(&max.to_string());
            buf.push('\n');
        }
    }
}

fn emit_parallel_groups(buf: &mut String, groups: &[ParallelGroup]) {
    if groups.is_empty() {
        return;
    }
    buf.push_str("parallel_groups:\n");
    for group in groups {
        buf.push_str("  - group: ");
        buf.push_str(&group.group);
        buf.push('\n');
        let node_refs: Vec<&str> = group.nodes.iter().map(String::as_str).collect();
        let use_inline = node_refs.len() <= 3
            && node_refs
                .iter()
                .all(|s| !s.contains(',') && !s.contains('[') && !s.contains(']'));
        if use_inline {
            buf.push_str("    nodes: [");
            buf.push_str(&node_refs.join(", "));
            buf.push_str("]\n");
        } else {
            buf.push_str("    nodes:\n");
            for n in node_refs {
                buf.push_str("      - ");
                buf.push_str(n);
                buf.push('\n');
            }
        }
        buf.push_str("    strategy: ");
        buf.push_str(&group.strategy.to_string());
        buf.push('\n');
        if let Some(ref reqs) = group.requires {
            let req_refs: Vec<&str> = reqs.iter().map(String::as_str).collect();
            let use_inline = req_refs.len() <= 3
                && req_refs
                    .iter()
                    .all(|s| !s.contains(',') && !s.contains('[') && !s.contains(']'));
            if use_inline {
                buf.push_str("    requires: [");
                buf.push_str(&req_refs.join(", "));
                buf.push_str("]\n");
            } else {
                buf.push_str("    requires:\n");
                for r in req_refs {
                    buf.push_str("      - ");
                    buf.push_str(r);
                    buf.push('\n');
                }
            }
        }
        if let Some(max) = group.max_concurrency {
            buf.push_str("    max_concurrency: ");
            buf.push_str(&max.to_string());
            buf.push('\n');
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::{NodeType, Priority, Span, Stability};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;
    use crate::model::verify::VerifyCheck;
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
                span: Span::default(),
            }],
        }
    }

    #[test]
    fn test_render_canonical_field_order() {
        let mut file = minimal_file();
        file.nodes[0].priority = Some(Priority::Critical);
        file.nodes[0].stability = Some(Stability::High);
        file.nodes[0].depends = Some(vec!["a.dep".into()]);
        file.nodes[0].summary = "ordered test".into();

        let output = render_canonical(&file);

        // type before status before summary before depends
        let type_pos = output.find("type:").unwrap();
        let summary_pos = output.find("summary:").unwrap();
        let depends_pos = output.find("depends:").unwrap();

        assert!(type_pos < depends_pos, "type must come before depends");
        assert!(
            depends_pos < summary_pos,
            "depends must come before summary"
        );
    }

    #[test]
    fn test_render_canonical_block_text_indented() {
        let mut file = minimal_file();
        file.nodes[0].detail = Some("line one\nline two".into());
        let output = render_canonical(&file);
        assert!(output.contains("detail:\n  line one\n  line two"));
    }

    #[test]
    fn test_render_canonical_inline_list() {
        let mut file = minimal_file();
        file.nodes[0].depends = Some(vec!["a.one".into(), "a.two".into()]);
        let output = render_canonical(&file);
        assert!(output.contains("depends: [a.one, a.two]"));
    }

    #[test]
    fn test_render_canonical_indented_list() {
        let mut file = minimal_file();
        file.nodes[0].items = Some(vec!["a".into(), "b".into(), "c".into(), "d".into()]);
        let output = render_canonical(&file);
        assert!(output.contains("items:\n  - a\n  - b\n  - c\n  - d"));
    }

    #[test]
    fn test_render_canonical_ends_with_single_newline() {
        let file = minimal_file();
        let output = render_canonical(&file);
        assert!(output.ends_with('\n'));
        assert!(!output.ends_with("\n\n"));
    }

    #[test]
    fn test_render_canonical_verify_checks() {
        let mut file = minimal_file();
        file.nodes[0].verify = Some(vec![VerifyCheck::Command {
            run: "cargo check".into(),
            expect: Some("exit_code_0".into()),
        }]);
        let output = render_canonical(&file);
        assert!(output.contains("verify:\n"));
        assert!(output.contains("  - type: command\n"));
        assert!(output.contains("    run: cargo check\n"));
    }

    #[test]
    fn test_render_canonical_minimal_contains_required_fields() {
        let file = minimal_file();
        let output = render_canonical(&file);
        assert!(output.contains("agm: 1\n"));
        assert!(output.contains("package: test.minimal\n"));
        assert!(output.contains("version: 0.1.0\n"));
        assert!(output.contains("node test.node\n"));
        assert!(output.contains("type: facts\n"));
        assert!(output.contains("summary: a minimal test node\n"));
    }

    #[test]
    fn test_render_canonical_inline_list_with_comma_uses_indented() {
        let mut file = minimal_file();
        // item contains comma -> should use indented form
        file.nodes[0].items = Some(vec!["key: val, extra".into(), "other".into()]);
        let output = render_canonical(&file);
        assert!(output.contains("items:\n  - key: val, extra\n"));
    }
}
