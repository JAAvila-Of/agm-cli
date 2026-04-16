//! Context builder for agent prompts (spec S25).
//!
//! Assembles the prompt string sent to an agent when executing a node.
//! Priority order (highest to lowest):
//!   1. system_hint
//!   2. target node (full)
//!   3. load_nodes entries (operational)
//!   4. transitive depends (summary)
//!   5. load_files content
//!   6. load_memory entries
#![allow(dead_code)]

use std::collections::HashSet;
use std::path::Path;

use agm_core::graph::AgmGraph;
use agm_core::loader::{LoadMode, filter_node};
use agm_core::model::context::{AgentContext, FileRange};
use agm_core::model::file::AgmFile;
use agm_core::model::node::Node;
use agm_core::renderer::markdown::render_markdown;

use super::memory::MemoryRuntime;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Identifies the origin of a context section.
#[derive(Debug, Clone)]
pub enum ContextSource {
    Node { id: String, mode: LoadMode },
    File { path: String },
    Memory { topic: String },
    SystemHint,
}

/// A named section within a built context.
#[derive(Debug, Clone)]
pub struct ContextSection {
    pub name: String,
    pub content: String,
    pub source: ContextSource,
}

/// A fully built context ready to send to an agent.
#[derive(Debug, Clone)]
pub struct BuiltContext {
    /// The assembled prompt string.
    pub prompt: String,
    /// Estimated token count (rough: chars / 4).
    pub token_estimate: usize,
    /// Named sections included in the prompt (for debugging/logging).
    pub sections: Vec<ContextSection>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Builds a context string for the given node.
///
/// If the node has an `agent_context` field, uses the explicit specification.
/// Otherwise falls back to: target node at full level + direct `depends` at
/// operational level.
#[must_use]
pub fn build_context(
    node: &Node,
    file: &AgmFile,
    graph: &AgmGraph,
    memory: &MemoryRuntime,
    working_dir: &Path,
) -> BuiltContext {
    match &node.agent_context {
        Some(agent_ctx) => {
            build_explicit_context(node, file, graph, memory, working_dir, agent_ctx)
        }
        None => build_default_context(node, file),
    }
}

// ---------------------------------------------------------------------------
// Default context (no agent_context field)
// ---------------------------------------------------------------------------

fn build_default_context(node: &Node, file: &AgmFile) -> BuiltContext {
    let mut sections = Vec::new();

    // Target node at full level
    let full_node = filter_node(node, LoadMode::Full);
    let target_file = single_node_file(file, &full_node);
    let rendered = strip_markdown_header(render_markdown(&target_file));
    sections.push(ContextSection {
        name: format!("node:{}", node.id),
        content: rendered,
        source: ContextSource::Node {
            id: node.id.clone(),
            mode: LoadMode::Full,
        },
    });

    // Direct depends at operational level
    if let Some(ref deps) = node.depends {
        for dep_id in deps {
            if let Some(dep_node) = file.nodes.iter().find(|n| &n.id == dep_id) {
                let op_node = filter_node(dep_node, LoadMode::Operational);
                let dep_file = single_node_file(file, &op_node);
                let rendered = strip_markdown_header(render_markdown(&dep_file));
                sections.push(ContextSection {
                    name: format!("dep:{}", dep_id),
                    content: rendered,
                    source: ContextSource::Node {
                        id: dep_id.clone(),
                        mode: LoadMode::Operational,
                    },
                });
            }
        }
    }

    let prompt = assemble_prompt(&sections);
    let token_estimate = estimate_tokens(&prompt);
    BuiltContext {
        prompt,
        token_estimate,
        sections,
    }
}

// ---------------------------------------------------------------------------
// Explicit context (node has agent_context field)
// ---------------------------------------------------------------------------

fn build_explicit_context(
    node: &Node,
    file: &AgmFile,
    graph: &AgmGraph,
    memory: &MemoryRuntime,
    working_dir: &Path,
    agent_ctx: &AgentContext,
) -> BuiltContext {
    let max_tokens = agent_ctx.max_tokens.map(|t| t as usize);
    let mut sections = Vec::new();

    // 1. system_hint (highest priority)
    if let Some(ref hint) = agent_ctx.system_hint {
        sections.push(ContextSection {
            name: "system_hint".to_owned(),
            content: hint.clone(),
            source: ContextSource::SystemHint,
        });
    }

    // 2. Target node at full level
    let full_node = filter_node(node, LoadMode::Full);
    let target_file = single_node_file(file, &full_node);
    let rendered = strip_markdown_header(render_markdown(&target_file));
    sections.push(ContextSection {
        name: format!("node:{}", node.id),
        content: rendered,
        source: ContextSource::Node {
            id: node.id.clone(),
            mode: LoadMode::Full,
        },
    });

    // 3. load_nodes at operational level
    if let Some(ref load_nodes) = agent_ctx.load_nodes {
        for node_id in load_nodes {
            if let Some(dep_node) = file.nodes.iter().find(|n| &n.id == node_id) {
                let op_node = filter_node(dep_node, LoadMode::Operational);
                let dep_file = single_node_file(file, &op_node);
                let rendered = strip_markdown_header(render_markdown(&dep_file));
                sections.push(ContextSection {
                    name: format!("load_node:{}", node_id),
                    content: rendered,
                    source: ContextSource::Node {
                        id: node_id.clone(),
                        mode: LoadMode::Operational,
                    },
                });
            }
        }
    }

    // 4. Transitive depends at summary level (exclude already-included nodes)
    let already_included: HashSet<&str> = {
        let mut s = HashSet::new();
        s.insert(node.id.as_str());
        if let Some(ref ln) = agent_ctx.load_nodes {
            for id in ln {
                s.insert(id.as_str());
            }
        }
        s
    };

    let trans_deps = agm_core::graph::transitive_deps(graph, &node.id);
    let mut sorted_deps: Vec<&str> = trans_deps
        .iter()
        .map(|s| s.as_str())
        .filter(|id| !already_included.contains(id))
        .collect();
    sorted_deps.sort();

    for dep_id in sorted_deps {
        if let Some(dep_node) = file.nodes.iter().find(|n| n.id == dep_id) {
            let sum_node = filter_node(dep_node, LoadMode::Summary);
            let dep_file = single_node_file(file, &sum_node);
            let rendered = strip_markdown_header(render_markdown(&dep_file));
            sections.push(ContextSection {
                name: format!("dep:{}", dep_id),
                content: rendered,
                source: ContextSource::Node {
                    id: dep_id.to_owned(),
                    mode: LoadMode::Summary,
                },
            });
        }
    }

    // 5. load_files
    if let Some(ref load_files) = agent_ctx.load_files {
        for lf in load_files {
            let full_path = working_dir.join(&lf.path);
            let content = load_file_content(&full_path, &lf.range);
            sections.push(ContextSection {
                name: format!("file:{}", lf.path),
                content,
                source: ContextSource::File {
                    path: lf.path.clone(),
                },
            });
        }
    }

    // 6. load_memory (lowest priority)
    if let Some(ref topics) = agent_ctx.load_memory {
        let mem_sections = load_memory_topics(topics, memory);
        sections.extend(mem_sections);
    }

    // Truncate to budget if max_tokens set
    let sections = if let Some(budget) = max_tokens {
        truncate_to_budget(sections, budget)
    } else {
        sections
    };

    let prompt = assemble_prompt(&sections);
    let token_estimate = estimate_tokens(&prompt);
    BuiltContext {
        prompt,
        token_estimate,
        sections,
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Builds a single-node `AgmFile` for rendering purposes.
fn single_node_file(file: &AgmFile, node: &Node) -> AgmFile {
    AgmFile {
        header: file.header.clone(),
        nodes: vec![node.clone()],
    }
}

/// Strips the top-level markdown header line (e.g. `# Package vX.Y`) from
/// the rendered output so sections can be composed without repeated headers.
fn strip_markdown_header(s: String) -> String {
    let mut lines = s.lines();
    match lines.next() {
        Some(first) if first.starts_with("# ") => {
            // Skip the header line and one optional blank line
            let rest: String = lines
                .skip_while(|l| l.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            rest.trim_start().to_owned()
        }
        _ => s,
    }
}

// ---------------------------------------------------------------------------
// File content loading
// ---------------------------------------------------------------------------

fn load_file_content(path: &Path, range: &FileRange) -> String {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return format!("<!-- could not read {}: {} -->", path.display(), e),
    };

    match range {
        FileRange::Full => content,
        FileRange::Lines(start, end) => extract_lines(&content, *start, *end),
        FileRange::Function(name) => extract_function(&content, name, path),
    }
}

fn extract_lines(content: &str, start: u64, end: u64) -> String {
    content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let lineno = (i + 1) as u64;
            if lineno >= start && lineno <= end {
                Some(line)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_function(content: &str, name: &str, path: &Path) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "rs" => extract_rust_function(content, name),
        "py" => extract_python_function(content, name),
        "js" | "ts" | "jsx" | "tsx" => extract_js_function(content, name),
        _ => extract_brace_function(content, name),
    }
}

fn extract_rust_function(content: &str, name: &str) -> String {
    // Match `fn <name>(` with optional pub/async/unsafe/extern qualifiers
    let pattern = format!("fn {}(", name);
    extract_brace_block(content, &pattern)
        .unwrap_or_else(|| format!("<!-- function '{}' not found -->", name))
}

fn extract_python_function(content: &str, name: &str) -> String {
    let pattern = format!("def {}(", name);
    // Python uses indentation — collect until next def/class at same indent
    let start_idx = content.find(&pattern);
    let Some(start_byte) = start_idx else {
        return format!("<!-- function '{}' not found -->", name);
    };

    // Find the line number of start
    let before = &content[..start_byte];
    let start_line = before.lines().count();

    let lines: Vec<&str> = content.lines().collect();
    if start_line >= lines.len() {
        return format!("<!-- function '{}' not found -->", name);
    }

    // Determine base indent
    let base_indent = lines[start_line]
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();

    let mut result_lines = vec![lines[start_line]];
    for line in &lines[start_line + 1..] {
        if line.trim().is_empty() {
            result_lines.push(line);
            continue;
        }
        let indent = line.chars().take_while(|c| c.is_whitespace()).count();
        if indent <= base_indent && !line.trim().is_empty() {
            break;
        }
        result_lines.push(line);
    }

    result_lines.join("\n")
}

fn extract_js_function(content: &str, name: &str) -> String {
    // Handles: `function name(`, `const name =`, `name(`
    let patterns = [
        format!("function {}(", name),
        format!("const {} =", name),
        format!("{}(", name),
    ];
    for pat in &patterns {
        if let Some(block) = extract_brace_block(content, pat) {
            return block;
        }
    }
    format!("<!-- function '{}' not found -->", name)
}

/// Generic fallback: finds `pattern` then extracts the matching brace block.
fn extract_brace_function(content: &str, name: &str) -> String {
    let pattern = format!("{}(", name);
    extract_brace_block(content, &pattern)
        .unwrap_or_else(|| format!("<!-- function '{}' not found -->", name))
}

/// Finds `pattern` in `content` then collects characters until the opening
/// `{` and its matching closing `}`.  Returns the full function text.
fn extract_brace_block(content: &str, pattern: &str) -> Option<String> {
    let start_byte = content.find(pattern)?;

    // Find the line that contains start_byte
    let before = &content[..start_byte];
    let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let from = line_start;

    // Now scan for the opening brace
    let rest = &content[start_byte..];
    let brace_offset = rest.find('{')?;
    let abs_brace = start_byte + brace_offset;

    let mut depth: i32 = 0;
    let mut end_byte = abs_brace;
    for (i, ch) in content[abs_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_byte = abs_brace + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    Some(content[from..end_byte].to_owned())
}

// ---------------------------------------------------------------------------
// Memory loading
// ---------------------------------------------------------------------------

fn load_memory_topics(topics: &[String], memory: &MemoryRuntime) -> Vec<ContextSection> {
    let mut sections = Vec::new();
    for topic in topics {
        let entries = memory.list_topic(topic);
        if entries.is_empty() {
            continue;
        }
        let content = entries
            .iter()
            .map(|(key, value)| format!("**{}**: {}", key, value))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(ContextSection {
            name: format!("memory:{}", topic),
            content,
            source: ContextSource::Memory {
                topic: topic.clone(),
            },
        });
    }
    sections
}

// ---------------------------------------------------------------------------
// Prompt assembly
// ---------------------------------------------------------------------------

fn assemble_prompt(sections: &[ContextSection]) -> String {
    let mut parts = Vec::new();
    for section in sections {
        match &section.source {
            ContextSource::SystemHint => {
                parts.push(section.content.clone());
            }
            ContextSource::Node { id, .. } => {
                parts.push(format!("## Node: {}\n\n{}", id, section.content));
            }
            ContextSource::File { path } => {
                parts.push(format!(
                    "## File: {}\n\n```\n{}\n```",
                    path, section.content
                ));
            }
            ContextSource::Memory { topic } => {
                parts.push(format!("## Memory: {}\n\n{}", topic, section.content));
            }
        }
    }
    parts.join("\n\n")
}

fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

// ---------------------------------------------------------------------------
// Token budget enforcement
// ---------------------------------------------------------------------------

/// Truncates sections from lowest priority (end of list) first until the
/// estimated total fits within `budget` tokens.
fn truncate_to_budget(mut sections: Vec<ContextSection>, budget: usize) -> Vec<ContextSection> {
    while sections.len() > 1 && estimate_total_tokens(&sections) > budget {
        sections.pop();
    }
    sections
}

fn estimate_total_tokens(sections: &[ContextSection]) -> usize {
    let total_chars: usize = sections.iter().map(|s| s.content.len()).sum();
    total_chars / 4
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use agm_core::graph::build_graph;
    use agm_core::model::context::AgentContext;
    use agm_core::model::fields::{NodeType, Span};
    use agm_core::model::file::{AgmFile, Header};
    use agm_core::model::node::Node;

    use crate::runtime::memory::MemoryRuntime;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_header() -> Header {
        Header {
            agm: "0.2".to_owned(),
            package: "test.pkg".to_owned(),
            version: "1.0".to_owned(),
            title: None,
            owner: None,
            imports: None,
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        }
    }

    fn make_node(id: &str, summary: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: summary.to_owned(),
            span: Span::new(1, 5),
            ..Default::default()
        }
    }

    fn make_file(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: make_header(),
            nodes,
        }
    }

    fn make_memory() -> MemoryRuntime {
        let tmp = tempfile::tempdir().unwrap();
        let project_path = tmp.path().join("test.agm.mem");
        let global_path = tmp.path().join("global.mem");
        MemoryRuntime::new(project_path, global_path, "test.pkg").unwrap()
    }

    // -----------------------------------------------------------------------
    // 1. Default context: no agent_context → target node + direct deps
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_context_default_no_deps() {
        let node = make_node("a.node", "A summary");
        let file = make_file(vec![node.clone()]);
        let graph = build_graph(&file);
        let memory = make_memory();

        let ctx = build_context(&node, &file, &graph, &memory, Path::new("."));

        assert!(!ctx.prompt.is_empty());
        assert!(ctx.prompt.contains("a.node"));
        assert!(ctx.prompt.contains("A summary"));
        assert_eq!(ctx.sections.len(), 1);
    }

    #[test]
    fn test_build_context_default_with_deps() {
        let dep = make_node("dep.node", "Dep summary");
        let mut target = make_node("a.node", "A summary");
        target.depends = Some(vec!["dep.node".to_owned()]);

        let file = make_file(vec![dep.clone(), target.clone()]);
        let graph = build_graph(&file);
        let memory = make_memory();

        let ctx = build_context(&target, &file, &graph, &memory, Path::new("."));

        assert!(ctx.prompt.contains("a.node"));
        assert!(ctx.prompt.contains("dep.node"));
        // Two sections: target + dep
        assert_eq!(ctx.sections.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 2. Token estimate
    // -----------------------------------------------------------------------

    #[test]
    fn test_token_estimate_is_chars_over_4() {
        let text = "a".repeat(400);
        assert_eq!(estimate_tokens(&text), 100);
    }

    // -----------------------------------------------------------------------
    // 3. strip_markdown_header
    // -----------------------------------------------------------------------

    #[test]
    fn test_strip_markdown_header_removes_h1() {
        let input = "# My Package v1.0\n\n## Node: foo\n\nBody text".to_owned();
        let stripped = strip_markdown_header(input);
        assert!(!stripped.starts_with("# My Package"));
        assert!(stripped.contains("## Node: foo"));
    }

    #[test]
    fn test_strip_markdown_header_passthrough_when_no_h1() {
        let input = "## Node: foo\n\nBody".to_owned();
        let stripped = strip_markdown_header(input);
        assert!(stripped.starts_with("## Node:"));
    }

    // -----------------------------------------------------------------------
    // 4. extract_lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_lines_basic() {
        let content = "line1\nline2\nline3\nline4";
        let result = extract_lines(content, 2, 3);
        assert_eq!(result, "line2\nline3");
    }

    #[test]
    fn test_extract_lines_out_of_range() {
        let content = "line1\nline2";
        let result = extract_lines(content, 5, 10);
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // 5. extract_rust_function
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_rust_function_found() {
        let content = "fn foo(x: i32) -> i32 {\n    x + 1\n}\n\nfn bar() {}";
        let result = extract_rust_function(content, "foo");
        assert!(result.contains("fn foo"));
        assert!(result.contains("x + 1"));
        assert!(!result.contains("fn bar"));
    }

    #[test]
    fn test_extract_rust_function_not_found() {
        let content = "fn bar() {}";
        let result = extract_rust_function(content, "missing");
        assert!(result.contains("not found"));
    }

    // -----------------------------------------------------------------------
    // 6. extract_python_function
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_python_function_found() {
        let content = "def foo(x):\n    return x + 1\n\ndef bar():\n    pass\n";
        let result = extract_python_function(content, "foo");
        assert!(result.contains("def foo"));
        assert!(result.contains("return x + 1"));
        assert!(!result.contains("def bar"));
    }

    // -----------------------------------------------------------------------
    // 7. load_file_content
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_file_content_full() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "hello\nworld").unwrap();

        let result = load_file_content(&path, &FileRange::Full);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_load_file_content_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "a\nb\nc\nd").unwrap();

        let result = load_file_content(&path, &FileRange::Lines(2, 3));
        assert_eq!(result, "b\nc");
    }

    #[test]
    fn test_load_file_content_missing_file() {
        let result = load_file_content(Path::new("/nonexistent/file.txt"), &FileRange::Full);
        assert!(result.contains("could not read"));
    }

    // -----------------------------------------------------------------------
    // 8. load_memory_topics
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_memory_topics_empty() {
        let memory = make_memory();
        let sections = load_memory_topics(&["nonexistent.topic".to_owned()], &memory);
        assert!(sections.is_empty());
    }

    // -----------------------------------------------------------------------
    // 9. truncate_to_budget
    // -----------------------------------------------------------------------

    #[test]
    fn test_truncate_to_budget_removes_low_priority() {
        let sections = vec![
            ContextSection {
                name: "s1".to_owned(),
                content: "a".repeat(100),
                source: ContextSource::SystemHint,
            },
            ContextSection {
                name: "s2".to_owned(),
                content: "b".repeat(100),
                source: ContextSource::SystemHint,
            },
            ContextSection {
                name: "s3".to_owned(),
                content: "c".repeat(400),
                source: ContextSource::SystemHint,
            },
        ];
        // Budget of 50 tokens = 200 chars — s3 alone is 400 chars (100 tokens)
        // After removing s3: (200 chars / 4 = 50 tokens) — fits in budget
        let result = truncate_to_budget(sections, 50);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "s1");
        assert_eq!(result[1].name, "s2");
    }

    #[test]
    fn test_truncate_to_budget_preserves_when_fits() {
        let sections = vec![ContextSection {
            name: "s1".to_owned(),
            content: "a".repeat(40),
            source: ContextSource::SystemHint,
        }];
        let result = truncate_to_budget(sections, 10000);
        assert_eq!(result.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 10. assemble_prompt sections format
    // -----------------------------------------------------------------------

    #[test]
    fn test_assemble_prompt_system_hint_not_wrapped() {
        let sections = vec![ContextSection {
            name: "system_hint".to_owned(),
            content: "You are a Rust expert.".to_owned(),
            source: ContextSource::SystemHint,
        }];
        let prompt = assemble_prompt(&sections);
        assert_eq!(prompt, "You are a Rust expert.");
    }

    #[test]
    fn test_assemble_prompt_file_wrapped_in_code_block() {
        let sections = vec![ContextSection {
            name: "file:src/main.rs".to_owned(),
            content: "fn main() {}".to_owned(),
            source: ContextSource::File {
                path: "src/main.rs".to_owned(),
            },
        }];
        let prompt = assemble_prompt(&sections);
        assert!(prompt.contains("## File: src/main.rs"));
        assert!(prompt.contains("```"));
    }

    // -----------------------------------------------------------------------
    // 11. Explicit context: system_hint prepended
    // -----------------------------------------------------------------------

    #[test]
    fn test_explicit_context_system_hint_is_first() {
        let mut node = make_node("a.node", "A summary");
        node.agent_context = Some(AgentContext {
            load_nodes: None,
            load_files: None,
            system_hint: Some("SYSTEM: be precise.".to_owned()),
            max_tokens: None,
            load_memory: None,
        });
        let file = make_file(vec![node.clone()]);
        let graph = build_graph(&file);
        let memory = make_memory();

        let ctx = build_context(&node, &file, &graph, &memory, Path::new("."));

        assert!(ctx.prompt.starts_with("SYSTEM: be precise."));
    }

    // -----------------------------------------------------------------------
    // 12. Explicit context: load_nodes at operational level
    // -----------------------------------------------------------------------

    #[test]
    fn test_explicit_context_load_nodes_included() {
        let dep = make_node("dep.node", "Dep summary");
        let mut target = make_node("a.node", "A summary");
        target.agent_context = Some(AgentContext {
            load_nodes: Some(vec!["dep.node".to_owned()]),
            load_files: None,
            system_hint: None,
            max_tokens: None,
            load_memory: None,
        });

        let file = make_file(vec![dep, target.clone()]);
        let graph = build_graph(&file);
        let memory = make_memory();

        let ctx = build_context(&target, &file, &graph, &memory, Path::new("."));

        assert!(ctx.prompt.contains("dep.node"));
    }

    // -----------------------------------------------------------------------
    // 13. Explicit context: max_tokens truncation
    // -----------------------------------------------------------------------

    #[test]
    fn test_explicit_context_max_tokens_truncates() {
        let mut target = make_node("a.node", "A summary");
        // Add a dep to give us extra sections to truncate
        let dep = make_node("dep.node", "x".repeat(2000).as_str());
        target.depends = Some(vec!["dep.node".to_owned()]);
        target.agent_context = Some(AgentContext {
            load_nodes: Some(vec!["dep.node".to_owned()]),
            load_files: None,
            system_hint: None,
            max_tokens: Some(10), // tiny budget
            load_memory: None,
        });

        let file = make_file(vec![dep, target.clone()]);
        let graph = build_graph(&file);
        let memory = make_memory();

        let ctx = build_context(&target, &file, &graph, &memory, Path::new("."));

        // With a tiny budget, sections should be truncated (at least the target
        // node section must remain — truncate_to_budget keeps at least 1)
        assert!(!ctx.sections.is_empty());
    }

    // -----------------------------------------------------------------------
    // 14. BuiltContext fields populated
    // -----------------------------------------------------------------------

    #[test]
    fn test_built_context_fields_populated() {
        let node = make_node("a.node", "A summary");
        let file = make_file(vec![node.clone()]);
        let graph = build_graph(&file);
        let memory = make_memory();

        let ctx = build_context(&node, &file, &graph, &memory, Path::new("."));

        assert!(!ctx.prompt.is_empty());
        assert!(ctx.token_estimate > 0);
        assert!(!ctx.sections.is_empty());
    }

    // -----------------------------------------------------------------------
    // 15. extract_brace_block: nested braces
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_brace_block_nested() {
        let content = "fn foo() {\n    if true {\n        bar();\n    }\n}\nfn baz() {}";
        let result = extract_brace_block(content, "fn foo(");
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("if true"));
        assert!(!s.contains("fn baz"));
    }

    // -----------------------------------------------------------------------
    // 16. single_node_file header preserved
    // -----------------------------------------------------------------------

    #[test]
    fn test_single_node_file_preserves_header() {
        let node = make_node("a.node", "A summary");
        let file = make_file(vec![node.clone()]);
        let result = single_node_file(&file, &node);
        assert_eq!(result.header.package, "test.pkg");
        assert_eq!(result.nodes.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 17. estimate_total_tokens
    // -----------------------------------------------------------------------

    #[test]
    fn test_estimate_total_tokens() {
        let sections = vec![
            ContextSection {
                name: "a".to_owned(),
                content: "a".repeat(200),
                source: ContextSource::SystemHint,
            },
            ContextSection {
                name: "b".to_owned(),
                content: "b".repeat(200),
                source: ContextSource::SystemHint,
            },
        ];
        assert_eq!(estimate_total_tokens(&sections), 100);
    }

    // -----------------------------------------------------------------------
    // 18. list_topic returns correct entries
    // -----------------------------------------------------------------------

    #[test]
    fn test_list_topic_empty_when_no_entries() {
        let memory = make_memory();
        let results = memory.list_topic("my.topic");
        assert!(results.is_empty());
    }
}
