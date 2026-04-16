//! Snapshot tests for all renderers using insta.
//!
//! Run `cargo insta review` after first run to accept snapshots.

use agm_core::parser::parse;
use agm_core::renderer::{
    RenderFormat,
    canonical::render_canonical,
    graph::{render_graph_dot, render_graph_mermaid},
    json::render_json,
    json_canonical::render_json_canonical,
    markdown::render_markdown,
    render,
};

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

fn crate_fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_fixture(relative: &str) -> String {
    let path = fixtures_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"))
}

fn parse_fixture(relative: &str) -> agm_core::model::file::AgmFile {
    let text = read_fixture(relative);
    parse(&text).unwrap_or_else(|errs| panic!("parse errors in {relative}: {errs:?}"))
}

fn parse_crate_fixture(relative: &str) -> agm_core::model::file::AgmFile {
    let path = crate_fixtures_root().join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {relative}: {e}"));
    parse(&text).unwrap_or_else(|errs| panic!("parse errors in {relative}: {errs:?}"))
}

// ---------------------------------------------------------------------------
// JSON renderer snapshots
// ---------------------------------------------------------------------------

#[test]
fn test_render_json_minimal_snapshot() {
    // Use json/forward/minimal.agm (uses agm: 1.0 which parses correctly)
    let file = parse_fixture("json/forward/minimal.agm");
    let output = render_json(&file);
    insta::assert_snapshot!("render_json_minimal", output);
}

#[test]
fn test_render_json_canonical_minimal_snapshot() {
    let file = parse_fixture("json/forward/minimal.agm");
    let output = render_json_canonical(&file);
    insta::assert_snapshot!("render_json_canonical_minimal", output);
}

// ---------------------------------------------------------------------------
// Markdown renderer snapshot
// ---------------------------------------------------------------------------

#[test]
fn test_render_markdown_minimal_snapshot() {
    let file = parse_fixture("json/forward/minimal.agm");
    let output = render_markdown(&file);
    insta::assert_snapshot!("render_markdown_minimal", output);
}

// ---------------------------------------------------------------------------
// Canonical AGM text renderer snapshot
// ---------------------------------------------------------------------------

#[test]
fn test_render_canonical_minimal_snapshot() {
    let file = parse_fixture("json/forward/minimal.agm");
    let output = render_canonical(&file);
    insta::assert_snapshot!("render_canonical_minimal", output);
}

// ---------------------------------------------------------------------------
// Graph renderer snapshots (auth_platform — rich fixture with edges)
// ---------------------------------------------------------------------------

#[test]
fn test_render_dot_auth_platform_snapshot() {
    let file = parse_fixture("json/forward/auth_platform.agm");
    let output = render_graph_dot(&file);
    insta::assert_snapshot!("render_dot_auth_platform", output);
}

#[test]
fn test_render_mermaid_auth_platform_snapshot() {
    let file = parse_fixture("json/forward/auth_platform.agm");
    let output = render_graph_mermaid(&file);
    insta::assert_snapshot!("render_mermaid_auth_platform", output);
}

// ---------------------------------------------------------------------------
// Ticket renderer snapshots
// ---------------------------------------------------------------------------

#[test]
fn test_render_canonical_ticket_create_snapshot() {
    let file = parse_crate_fixture("valid/ticket_create.agm");
    let output = render_canonical(&file);
    insta::assert_snapshot!("renderer__canonical__ticket_create", output);
}

#[test]
fn test_render_canonical_ticket_full_snapshot() {
    let file = parse_crate_fixture("valid/ticket_full.agm");
    let output = render_canonical(&file);
    insta::assert_snapshot!("renderer__canonical__ticket_full", output);
}

#[test]
fn test_render_json_ticket_create_snapshot() {
    let file = parse_crate_fixture("valid/ticket_create.agm");
    let output = render_json(&file);
    insta::assert_snapshot!("renderer__json__ticket_create", output);
}

#[test]
fn test_render_markdown_ticket_create_snapshot() {
    let file = parse_crate_fixture("valid/ticket_create.agm");
    let output = render_markdown(&file);
    insta::assert_snapshot!("renderer__markdown__ticket_create", output);
}

// ---------------------------------------------------------------------------
// render() dispatch function smoke test
// ---------------------------------------------------------------------------

#[test]
fn test_render_dispatch_all_formats_do_not_panic() {
    let file = parse_fixture("json/forward/minimal.agm");
    let formats = [
        RenderFormat::Json,
        RenderFormat::JsonCanonical,
        RenderFormat::Markdown,
        RenderFormat::Canonical,
        RenderFormat::Dot,
        RenderFormat::Mermaid,
    ];
    for format in formats {
        let output = render(&file, format);
        assert!(
            !output.is_empty(),
            "format {format:?} produced empty output"
        );
    }
}
