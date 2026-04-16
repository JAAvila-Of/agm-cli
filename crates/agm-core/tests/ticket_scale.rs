//! Scale and complexity integration tests for `type: ticket` nodes.
//!
//! Covers:
//!  - Many tickets with all action/sdd_phase variants in one file
//!  - Heavy files mixing tickets with every other node type
//!  - Tickets with maximal allowed-field populations (long description,
//!    multi-line prompt, many labels, boundary-length title, dependencies)
//!  - Canonical renderer determinism and roundtrip on ticket-heavy files
//!  - Markdown renderer grouping with many tickets
//!  - Validator throughput on files with many invalid tickets

use agm_core::model::fields::{NodeType, Priority, SddPhase, Span, TicketAction};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::node::Node;
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::parser::parse;
use agm_core::renderer::canonical::render_canonical;
use agm_core::renderer::{RenderFormat, render};
use agm_core::validator::{ValidateOptions, validate};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn scale_header() -> Header {
    Header {
        agm: "1.2".to_owned(),
        package: "test.ticket.scale".to_owned(),
        version: "1.0.0".to_owned(),
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

/// Build a valid ticket with the given id, action, and sdd_phase. When the
/// action is not `Create`, a synthetic `ticket_id` is added so validation
/// passes.
fn make_ticket(id: &str, action: TicketAction, phase: SddPhase) -> Node {
    let needs_id = !matches!(action, TicketAction::Create);
    Node {
        id: id.to_owned(),
        node_type: NodeType::Ticket,
        summary: format!("summary for {id}"),
        title: Some(format!("Title for {id}")),
        description: Some(format!("Description for {id}.")),
        priority: Some(Priority::Normal),
        action: Some(action),
        sdd_phase: Some(phase),
        ticket_id: if needs_id {
            Some(format!("parent.of.{id}"))
        } else {
            None
        },
        ..Default::default()
    }
}

fn all_actions() -> [TicketAction; 6] {
    [
        TicketAction::Create,
        TicketAction::Edit,
        TicketAction::Close,
        TicketAction::Archive,
        TicketAction::Split,
        TicketAction::Link,
    ]
}

fn all_sdd_phases() -> [SddPhase; 9] {
    [
        SddPhase::Backlog,
        SddPhase::Explore,
        SddPhase::Propose,
        SddPhase::Spec,
        SddPhase::Design,
        SddPhase::Tasks,
        SddPhase::Apply,
        SddPhase::Verify,
        SddPhase::Archive,
    ]
}

/// Strip span information so reparsed ASTs compare equal to in-memory ones.
fn strip_spans(file: &AgmFile) -> AgmFile {
    AgmFile {
        header: file.header.clone(),
        nodes: file
            .nodes
            .iter()
            .map(|n| Node {
                span: Span::default(),
                ..n.clone()
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// 1. Many tickets — every action × every sdd_phase
// ---------------------------------------------------------------------------

/// Build a file with 54 tickets: one for every (action, sdd_phase) pair.
fn file_all_action_phase_combos() -> AgmFile {
    let mut nodes = Vec::with_capacity(54);
    for action in all_actions() {
        for phase in all_sdd_phases() {
            let id = format!(
                "ticket.{}.{}",
                format!("{action:?}").to_lowercase(),
                format!("{phase:?}").to_lowercase(),
            );
            nodes.push(make_ticket(&id, action.clone(), phase.clone()));
        }
    }
    AgmFile {
        header: scale_header(),
        nodes,
    }
}

#[test]
fn test_scale_all_action_phase_combos_validate_clean() {
    let file = file_all_action_phase_combos();
    assert_eq!(file.nodes.len(), 54, "expected 54 ticket nodes");

    let src = render_canonical(&file);
    let collection = validate(&file, &src, "ticket_scale.agm", &ValidateOptions::default());
    assert!(
        !collection.has_errors(),
        "unexpected errors: {:?}",
        collection
            .diagnostics()
            .iter()
            .filter(|d| d.severity == agm_core::error::Severity::Error)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_scale_all_action_phase_combos_canonical_roundtrip() {
    let file = file_all_action_phase_combos();
    let canonical1 = render_canonical(&file);

    let reparsed = parse(&canonical1)
        .unwrap_or_else(|errs| panic!("re-parse failed: {errs:?}\n\n{canonical1}"));
    let canonical2 = render_canonical(&reparsed);

    assert_eq!(
        canonical1, canonical2,
        "canonical output differs after roundtrip"
    );
    assert_eq!(
        strip_spans(&file),
        strip_spans(&reparsed),
        "AST differs after roundtrip"
    );
}

#[test]
fn test_scale_all_action_phase_combos_canonical_is_deterministic() {
    let file = file_all_action_phase_combos();
    let first = render_canonical(&file);
    for i in 1..10 {
        assert_eq!(
            first,
            render_canonical(&file),
            "canonical render not deterministic at iteration {i}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Heavy mixed file — tickets alongside every other node type
// ---------------------------------------------------------------------------

fn make_simple(id: &str, node_type: NodeType) -> Node {
    Node {
        id: id.to_owned(),
        node_type,
        summary: format!("summary for {id}"),
        ..Default::default()
    }
}

/// Build a node populated with the minimum fields required by its type schema.
fn make_valid_for_type(id: &str, node_type: NodeType) -> Node {
    let mut n = make_simple(id, node_type.clone());
    match node_type {
        NodeType::Rules => {
            n.items = Some(vec![format!("{id}.item_a"), format!("{id}.item_b")]);
        }
        NodeType::Entity => {
            n.fields = Some(vec!["id:uuid".to_owned(), "name:string".to_owned()]);
        }
        NodeType::Decision => {
            n.rationale = Some(vec![format!("rationale for {id}")]);
        }
        NodeType::Orchestration => {
            // Callers must ensure the group's member ids exist in the file;
            // `file_mixed_tickets_and_types` populates workflow nodes with
            // the matching ids.
            n.parallel_groups = Some(vec![ParallelGroup {
                group: "g1".to_owned(),
                nodes: vec!["impl.workflow_a".to_owned(), "impl.workflow_b".to_owned()],
                strategy: Strategy::Parallel,
                requires: None,
                max_concurrency: None,
            }]);
        }
        _ => {}
    }
    n
}

/// Build a 25-node file mixing tickets with every other node type.
fn file_mixed_tickets_and_types() -> AgmFile {
    let mut nodes = Vec::with_capacity(25);

    // 8 tickets across varied actions and phases
    let specs = [
        (TicketAction::Create, SddPhase::Backlog),
        (TicketAction::Create, SddPhase::Explore),
        (TicketAction::Edit, SddPhase::Propose),
        (TicketAction::Close, SddPhase::Verify),
        (TicketAction::Archive, SddPhase::Archive),
        (TicketAction::Split, SddPhase::Tasks),
        (TicketAction::Link, SddPhase::Design),
        (TicketAction::Create, SddPhase::Apply),
    ];
    for (i, (action, phase)) in specs.iter().enumerate() {
        nodes.push(make_ticket(
            &format!("backlog.ticket_{i:02}"),
            action.clone(),
            phase.clone(),
        ));
    }

    // Other node types — 2 each where it makes sense
    nodes.push(make_valid_for_type("platform.facts_a", NodeType::Facts));
    nodes.push(make_valid_for_type("platform.facts_b", NodeType::Facts));
    nodes.push(make_valid_for_type("platform.rules_a", NodeType::Rules));
    nodes.push(make_valid_for_type("platform.rules_b", NodeType::Rules));
    nodes.push(make_valid_for_type("impl.workflow_a", NodeType::Workflow));
    nodes.push(make_valid_for_type("impl.workflow_b", NodeType::Workflow));
    nodes.push(make_valid_for_type("model.entity_a", NodeType::Entity));
    nodes.push(make_valid_for_type("model.entity_b", NodeType::Entity));
    nodes.push(make_valid_for_type("arch.decision_a", NodeType::Decision));
    nodes.push(make_valid_for_type("arch.decision_b", NodeType::Decision));
    nodes.push(make_valid_for_type("ops.exception_a", NodeType::Exception));
    nodes.push(make_valid_for_type("docs.example_a", NodeType::Example));
    nodes.push(make_valid_for_type("docs.glossary_a", NodeType::Glossary));
    nodes.push(make_valid_for_type(
        "quality.anti_pattern_a",
        NodeType::AntiPattern,
    ));
    nodes.push(make_valid_for_type(
        "run.orchestration_a",
        NodeType::Orchestration,
    ));
    nodes.push(make_valid_for_type(
        "ext.custom_a",
        NodeType::Custom("lesson".to_owned()),
    ));
    nodes.push(make_valid_for_type(
        "ext.custom_b",
        NodeType::Custom("lesson".to_owned()),
    ));

    AgmFile {
        header: scale_header(),
        nodes,
    }
}

#[test]
fn test_scale_mixed_file_validates_clean() {
    let file = file_mixed_tickets_and_types();
    assert_eq!(file.nodes.len(), 25);

    let src = render_canonical(&file);
    let collection = validate(&file, &src, "mixed.agm", &ValidateOptions::default());
    assert!(
        !collection.has_errors(),
        "unexpected errors: {:?}",
        collection
            .diagnostics()
            .iter()
            .filter(|d| d.severity == agm_core::error::Severity::Error)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_scale_mixed_file_markdown_groups_tickets_section() {
    let file = file_mixed_tickets_and_types();
    let output = render(&file, RenderFormat::Markdown);

    // Every built-in type present must appear as a group heading.
    for heading in [
        "## Tickets",
        "## Facts",
        "## Rules",
        "## Workflow",
        "## Entity",
        "## Decision",
        "## Exception",
        "## Example",
        "## Glossary",
        "## Anti-Pattern",
        "## Orchestration",
    ] {
        assert!(
            output.contains(heading),
            "missing markdown heading: {heading}"
        );
    }

    // Every node id appears somewhere in the output.
    for node in &file.nodes {
        assert!(
            output.contains(&node.id),
            "node id '{}' missing from markdown output",
            node.id
        );
    }
}

#[test]
fn test_scale_mixed_file_canonical_roundtrip_preserves_tickets() {
    let file = file_mixed_tickets_and_types();
    let canonical1 = render_canonical(&file);

    let reparsed = parse(&canonical1)
        .unwrap_or_else(|errs| panic!("re-parse failed: {errs:?}\n\n{canonical1}"));
    let canonical2 = render_canonical(&reparsed);

    assert_eq!(
        canonical1, canonical2,
        "canonical output differs after roundtrip"
    );
    assert_eq!(
        strip_spans(&file),
        strip_spans(&reparsed),
        "AST differs after roundtrip"
    );

    // Every ticket node survived with its action preserved.
    let orig_tickets: Vec<_> = file
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Ticket)
        .collect();
    let reparsed_tickets: Vec<_> = reparsed
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Ticket)
        .collect();
    assert_eq!(orig_tickets.len(), reparsed_tickets.len());
    for (orig, after) in orig_tickets.iter().zip(reparsed_tickets.iter()) {
        assert_eq!(orig.id, after.id);
        assert_eq!(orig.action, after.action);
        assert_eq!(orig.sdd_phase, after.sdd_phase);
        assert_eq!(orig.ticket_id, after.ticket_id);
    }
}

// ---------------------------------------------------------------------------
// 3. Heavy ticket — maximal allowed field populations
// ---------------------------------------------------------------------------

fn file_heavy_ticket() -> AgmFile {
    // Title exactly at the 200-char cap.
    let title: String = "a".repeat(200);

    // Multi-paragraph description.
    let description = (0..20)
        .map(|i| format!("Paragraph {i} of the extended description."))
        .collect::<Vec<_>>()
        .join("\n");

    // Multi-line prompt with a realistic agent-facing body.
    let prompt_lines = [
        "You are operating on a large refactor ticket.",
        "Read the referenced workflow before proposing edits.",
        "Emit a follow-up ticket per out-of-scope finding.",
        "Respect the acceptance criteria listed in the description.",
    ];
    let prompt = prompt_lines.join("\n");

    // Fifty labels — well above any sensible real-world set.
    let labels: Vec<String> = (0..50).map(|i| format!("label_{i:02}")).collect();

    // A fan-out of dependencies on other nodes.
    let depends: Vec<String> = (0..15).map(|i| format!("parent.workflow_{i:02}")).collect();

    let ticket = Node {
        id: "mega.ticket.overhaul".to_owned(),
        node_type: NodeType::Ticket,
        summary: "overhaul ticket with every allowed field populated".to_owned(),
        title: Some(title),
        description: Some(description),
        priority: Some(Priority::Critical),
        action: Some(TicketAction::Create),
        sdd_phase: Some(SddPhase::Propose),
        prompt: Some(prompt),
        assignee: Some("platform-team".to_owned()),
        labels: Some(labels),
        depends: Some(depends),
        ..Default::default()
    };

    // Include the dependency targets so the graph validator can resolve them.
    let mut nodes = vec![ticket];
    for i in 0..15 {
        nodes.push(make_valid_for_type(
            &format!("parent.workflow_{i:02}"),
            NodeType::Workflow,
        ));
    }

    AgmFile {
        header: scale_header(),
        nodes,
    }
}

#[test]
fn test_scale_heavy_ticket_validates_clean() {
    let file = file_heavy_ticket();
    let src = render_canonical(&file);
    let collection = validate(&file, &src, "heavy.agm", &ValidateOptions::default());
    assert!(
        !collection.has_errors(),
        "unexpected errors: {:?}",
        collection
            .diagnostics()
            .iter()
            .filter(|d| d.severity == agm_core::error::Severity::Error)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_scale_heavy_ticket_canonical_roundtrip_preserves_blocks() {
    let file = file_heavy_ticket();
    let canonical1 = render_canonical(&file);
    let reparsed = parse(&canonical1)
        .unwrap_or_else(|errs| panic!("re-parse failed: {errs:?}\n\n{canonical1}"));
    let canonical2 = render_canonical(&reparsed);

    assert_eq!(canonical1, canonical2, "canonical roundtrip not stable");

    let orig_ticket = file
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::Ticket)
        .unwrap();
    let after_ticket = reparsed
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::Ticket)
        .unwrap();

    assert_eq!(orig_ticket.title, after_ticket.title);
    assert_eq!(orig_ticket.description, after_ticket.description);
    assert_eq!(orig_ticket.prompt, after_ticket.prompt);
    assert_eq!(orig_ticket.labels, after_ticket.labels);
    assert_eq!(orig_ticket.depends, after_ticket.depends);
}

#[test]
fn test_scale_heavy_ticket_title_at_200_boundary_no_warning() {
    let file = file_heavy_ticket();
    let src = render_canonical(&file);
    let collection = validate(&file, &src, "heavy.agm", &ValidateOptions::default());
    let has_v032 = collection
        .diagnostics()
        .iter()
        .any(|d| format!("{}", d.code).contains("V032"));
    assert!(
        !has_v032,
        "title of exactly 200 chars should not trigger V032"
    );
}

// ---------------------------------------------------------------------------
// 4. Validator throughput — many invalid tickets in one file
// ---------------------------------------------------------------------------

/// Build a file containing 30 tickets, each with a specific invalid condition.
fn file_many_invalid_tickets() -> AgmFile {
    let mut nodes: Vec<Node> = Vec::with_capacity(30);

    // 10 tickets with non-create action but no ticket_id → V031
    for i in 0..10 {
        nodes.push(Node {
            id: format!("bad.missing_id_{i:02}"),
            node_type: NodeType::Ticket,
            summary: format!("ticket {i}"),
            title: Some(format!("Title {i}")),
            description: Some("desc".to_owned()),
            priority: Some(Priority::Low),
            action: Some(TicketAction::Edit),
            ticket_id: None,
            ..Default::default()
        });
    }

    // 10 tickets with title > 200 chars → V032 (warning)
    for i in 0..10 {
        nodes.push(Node {
            id: format!("bad.long_title_{i:02}"),
            node_type: NodeType::Ticket,
            summary: format!("ticket {i}"),
            title: Some("x".repeat(201 + i)),
            description: Some("desc".to_owned()),
            priority: Some(Priority::Low),
            action: Some(TicketAction::Create),
            ..Default::default()
        });
    }

    // 10 tickets missing required priority → V024
    for i in 0..10 {
        nodes.push(Node {
            id: format!("bad.missing_priority_{i:02}"),
            node_type: NodeType::Ticket,
            summary: format!("ticket {i}"),
            title: Some(format!("Title {i}")),
            description: Some("desc".to_owned()),
            priority: None,
            action: Some(TicketAction::Create),
            ..Default::default()
        });
    }

    AgmFile {
        header: scale_header(),
        nodes,
    }
}

fn count_code(collection: &agm_core::error::DiagnosticCollection, code: &str) -> usize {
    collection
        .diagnostics()
        .iter()
        .filter(|d| format!("{}", d.code).contains(code))
        .count()
}

#[test]
fn test_scale_many_invalid_tickets_produce_expected_diagnostic_counts() {
    let file = file_many_invalid_tickets();
    let src = render_canonical(&file);
    let collection = validate(&file, &src, "bad.agm", &ValidateOptions::default());

    assert_eq!(
        count_code(&collection, "V031"),
        10,
        "expected 10 V031 diagnostics"
    );
    assert_eq!(
        count_code(&collection, "V032"),
        10,
        "expected 10 V032 diagnostics"
    );
    // V024 is raised once per missing required field (title/description/priority).
    // Here only priority is missing on those 10, so at least 10 V024s.
    assert!(
        count_code(&collection, "V024") >= 10,
        "expected at least 10 V024 diagnostics, got {}",
        count_code(&collection, "V024")
    );
}
