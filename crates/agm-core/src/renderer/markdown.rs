//! Markdown renderer: produces a human-readable Markdown document.
//!
//! Nodes are grouped by type with type headers. Within each group,
//! nodes appear in their original order.

use crate::model::code::CodeBlock;
use crate::model::context::AgentContext;
use crate::model::fields::NodeType;
use crate::model::file::AgmFile;
use crate::model::memory::MemoryEntry;
use crate::model::node::Node;
use crate::model::orchestration::ParallelGroup;
use crate::model::verify::VerifyCheck;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Renders the `AgmFile` as a human-readable Markdown document.
///
/// Nodes are grouped by type with type headers. Within each group,
/// nodes appear in their original order. Only sections with at least one
/// node are emitted.
#[must_use]
pub fn render_markdown(file: &AgmFile) -> String {
    let mut buf = String::new();

    // Document title
    buf.push_str(&format!(
        "# {} v{}\n",
        file.header.package, file.header.version
    ));

    if let Some(ref title) = file.header.title {
        buf.push('\n');
        buf.push_str(&format!("> {title}\n"));
    }

    if let Some(ref desc) = file.header.description {
        buf.push('\n');
        buf.push_str(desc);
        buf.push('\n');
    }

    // Properties table
    let has_props = file.header.owner.is_some()
        || file.header.status.is_some()
        || file.header.default_load.is_some()
        || file.header.tags.is_some()
        || file.header.target_runtime.is_some();

    if has_props {
        buf.push('\n');
        buf.push_str("| Property | Value |\n");
        buf.push_str("|----------|-------|\n");
        if let Some(ref owner) = file.header.owner {
            buf.push_str(&format!("| Owner | {owner} |\n"));
        }
        if let Some(ref status) = file.header.status {
            buf.push_str(&format!("| Status | {status} |\n"));
        }
        if let Some(ref dl) = file.header.default_load {
            buf.push_str(&format!("| Default load | {dl} |\n"));
        }
        if let Some(ref tags) = file.header.tags {
            buf.push_str(&format!("| Tags | {} |\n", tags.join(", ")));
        }
        if let Some(ref rt) = file.header.target_runtime {
            buf.push_str(&format!("| Target runtime | {rt} |\n"));
        }
    }

    // Imports section
    if let Some(ref imports) = file.header.imports {
        if !imports.is_empty() {
            buf.push('\n');
            buf.push_str("## Imports\n\n");
            for import in imports {
                buf.push_str(&format!("- `{import}`\n"));
            }
        }
    }

    // Node sections grouped by type
    let type_order = type_order();
    let mut remaining: Vec<&Node> = file.nodes.iter().collect();

    for node_type in &type_order {
        let group: Vec<&Node> = remaining
            .iter()
            .copied()
            .filter(|n| std::mem::discriminant(&n.node_type) == std::mem::discriminant(node_type))
            .collect();

        if group.is_empty() {
            continue;
        }

        // Remove these nodes from remaining so custom types can be handled
        remaining
            .retain(|n| std::mem::discriminant(&n.node_type) != std::mem::discriminant(node_type));

        buf.push('\n');
        buf.push_str(&format!("## {}\n", type_display_name(node_type)));

        for node in group {
            buf.push('\n');
            render_node_section(&mut buf, node);
        }
    }

    // Custom types — group by type name, sorted
    let mut custom_types: Vec<String> = remaining
        .iter()
        .map(|n| n.node_type.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    custom_types.sort();

    for type_name in custom_types {
        let group: Vec<&Node> = remaining
            .iter()
            .copied()
            .filter(|n| n.node_type.to_string() == type_name)
            .collect();

        if group.is_empty() {
            continue;
        }

        buf.push('\n');
        buf.push_str(&format!("## {}\n", capitalize_first(&type_name)));

        for node in group {
            buf.push('\n');
            render_node_section(&mut buf, node);
        }
    }

    buf
}

// ---------------------------------------------------------------------------
// Node section rendering
// ---------------------------------------------------------------------------

fn render_node_section(buf: &mut String, node: &Node) {
    buf.push_str(&format!("### `{}`\n\n", node.id));

    // Ticket-specific header
    if node.node_type == NodeType::Ticket {
        if let Some(ref title) = node.title {
            buf.push_str(&format!("**Ticket:** {title}\n\n"));
        }
        let has_ticket_meta =
            node.priority.is_some() || node.action.is_some() || node.sdd_phase.is_some();
        if has_ticket_meta {
            buf.push_str("| Field | Value |\n");
            buf.push_str("|-------|-------|\n");
            if let Some(ref p) = node.priority {
                buf.push_str(&format!("| Priority | {p} |\n"));
            }
            if let Some(ref a) = node.action {
                buf.push_str(&format!("| Action | {a} |\n"));
            }
            if let Some(ref ph) = node.sdd_phase {
                buf.push_str(&format!("| Phase | {ph} |\n"));
            }
            buf.push('\n');
        }
        if let Some(ref desc) = node.description {
            buf.push_str(desc);
            buf.push('\n');
        }
    }

    buf.push_str(&node.summary);
    buf.push('\n');

    // Control table (for ticket: skip priority since it's in the ticket meta table above)
    let has_control = if node.node_type == NodeType::Ticket {
        node.stability.is_some() || node.confidence.is_some() || node.status.is_some()
    } else {
        node.priority.is_some()
            || node.stability.is_some()
            || node.confidence.is_some()
            || node.status.is_some()
    };

    if has_control {
        buf.push('\n');
        buf.push_str("| Control | Value |\n");
        buf.push_str("|---------|-------|\n");
        // For tickets, priority is shown in the ticket meta table above — skip here
        if node.node_type != NodeType::Ticket {
            if let Some(ref p) = node.priority {
                buf.push_str(&format!("| Priority | {p} |\n"));
            }
        }
        if let Some(ref s) = node.stability {
            buf.push_str(&format!("| Stability | {s} |\n"));
        }
        if let Some(ref c) = node.confidence {
            buf.push_str(&format!("| Confidence | {c} |\n"));
        }
        if let Some(ref s) = node.status {
            buf.push_str(&format!("| Status | {s} |\n"));
        }
    }

    // Ticket assignee and labels (after control table)
    if node.node_type == NodeType::Ticket {
        if let Some(ref assignee) = node.assignee {
            buf.push('\n');
            buf.push_str(&format!("**Assignee:** {assignee}\n"));
        }
        if let Some(ref labels) = node.labels {
            if !labels.is_empty() {
                buf.push('\n');
                buf.push_str(&format!("**Labels:** {}\n", labels.join(", ")));
            }
        }
        if let Some(ref ticket_id) = node.ticket_id {
            buf.push('\n');
            buf.push_str(&format!("**Ticket ID:** {ticket_id}\n"));
        }
        if let Some(ref prompt) = node.prompt {
            buf.push('\n');
            buf.push_str("#### Prompt\n\n");
            buf.push_str(prompt);
            buf.push('\n');
        }
    }

    // Tags
    if let Some(ref tags) = node.tags {
        if !tags.is_empty() {
            buf.push('\n');
            buf.push_str(&format!("**Tags**: {}\n", tags.join(", ")));
        }
    }

    // Relationships
    render_opt_rel(buf, "Depends on", &node.depends);
    render_opt_rel(buf, "Related to", &node.related_to);
    render_opt_rel(buf, "Replaces", &node.replaces);
    render_opt_rel(buf, "Conflicts with", &node.conflicts);
    render_opt_rel(buf, "See also", &node.see_also);

    // I/O
    if let Some(ref input) = node.input {
        if !input.is_empty() {
            buf.push('\n');
            buf.push_str(&format!("**Input**: {}\n", format_id_list(input)));
        }
    }
    if let Some(ref output) = node.output {
        if !output.is_empty() {
            buf.push('\n');
            buf.push_str(&format!("**Output**: {}\n", format_id_list(output)));
        }
    }

    // Detail
    if let Some(ref detail) = node.detail {
        buf.push('\n');
        buf.push_str("#### Detail\n\n");
        buf.push_str(detail);
        buf.push('\n');
    }

    // Items
    if let Some(ref items) = node.items {
        if !items.is_empty() {
            buf.push('\n');
            buf.push_str("#### Items\n\n");
            for item in items {
                buf.push_str(&format!("- {item}\n"));
            }
        }
    }

    // Steps
    if let Some(ref steps) = node.steps {
        if !steps.is_empty() {
            buf.push('\n');
            buf.push_str("#### Steps\n\n");
            for (i, step) in steps.iter().enumerate() {
                buf.push_str(&format!("{}. {step}\n", i + 1));
            }
        }
    }

    // Fields
    if let Some(ref fields) = node.fields {
        if !fields.is_empty() {
            buf.push('\n');
            buf.push_str("#### Fields\n\n");
            for field in fields {
                buf.push_str(&format!("- {field}\n"));
            }
        }
    }

    // Rationale
    if let Some(ref rationale) = node.rationale {
        if !rationale.is_empty() {
            buf.push('\n');
            buf.push_str("#### Rationale\n\n");
            for r in rationale {
                buf.push_str(&format!("- {r}\n"));
            }
        }
    }

    // Tradeoffs
    if let Some(ref tradeoffs) = node.tradeoffs {
        if !tradeoffs.is_empty() {
            buf.push('\n');
            buf.push_str("#### Tradeoffs\n\n");
            for t in tradeoffs {
                buf.push_str(&format!("- {t}\n"));
            }
        }
    }

    // Resolution
    if let Some(ref resolution) = node.resolution {
        if !resolution.is_empty() {
            buf.push('\n');
            buf.push_str("#### Resolution\n\n");
            for r in resolution {
                buf.push_str(&format!("- {r}\n"));
            }
        }
    }

    // Examples
    if let Some(ref examples) = node.examples {
        buf.push('\n');
        buf.push_str("#### Examples\n\n");
        buf.push_str(examples);
        buf.push('\n');
    }

    // Notes
    if let Some(ref notes) = node.notes {
        buf.push('\n');
        buf.push_str("#### Notes\n\n");
        buf.push_str(notes);
        buf.push('\n');
    }

    // Code
    if let Some(ref cb) = node.code {
        buf.push('\n');
        buf.push_str("#### Code\n\n");
        render_code_block_md(buf, cb);
    }

    // Code blocks
    if let Some(ref blocks) = node.code_blocks {
        if !blocks.is_empty() {
            buf.push('\n');
            buf.push_str("#### Code Blocks\n\n");
            for cb in blocks {
                render_code_block_md(buf, cb);
            }
        }
    }

    // Verify
    if let Some(ref checks) = node.verify {
        if !checks.is_empty() {
            buf.push('\n');
            buf.push_str("#### Verification\n\n");
            for check in checks {
                render_verify_check_md(buf, check);
            }
        }
    }

    // Agent context
    if let Some(ref ctx) = node.agent_context {
        buf.push('\n');
        render_agent_context_md(buf, ctx);
    }

    // Memory
    if let Some(ref entries) = node.memory {
        if !entries.is_empty() {
            buf.push('\n');
            buf.push_str("#### Memory\n\n");
            render_memory_table_md(buf, entries);
        }
    }

    // Parallel groups
    if let Some(ref groups) = node.parallel_groups {
        if !groups.is_empty() {
            buf.push('\n');
            buf.push_str("#### Parallel Groups\n\n");
            render_parallel_groups_md(buf, groups);
        }
    }

    // Execution state
    let has_exec =
        node.execution_status.is_some() || node.executed_by.is_some() || node.executed_at.is_some();

    if has_exec {
        buf.push('\n');
        buf.push_str("#### Execution State\n\n");
        if let Some(ref s) = node.execution_status {
            buf.push_str(&format!("- **Status**: {s}\n"));
        }
        if let Some(ref by) = node.executed_by {
            buf.push_str(&format!("- **Executed by**: {by}\n"));
        }
        if let Some(ref at) = node.executed_at {
            buf.push_str(&format!("- **Executed at**: {at}\n"));
        }
        if let Some(n) = node.retry_count {
            buf.push_str(&format!("- **Retry count**: {n}\n"));
        }
    }

    buf.push_str("\n---\n");
}

// ---------------------------------------------------------------------------
// Sub-renderers
// ---------------------------------------------------------------------------

fn render_opt_rel(buf: &mut String, label: &str, val: &Option<Vec<String>>) {
    if let Some(ids) = val {
        if !ids.is_empty() {
            buf.push('\n');
            buf.push_str(&format!("**{label}**: {}\n", format_id_list(ids)));
        }
    }
}

fn format_id_list(ids: &[String]) -> String {
    ids.iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_code_block_md(buf: &mut String, cb: &CodeBlock) {
    let lang = cb.lang.as_deref().unwrap_or("");
    if let Some(ref target) = cb.target {
        buf.push_str(&format!("**File**: `{target}`  \n"));
    }
    if let Some(ref anchor) = cb.anchor {
        buf.push_str(&format!("**Anchor**: `{anchor}`  \n"));
    }
    buf.push_str(&format!("**Action**: {}  \n\n", cb.action));
    buf.push_str(&format!("```{lang}\n"));
    buf.push_str(&cb.body);
    if !cb.body.ends_with('\n') {
        buf.push('\n');
    }
    buf.push_str("```\n");
}

fn render_verify_check_md(buf: &mut String, check: &VerifyCheck) {
    match check {
        VerifyCheck::Command { run, expect } => {
            let exp_str = expect
                .as_deref()
                .map(|e| format!(" (expect: {e})"))
                .unwrap_or_default();
            buf.push_str(&format!("- `{run}`{exp_str}\n"));
        }
        VerifyCheck::FileExists { file } => {
            buf.push_str(&format!("- File exists: `{file}`\n"));
        }
        VerifyCheck::FileContains { file, pattern } => {
            buf.push_str(&format!("- `{file}` contains `{pattern}`\n"));
        }
        VerifyCheck::FileNotContains { file, pattern } => {
            buf.push_str(&format!("- `{file}` does NOT contain `{pattern}`\n"));
        }
        VerifyCheck::NodeStatus { node, status } => {
            buf.push_str(&format!("- Node `{node}` has status `{status}`\n"));
        }
    }
}

fn render_agent_context_md(buf: &mut String, ctx: &AgentContext) {
    buf.push_str("#### Agent Context\n\n");
    if let Some(ref nodes) = ctx.load_nodes {
        buf.push_str(&format!("**Load nodes**: {}\n\n", format_id_list(nodes)));
    }
    if let Some(ref files) = ctx.load_files {
        buf.push_str("**Load files**:\n");
        for lf in files {
            buf.push_str(&format!(
                "- `{}` (range: {})\n",
                lf.path,
                format_file_range(&lf.range)
            ));
        }
        buf.push('\n');
    }
    if let Some(ref hint) = ctx.system_hint {
        buf.push_str(&format!("**System hint**: {hint}\n\n"));
    }
}

fn format_file_range(range: &crate::model::context::FileRange) -> String {
    match range {
        crate::model::context::FileRange::Full => "full".into(),
        crate::model::context::FileRange::Lines(s, e) => format!("{s}-{e}"),
        crate::model::context::FileRange::Function(name) => format!("function: {name}"),
    }
}

fn render_memory_table_md(buf: &mut String, entries: &[MemoryEntry]) {
    buf.push_str("| Key | Topic | Action | Value |\n");
    buf.push_str("|-----|-------|--------|-------|\n");
    for entry in entries {
        let value = entry.value.as_deref().unwrap_or("-");
        buf.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            entry.key, entry.topic, entry.action, value
        ));
    }
}

fn render_parallel_groups_md(buf: &mut String, groups: &[ParallelGroup]) {
    buf.push_str("| Group | Nodes | Strategy |\n");
    buf.push_str("|-------|-------|----------|\n");
    for group in groups {
        buf.push_str(&format!(
            "| {} | {} | {} |\n",
            group.group,
            group.nodes.join(", "),
            group.strategy
        ));
    }
}

// ---------------------------------------------------------------------------
// Type ordering and display names
// ---------------------------------------------------------------------------

fn type_order() -> Vec<NodeType> {
    vec![
        NodeType::Facts,
        NodeType::Rules,
        NodeType::Workflow,
        NodeType::Entity,
        NodeType::Decision,
        NodeType::Exception,
        NodeType::Example,
        NodeType::Glossary,
        NodeType::AntiPattern,
        NodeType::Orchestration,
        NodeType::Ticket,
    ]
}

fn type_display_name(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Facts => "Facts",
        NodeType::Rules => "Rules",
        NodeType::Workflow => "Workflow",
        NodeType::Entity => "Entity",
        NodeType::Decision => "Decision",
        NodeType::Exception => "Exception",
        NodeType::Example => "Example",
        NodeType::Glossary => "Glossary",
        NodeType::AntiPattern => "Anti-Pattern",
        NodeType::Orchestration => "Orchestration",
        NodeType::Ticket => "Tickets",
        NodeType::Custom(_) => "Custom",
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::{NodeType, Priority, Stability};
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
                ..Default::default()
            }],
        }
    }

    #[test]
    fn test_render_markdown_groups_by_type() {
        let mut file = minimal_file();
        // Add a workflow node
        let mut wf = file.nodes[0].clone();
        wf.id = "test.workflow".into();
        wf.node_type = NodeType::Workflow;
        wf.summary = "a workflow node".into();
        file.nodes.push(wf);

        let output = render_markdown(&file);
        let facts_pos = output.find("## Facts").unwrap();
        let workflow_pos = output.find("## Workflow").unwrap();
        assert!(
            facts_pos < workflow_pos,
            "Facts section before Workflow section"
        );
    }

    #[test]
    fn test_render_markdown_title_in_header() {
        let file = minimal_file();
        let output = render_markdown(&file);
        assert!(output.starts_with("# test.minimal v0.1.0\n"));
    }

    #[test]
    fn test_render_markdown_node_has_summary() {
        let file = minimal_file();
        let output = render_markdown(&file);
        assert!(output.contains("a minimal test node"));
    }

    #[test]
    fn test_render_markdown_control_table_when_priority() {
        let mut file = minimal_file();
        file.nodes[0].priority = Some(Priority::Critical);
        file.nodes[0].stability = Some(Stability::High);
        let output = render_markdown(&file);
        assert!(output.contains("| Control | Value |"));
        assert!(output.contains("| Priority | critical |"));
        assert!(output.contains("| Stability | high |"));
    }

    #[test]
    fn test_render_markdown_depends_section() {
        let mut file = minimal_file();
        file.nodes[0].depends = Some(vec!["auth.constraints".into()]);
        let output = render_markdown(&file);
        assert!(output.contains("**Depends on**: `auth.constraints`"));
    }

    #[test]
    fn test_render_markdown_no_empty_sections() {
        let file = minimal_file();
        let output = render_markdown(&file);
        // Should not have empty type sections for types with no nodes
        assert!(!output.contains("## Rules\n\n---"));
        assert!(!output.contains("## Workflow\n\n---"));
    }

    #[test]
    fn test_render_markdown_imports_section() {
        let mut file = minimal_file();
        file.header.imports = Some(vec![crate::model::imports::ImportEntry::new(
            "shared.security".into(),
            Some("^1.0.0".into()),
        )]);
        let output = render_markdown(&file);
        assert!(output.contains("## Imports\n"));
        assert!(output.contains("- `shared.security@^1.0.0`"));
    }
}
