//! Round-trip parser tests for `type: ticket` nodes (§7.5 of the plan).
//!
//! Verifies that parsing → rendering canonical → parsing again produces the
//! same model (deep equality), and that multi-line block fields (prompt,
//! description) survive the round trip without modification.

use agm_core::model::fields::NodeType;
use agm_core::parser::parse;
use agm_core::renderer::canonical::render_canonical;

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_fixture(relative: &str) -> String {
    let path = fixtures_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"))
}

// ---------------------------------------------------------------------------
// §7.5 — test_parser_ticket_full_roundtrip_preserves_all_fields
// ---------------------------------------------------------------------------

/// Parse `valid/ticket_full.agm`, render to canonical AGM text, parse again,
/// and assert the two `AgmFile` values are deeply equal.
#[test]
fn test_parser_ticket_full_roundtrip_preserves_all_fields() {
    let text = read_fixture("valid/ticket_full.agm");

    let first = parse(&text).unwrap_or_else(|errs| {
        panic!("first parse failed: {errs:?}");
    });

    let canonical = render_canonical(&first);

    let second = parse(&canonical).unwrap_or_else(|errs| {
        panic!(
            "second parse (of canonical output) failed: {errs:?}\n--- canonical ---\n{canonical}"
        );
    });

    assert_eq!(
        first.header, second.header,
        "headers must be equal after round trip"
    );
    assert_eq!(
        first.nodes.len(),
        second.nodes.len(),
        "node count must be equal after round trip"
    );

    for (a, b) in first.nodes.iter().zip(second.nodes.iter()) {
        assert_eq!(a.id, b.id, "node id must survive round trip");
        assert_eq!(
            a.node_type, b.node_type,
            "node_type must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.summary, b.summary,
            "summary must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.title, b.title,
            "title must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.description, b.description,
            "description must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.priority, b.priority,
            "priority must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.action, b.action,
            "action must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.sdd_phase, b.sdd_phase,
            "sdd_phase must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.assignee, b.assignee,
            "assignee must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.labels, b.labels,
            "labels must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.prompt, b.prompt,
            "prompt must survive round trip for {}",
            a.id
        );
        assert_eq!(
            a.ticket_id, b.ticket_id,
            "ticket_id must survive round trip for {}",
            a.id
        );
    }

    // Confirm the node type was preserved
    let ticket = first
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::Ticket)
        .expect("expected a ticket node in ticket_full.agm");
    assert_eq!(ticket.node_type, NodeType::Ticket);
}

// ---------------------------------------------------------------------------
// §7.5 — test_parser_ticket_prompt_multiline_preserved
// ---------------------------------------------------------------------------

/// Verify that a block-literal `prompt:` field with multiple lines survives
/// the parse → canonical-render → parse round trip with each line intact.
#[test]
fn test_parser_ticket_prompt_multiline_preserved() {
    let text = read_fixture("valid/ticket_full.agm");

    let file = parse(&text).unwrap_or_else(|errs| {
        panic!("parse failed: {errs:?}");
    });

    let ticket = file
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::Ticket)
        .expect("expected a ticket node in ticket_full.agm");

    let original_prompt = ticket
        .prompt
        .clone()
        .expect("ticket_full.agm must have a prompt field");

    // The fixture prompt spans two lines — verify both lines are present
    assert!(
        original_prompt.contains("Design a minimal OAuth2 flow"),
        "first prompt line must be present: got {original_prompt:?}"
    );
    assert!(
        original_prompt.contains("Emit a workflow node"),
        "second prompt line must be present: got {original_prompt:?}"
    );

    // Round trip
    let canonical = render_canonical(&file);
    let file2 = parse(&canonical).unwrap_or_else(|errs| {
        panic!("second parse failed: {errs:?}\n--- canonical ---\n{canonical}");
    });

    let ticket2 = file2
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::Ticket)
        .expect("expected a ticket node after round trip");

    let round_trip_prompt = ticket2
        .prompt
        .clone()
        .expect("prompt must survive round trip");

    assert_eq!(
        original_prompt, round_trip_prompt,
        "prompt content must be identical after parse → canonical → parse"
    );
}
