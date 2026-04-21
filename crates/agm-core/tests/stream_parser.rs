//! Integration tests for `StreamParser`.
//!
//! Tests cover:
//! - Equivalence with batch `parser::parse` across all fixtures × multiple chunk sizes.
//! - Event ordering invariants.
//! - CRLF handling.
//! - Final-node flush without trailing blank.

use std::fs;
use std::path::PathBuf;

use agm_core::error::AgmError;
use agm_core::model::file::AgmFile;
use agm_core::parser::{self, ParseEvent, StreamParser};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Feed `input` to a `StreamParser` in chunks of `chunk_size` bytes.
/// Returns all events (push_chunk + finish) and the finish result.
fn feed(input: &str, chunk_size: usize) -> (Vec<ParseEvent>, Result<AgmFile, Vec<AgmError>>) {
    let mut p = StreamParser::new();
    let mut events = Vec::new();
    let mut start = 0;
    while start < input.len() {
        let end = (start + chunk_size).min(input.len());
        events.extend(p.push_chunk(&input[start..end]));
        start = end;
    }
    let (finish_events, result) = p.finish();
    events.extend(finish_events);
    (events, result)
}

/// Sorts a `Vec<AgmError>` by (code as display string, line, message) for
/// order-independent comparison.
fn sort_errors(mut v: Vec<AgmError>) -> Vec<AgmError> {
    v.sort_by_key(|e| {
        (
            format!("{:?}", e.code),
            e.location.line.unwrap_or(0),
            e.message.clone(),
        )
    });
    v
}

/// Collects all `.agm` files under `dir`, up to `cap` entries.
fn collect_fixtures(dir: &str, cap: usize) -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(dir);
    if !base.exists() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(&base)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "agm"))
        .collect();
    paths.sort();
    paths.truncate(cap);
    paths
}

// ---------------------------------------------------------------------------
// Equivalence test
// ---------------------------------------------------------------------------

/// For every fixture, the stream parser must produce the same Ok/Err result as
/// the batch parser, across all chunk sizes.
///
/// Error *set* comparison is relaxed to multiset of (ErrorCode, line) pairs
/// because the stream parser emits errors per-node while the batch parser
/// collects all errors at the end, so ordering may differ.
#[test]
fn test_stream_equivalent_to_batch_on_all_fixtures() {
    let chunk_sizes: &[usize] = &[1, 2, 4, 16, 64, usize::MAX];

    // Collect valid + invalid fixtures (cap at 30 total).
    let mut fixtures: Vec<PathBuf> = Vec::new();
    fixtures.extend(collect_fixtures("valid", 15));
    fixtures.extend(collect_fixtures("invalid", 15));

    // Hand-built inline cases that exercise boundary conditions.
    let empty_str = String::new();
    let header_only = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n".to_string();
    let single_no_trailing_nl =
        "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.x\ntype: facts\nsummary: s"
            .to_string();
    let two_nodes =
        "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.one\ntype: facts\nsummary: first\n\nnode a.two\ntype: rules\nsummary: second\n".to_string();
    let crlf_input =
        "agm: 1.0\r\npackage: test.pkg\r\nversion: 0.1.0\r\n\r\nnode a.x\r\ntype: facts\r\nsummary: s\r\n"
            .to_string();

    let inline: Vec<(&str, String)> = vec![
        ("empty", empty_str),
        ("header_only", header_only),
        ("single_no_trailing_nl", single_no_trailing_nl),
        ("two_nodes", two_nodes),
        ("crlf_input", crlf_input),
    ];

    for &chunk_size in chunk_sizes {
        // Test fixtures.
        for fixture_path in &fixtures {
            let content = fs::read_to_string(fixture_path).unwrap();
            let batch_result = parser::parse(&content);
            let (_, stream_result) = feed(&content, chunk_size);

            let label = format!(
                "{} chunk_size={}",
                fixture_path.file_name().unwrap().to_string_lossy(),
                chunk_size
            );
            compare_results(&label, batch_result, stream_result);
        }

        // Test inline cases.
        for (name, content) in &inline {
            // For CRLF, batch parser uses str::lines() which strips \r,
            // so we strip \r before feeding batch (same as StreamParser does).
            let batch_content = content.replace('\r', "");
            let batch_result = parser::parse(&batch_content);
            let (_, stream_result) = feed(content, chunk_size);

            let label = format!("{name} chunk_size={chunk_size}");
            compare_results(&label, batch_result, stream_result);
        }
    }
}

fn compare_results(
    label: &str,
    batch: Result<AgmFile, Vec<AgmError>>,
    stream: Result<AgmFile, Vec<AgmError>>,
) {
    match (batch, stream) {
        (Ok(b), Ok(s)) => {
            assert_eq!(b, s, "[{label}] AgmFile mismatch");
        }
        (Err(b_errs), Err(s_errs)) => {
            // Relaxed comparison: multiset of (ErrorCode, line) pairs.
            // Order may differ because the stream parser emits per-node while
            // the batch parser accumulates all errors before returning.
            let mut b_keys: Vec<_> = b_errs
                .iter()
                .map(|e| (format!("{:?}", e.code), e.location.line.unwrap_or(0)))
                .collect();
            let mut s_keys: Vec<_> = s_errs
                .iter()
                .map(|e| (format!("{:?}", e.code), e.location.line.unwrap_or(0)))
                .collect();
            b_keys.sort();
            s_keys.sort();
            assert_eq!(
                b_keys, s_keys,
                "[{label}] error sets differ\n  batch:  {b_errs:?}\n  stream: {s_errs:?}"
            );
        }
        (Ok(_), Err(s_errs)) => {
            panic!("[{label}] batch=Ok but stream=Err({s_errs:?})");
        }
        (Err(b_errs), Ok(_)) => {
            panic!("[{label}] batch=Err({b_errs:?}) but stream=Ok");
        }
    }
}

// ---------------------------------------------------------------------------
// Event ordering tests
// ---------------------------------------------------------------------------

/// For every fixture, validate the partial order invariants:
/// - HeaderComplete (if present) precedes all NodeStarted/NodeComplete.
/// - EndOfFile is the last event.
#[test]
fn test_stream_events_partial_order() {
    let mut fixtures: Vec<PathBuf> = Vec::new();
    fixtures.extend(collect_fixtures("valid", 15));
    fixtures.extend(collect_fixtures("invalid", 15));

    let inline: Vec<String> = vec![
        String::new(),
        "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n".to_string(),
        "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.x\ntype: facts\nsummary: s\n"
            .to_string(),
    ];

    for fixture_path in &fixtures {
        let content = fs::read_to_string(fixture_path).unwrap();
        let label = fixture_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        check_ordering(&label, &content);
    }
    for (i, content) in inline.iter().enumerate() {
        check_ordering(&format!("inline_{i}"), content);
    }
}

/// Validate event ordering invariants on a single input using whole-file chunk.
fn check_ordering(label: &str, input: &str) {
    let (events, _) = feed(input, usize::MAX);

    // 1. EndOfFile is the last event.
    if let Some(last) = events.last() {
        assert!(
            matches!(last, ParseEvent::EndOfFile),
            "[{label}] last event must be EndOfFile, got something else"
        );
    } else {
        // No events at all — should not happen (finish always emits EndOfFile).
        panic!("[{label}] no events emitted");
    }

    // 2. HeaderComplete (if present) precedes all NodeStarted/NodeComplete.
    let header_pos = events
        .iter()
        .position(|e| matches!(e, ParseEvent::HeaderComplete(_)));
    if let Some(hp) = header_pos {
        for (i, ev) in events.iter().enumerate() {
            if matches!(
                ev,
                ParseEvent::NodeStarted { .. } | ParseEvent::NodeComplete(_)
            ) {
                assert!(
                    i > hp,
                    "[{label}] HeaderComplete at {hp} must precede NodeStarted/NodeComplete at {i}"
                );
            }
        }
    }
}

/// For every NodeStarted(id), there must be a later NodeComplete(node) with
/// the same id. (parse_node is infallible, so this holds unconditionally.)
#[test]
fn test_stream_ordering_invariant() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");

    let mut all_fixtures: Vec<PathBuf> = Vec::new();
    for subdir in &["valid", "invalid"] {
        let dir = fixtures_dir.join(subdir);
        if dir.exists() {
            let mut v: Vec<PathBuf> = fs::read_dir(&dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "agm"))
                .collect();
            v.sort();
            all_fixtures.extend(v);
        }
    }

    let extra_inputs: Vec<String> = vec![
        "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.one\ntype: facts\nsummary: first\n\nnode a.two\ntype: rules\nsummary: second\n".to_string(),
    ];

    let chunk_sizes: &[usize] = &[1, 2, 4, 16, 64, usize::MAX];

    for &chunk_size in chunk_sizes {
        for fixture_path in &all_fixtures {
            let content = fs::read_to_string(fixture_path).unwrap();
            let label = format!(
                "{} cs={}",
                fixture_path.file_name().unwrap().to_string_lossy(),
                chunk_size
            );
            check_started_complete_pairs(&label, &content, chunk_size);
        }
        for (i, content) in extra_inputs.iter().enumerate() {
            check_started_complete_pairs(
                &format!("extra_{i} cs={chunk_size}"),
                content,
                chunk_size,
            );
        }
    }
}

fn check_started_complete_pairs(label: &str, input: &str, chunk_size: usize) {
    let (events, _) = feed(input, chunk_size);

    // Collect all NodeStarted ids in order.
    let started_ids: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            ParseEvent::NodeStarted { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();

    for id in &started_ids {
        // Find the NodeStarted position.
        let started_pos = events
            .iter()
            .position(|e| matches!(e, ParseEvent::NodeStarted { id: sid, .. } if sid == id))
            .unwrap();

        // There must be a NodeComplete with the same id after it.
        let complete_pos = events.iter().enumerate().position(|(i, e)| {
            i > started_pos && matches!(e, ParseEvent::NodeComplete(n) if n.id == *id)
        });

        assert!(
            complete_pos.is_some(),
            "[{label}] NodeStarted({id}) has no matching NodeComplete after it"
        );
    }
}

// ---------------------------------------------------------------------------
// Specific scenario tests
// ---------------------------------------------------------------------------

#[test]
fn test_stream_handles_crlf() {
    let lf_input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.x\ntype: facts\nsummary: crlf test\n";
    let crlf_input = lf_input.replace('\n', "\r\n");

    let (_, lf_result) = feed(lf_input, usize::MAX);
    let (_, crlf_result) = feed(&crlf_input, usize::MAX);

    match (lf_result, crlf_result) {
        (Ok(lf), Ok(cr)) => {
            assert_eq!(lf, cr, "CRLF and LF must produce identical AgmFile");
        }
        (Err(le), Err(ce)) => {
            let le = sort_errors(le);
            let ce = sort_errors(ce);
            assert_eq!(le, ce, "CRLF and LF must produce same errors");
        }
        _ => panic!("CRLF and LF disagree on Ok/Err"),
    }
}

#[test]
fn test_stream_final_node_flushes_without_trailing_blank() {
    let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
                 node a.first\ntype: facts\nsummary: first\n\n\
                 node a.last\ntype: rules\nsummary: last";
    // No trailing newline on the last summary line.

    let (events, result) = feed(input, usize::MAX);
    assert!(result.is_ok(), "expected Ok: {:?}", result);
    let file = result.unwrap();
    assert_eq!(file.nodes.len(), 2);

    assert!(
        events
            .iter()
            .any(|e| matches!(e, ParseEvent::NodeComplete(n) if n.id == "a.last")),
        "NodeComplete for a.last must be emitted"
    );
}
