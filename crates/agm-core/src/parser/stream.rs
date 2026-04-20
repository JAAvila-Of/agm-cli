//! Chunk-based streaming parser for AGM text.
//!
//! [`StreamParser`] lets you feed AGM source text in arbitrary-sized chunks and
//! receive [`ParseEvent`]s as node and header boundaries are reached. This is
//! useful when you receive AGM text incrementally (e.g. from a streaming model
//! response) and want to render nodes to the user before the full document
//! arrives.
//!
//! # Event ordering contract
//!
//! 1. If a [`ParseEvent::HeaderComplete`] event is emitted, it precedes every
//!    [`ParseEvent::NodeStarted`] and [`ParseEvent::NodeComplete`] event.
//! 2. For each node `X`, `NodeStarted { id: X }` strictly precedes the
//!    corresponding `NodeComplete(node)` where `node.id == X`.
//! 3. [`ParseEvent::EndOfFile`] is the last event, emitted only from
//!    [`StreamParser::finish`].
//!
//! # Gotchas
//!
//! - `HeaderComplete` is **not** emitted for files that contain no nodes,
//!   because the header boundary is only detected when the first `node` line
//!   is seen. The header is still parsed internally and included in the
//!   `AgmFile` returned by [`StreamParser::finish`].
//! - The final node is only flushed when [`StreamParser::finish`] is called.
//!   Callers must always call `finish` to get the complete output.
//! - The API is fully synchronous. Async callers can wrap `push_chunk` in a
//!   blocking task or thread-pool executor.

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::file::{AgmFile, Header};
use crate::model::node::Node;
use crate::parser::header::parse_header;
use crate::parser::lexer::{Line, LineKind, classify_line};
use crate::parser::node::parse_node;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Events emitted by [`StreamParser`] as AGM text is consumed.
///
/// The event sequence is ordered by the [contract in the module docs](self).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ParseEvent {
    /// The header section is fully parsed. Emitted at most once, and only when
    /// the first `node` line is seen (header/node boundary detected).
    HeaderComplete(Box<Header>),
    /// A `node <id>` line was consumed. This is an early signal — the node is
    /// not yet complete. Do **not** use this for authoritative node data; wait
    /// for the corresponding [`ParseEvent::NodeComplete`].
    NodeStarted { id: String, line: usize },
    /// A node is fully parsed and ready for use.
    NodeComplete(Box<Node>),
    /// A non-fatal advisory warning that occurred during parsing.
    Warning(AgmError),
    /// A parse error. Parsing continues from the next node boundary.
    Error(AgmError),
    /// Emitted exactly once from [`StreamParser::finish`] when input ends.
    EndOfFile,
}

// ---------------------------------------------------------------------------
// Private state machine
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum State {
    /// No lines consumed yet.
    BeforeHeader,
    /// Consuming header lines; no `node` line seen yet.
    InHeader,
    /// Header has been flushed; between two nodes (blank/comment gap).
    BetweenNodes,
    /// Inside a node body.
    InNode,
    /// `finish()` has been called — parser is consumed.
    Done,
}

// ---------------------------------------------------------------------------
// StreamParser
// ---------------------------------------------------------------------------

/// A push-based, chunk-oriented AGM parser.
///
/// Feed text with [`push_chunk`][StreamParser::push_chunk], then call
/// [`finish`][StreamParser::finish] to signal end-of-stream and receive the
/// assembled [`AgmFile`].
///
/// ## Equivalence with batch parsing
///
/// For any input split into arbitrary chunks:
/// ```text
/// StreamParser.push_chunk(chunks...).finish().1 == parser::parse(whole_input)
/// ```
/// (same `Ok(AgmFile)` or same error set, modulo ordering).
#[derive(Debug)]
pub struct StreamParser {
    /// Bytes received but not yet ending in `\n`.
    pending_text: String,
    /// Lines belonging to the current section (header or node body).
    buffered_lines: Vec<Line>,
    state: State,
    /// Populated once the header is parsed.
    header: Option<Header>,
    all_nodes: Vec<Node>,
    all_errors: Vec<AgmError>,
    /// 1-based line counter for lines fully consumed so far.
    line_number: usize,
    /// Total bytes fed via `push_chunk` (not counting the synthetic `\n`
    /// appended internally by `finish`).
    bytes_consumed: usize,
}

impl StreamParser {
    /// Creates a new, empty `StreamParser`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_text: String::new(),
            buffered_lines: Vec::new(),
            state: State::BeforeHeader,
            header: None,
            all_nodes: Vec::new(),
            all_errors: Vec::new(),
            line_number: 0,
            bytes_consumed: 0,
        }
    }

    /// Consumes a chunk of AGM text and returns any events completed by it.
    ///
    /// The chunk may end mid-line or mid-node; the parser buffers the partial
    /// tail automatically.
    #[must_use]
    pub fn push_chunk(&mut self, chunk: &str) -> Vec<ParseEvent> {
        self.bytes_consumed += chunk.len();
        self.pending_text.push_str(chunk);
        self.drain_lines()
    }

    /// Signals end-of-stream.
    ///
    /// Flushes any buffered partial line and the final node (if any), emits
    /// [`ParseEvent::EndOfFile`], and returns the assembled [`AgmFile`].
    ///
    /// Returns `Err(errors)` if any hard parse error was encountered (matching
    /// the same criterion as [`crate::parser::parse`]).
    pub fn finish(mut self) -> (Vec<ParseEvent>, Result<AgmFile, Vec<AgmError>>) {
        // If the input did not end with '\n', append one so the last line is
        // fully consumed by drain_lines.
        if !self.pending_text.is_empty() && !self.pending_text.ends_with('\n') {
            self.pending_text.push('\n');
        }

        let mut events = self.drain_lines();

        // Flush the final node (or header-only file).
        let flush_events = self.flush_current_section();
        events.extend(flush_events);

        // P008: no nodes
        if self.all_nodes.is_empty() {
            let err = AgmError::new(
                ErrorCode::P008,
                "Empty file (no nodes)",
                ErrorLocation::new(None, None, None),
            );
            self.all_errors.push(err.clone());
            events.push(ParseEvent::Error(err));
        }

        // Mark as done.
        self.state = State::Done;
        events.push(ParseEvent::EndOfFile);

        // Build AgmFile header. If we never saw a header (e.g. empty input),
        // call parse_header on an empty slice — it pushes P001 errors for
        // missing required fields. Those errors are already in all_errors for
        // the Result computation below; this edge case is also covered by P008.
        let header = self.header.unwrap_or_else(|| {
            let mut pos = 0;
            parse_header(&[], &mut pos, &mut self.all_errors)
        });

        let result = if self.all_errors.iter().any(|e| e.is_error()) {
            Err(self.all_errors)
        } else {
            Ok(AgmFile {
                header,
                nodes: self.all_nodes,
            })
        };

        (events, result)
    }

    /// Total bytes fed via [`push_chunk`][Self::push_chunk].
    ///
    /// Does not include any synthetic newline appended internally by `finish`.
    #[must_use]
    pub fn bytes_consumed(&self) -> usize {
        self.bytes_consumed
    }

    /// Number of lines fully consumed (excludes any buffered partial tail).
    #[must_use]
    pub fn lines_consumed(&self) -> usize {
        self.line_number
    }

    /// Returns `true` once the header has been parsed and
    /// [`ParseEvent::HeaderComplete`] has been emitted.
    #[must_use]
    pub fn header_complete(&self) -> bool {
        self.header.is_some()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Drains all complete lines from `pending_text`, runs the state machine,
    /// and returns accumulated events.
    fn drain_lines(&mut self) -> Vec<ParseEvent> {
        let mut events = Vec::new();

        loop {
            // Find the next newline.
            let Some(nl_pos) = self.pending_text.find('\n') else {
                break;
            };

            // Slice out the line (without the '\n'), strip a trailing '\r'.
            let raw_line = self.pending_text[..nl_pos]
                .strip_suffix('\r')
                .unwrap_or(&self.pending_text[..nl_pos]);

            self.line_number += 1;
            let line_num = self.line_number;

            // Classify the line.
            match classify_line(raw_line, line_num) {
                Err(err) => {
                    // Tab or other lex error — push to all_errors and emit.
                    self.all_errors.push(err.clone());
                    events.push(ParseEvent::Error(err));
                    // Consume the line from pending_text.
                    self.pending_text.drain(..nl_pos + 1);
                }
                Ok(line) => {
                    // Consume the line from pending_text before running the
                    // state machine (avoids borrow issues).
                    self.pending_text.drain(..nl_pos + 1);
                    let step_events = self.step(line);
                    events.extend(step_events);
                }
            }
        }

        events
    }

    /// Processes a single classified line through the state machine.
    fn step(&mut self, line: Line) -> Vec<ParseEvent> {
        let mut events = Vec::new();

        match &self.state {
            State::BeforeHeader | State::InHeader => {
                match &line.kind {
                    LineKind::NodeDeclaration(id) => {
                        // First node declaration — flush header.
                        let id = id.clone();
                        let line_num = line.number;

                        let header_events = self.flush_header();
                        events.extend(header_events);

                        // Transition to InNode; push the NodeDeclaration line.
                        self.buffered_lines.push(line);
                        self.state = State::InNode;
                        events.push(ParseEvent::NodeStarted { id, line: line_num });
                    }
                    _ => {
                        // Header content (field, blank, comment, etc.).
                        self.state = State::InHeader;
                        self.buffered_lines.push(line);
                    }
                }
            }

            State::BetweenNodes => {
                if let LineKind::NodeDeclaration(id) = &line.kind {
                    let id = id.clone();
                    let line_num = line.number;
                    self.buffered_lines.push(line);
                    self.state = State::InNode;
                    events.push(ParseEvent::NodeStarted { id, line: line_num });
                }
                // Blank / comment between nodes — ignored.
            }

            State::InNode => {
                match &line.kind {
                    LineKind::NodeDeclaration(id) => {
                        let id = id.clone();
                        let line_num = line.number;

                        // Flush the previous node.
                        let node_events = self.flush_node();
                        events.extend(node_events);

                        // Start the new node.
                        self.buffered_lines.push(line);
                        self.state = State::InNode;
                        events.push(ParseEvent::NodeStarted { id, line: line_num });
                    }
                    _ => {
                        self.buffered_lines.push(line);
                    }
                }
            }

            State::Done => {
                // Parser already finished — ignore any further lines.
            }
        }

        events
    }

    /// Flushes buffered header lines through `parse_header`, emits
    /// `HeaderComplete`, and transitions to `BetweenNodes`.
    ///
    /// Returns the events produced (HeaderComplete + any error/warning events).
    fn flush_header(&mut self) -> Vec<ParseEvent> {
        let mut events = Vec::new();
        let lines = std::mem::take(&mut self.buffered_lines);

        let errors_before = self.all_errors.len();
        let mut pos = 0;
        let header = parse_header(&lines, &mut pos, &mut self.all_errors);

        // Emit events for newly added errors.
        let new_errs = self.all_errors[errors_before..].to_vec();
        for err in new_errs {
            if err.is_error() {
                events.push(ParseEvent::Error(err));
            } else {
                events.push(ParseEvent::Warning(err));
            }
        }

        self.header = Some(header.clone());
        events.push(ParseEvent::HeaderComplete(Box::new(header)));
        self.state = State::BetweenNodes;
        events
    }

    /// Flushes buffered node lines through `parse_node`, emits `NodeComplete`,
    /// and transitions to `BetweenNodes`.
    ///
    /// Returns the events produced.
    fn flush_node(&mut self) -> Vec<ParseEvent> {
        let mut events = Vec::new();
        let lines = std::mem::take(&mut self.buffered_lines);

        if lines.is_empty() {
            self.state = State::BetweenNodes;
            return events;
        }

        let errors_before = self.all_errors.len();
        let mut pos = 0;
        let node = parse_node(&lines, &mut pos, &mut self.all_errors);

        // Emit events for newly added errors.
        let new_errs = self.all_errors[errors_before..].to_vec();
        for err in new_errs {
            if err.is_error() {
                events.push(ParseEvent::Error(err));
            } else {
                events.push(ParseEvent::Warning(err));
            }
        }

        self.all_nodes.push(node.clone());
        events.push(ParseEvent::NodeComplete(Box::new(node)));
        self.state = State::BetweenNodes;
        events
    }

    /// Called from `finish()` to flush whatever is currently buffered.
    ///
    /// - `InHeader`: parse header (no `HeaderComplete` emitted — no node
    ///   boundary was ever seen).
    /// - `InNode`: flush the final node.
    /// - Others: no-op.
    fn flush_current_section(&mut self) -> Vec<ParseEvent> {
        match self.state {
            State::InHeader => {
                // Header-only file: parse header internally but do NOT emit
                // HeaderComplete (contract: HeaderComplete requires seeing a
                // node boundary).
                let lines = std::mem::take(&mut self.buffered_lines);
                let errors_before = self.all_errors.len();
                let mut pos = 0;
                let header = parse_header(&lines, &mut pos, &mut self.all_errors);
                self.header = Some(header);

                // Emit error/warning events for header parse errors.
                let mut events = Vec::new();
                let new_errs = self.all_errors[errors_before..].to_vec();
                for err in new_errs {
                    if err.is_error() {
                        events.push(ParseEvent::Error(err));
                    } else {
                        events.push(ParseEvent::Warning(err));
                    }
                }
                self.state = State::BetweenNodes;
                events
            }
            State::InNode => self.flush_node(),
            _ => Vec::new(),
        }
    }
}

impl Default for StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn minimal_valid() -> &'static str {
        "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.node\ntype: facts\nsummary: A test node\n"
    }

    fn two_nodes() -> &'static str {
        "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\n\
         node a.one\ntype: facts\nsummary: first\n\n\
         node a.two\ntype: rules\nsummary: second\n"
    }

    /// Feeds a string chunk-by-chunk (chunk_size bytes per push) and returns
    /// all events plus the finish result.
    fn feed(input: &str, chunk_size: usize) -> (Vec<ParseEvent>, Result<AgmFile, Vec<AgmError>>) {
        let mut parser = StreamParser::new();
        let mut events = Vec::new();
        let mut start = 0;
        while start < input.len() {
            let end = (start + chunk_size).min(input.len());
            events.extend(parser.push_chunk(&input[start..end]));
            start = end;
        }
        let (finish_events, result) = parser.finish();
        events.extend(finish_events);
        (events, result)
    }

    fn has_event<F>(events: &[ParseEvent], f: F) -> bool
    where
        F: Fn(&ParseEvent) -> bool,
    {
        events.iter().any(f)
    }

    fn count_event<F>(events: &[ParseEvent], f: F) -> usize
    where
        F: Fn(&ParseEvent) -> bool,
    {
        events.iter().filter(|e| f(e)).count()
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_is_empty() {
        let p = StreamParser::new();
        assert_eq!(p.bytes_consumed(), 0);
        assert_eq!(p.lines_consumed(), 0);
        assert!(!p.header_complete());
    }

    #[test]
    fn test_header_complete_emits_once() {
        let (events, _) = feed(minimal_valid(), usize::MAX);
        let count = count_event(&events, |e| matches!(e, ParseEvent::HeaderComplete(_)));
        assert_eq!(count, 1, "expected exactly one HeaderComplete event");
    }

    #[test]
    fn test_node_started_before_node_complete() {
        let (events, _) = feed(two_nodes(), usize::MAX);

        for node_id in &["a.one", "a.two"] {
            let started_pos = events
                .iter()
                .position(|e| matches!(e, ParseEvent::NodeStarted { id, .. } if id == node_id));
            let complete_pos = events
                .iter()
                .position(|e| matches!(e, ParseEvent::NodeComplete(n) if n.id == *node_id));

            assert!(started_pos.is_some(), "NodeStarted for {node_id} not found");
            assert!(
                complete_pos.is_some(),
                "NodeComplete for {node_id} not found"
            );
            assert!(
                started_pos.unwrap() < complete_pos.unwrap(),
                "NodeStarted must precede NodeComplete for {node_id}"
            );
        }
    }

    #[test]
    fn test_push_chunk_byte_by_byte() {
        let input = minimal_valid();
        let (events, result) = feed(input, 1);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        assert!(has_event(&events, |e| matches!(
            e,
            ParseEvent::HeaderComplete(_)
        )));
        assert!(has_event(&events, |e| matches!(
            e,
            ParseEvent::NodeComplete(_)
        )));
        assert!(has_event(&events, |e| matches!(e, ParseEvent::EndOfFile)));
    }

    #[test]
    fn test_finish_flushes_last_node() {
        // Feed a file where the last node has no trailing blank line.
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.last\ntype: facts\nsummary: last node";
        let (events, result) = feed(input, usize::MAX);
        assert!(result.is_ok(), "expected Ok: {:?}", result);
        assert!(
            has_event(
                &events,
                |e| matches!(e, ParseEvent::NodeComplete(n) if n.id == "a.last")
            ),
            "NodeComplete for a.last not found"
        );
    }

    #[test]
    fn test_empty_file_produces_p008() {
        let (events, result) = feed("", usize::MAX);
        assert!(result.is_err(), "expected Err for empty input");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.code == ErrorCode::P008),
            "expected P008 in errors"
        );
        assert!(
            has_event(
                &events,
                |e| matches!(e, ParseEvent::Error(err) if err.code == ErrorCode::P008)
            ),
            "expected ParseEvent::Error(P008)"
        );
    }

    #[test]
    fn test_missing_header_field_surfaces_as_error_event() {
        // Missing `version:` field.
        let input = "agm: 1.0\npackage: test.pkg\n\nnode a.node\ntype: facts\nsummary: s\n";
        let (events, _) = feed(input, usize::MAX);
        assert!(
            has_event(
                &events,
                |e| matches!(e, ParseEvent::Error(err) if err.code == ErrorCode::P001)
            ),
            "expected ParseEvent::Error(P001) for missing version"
        );
    }

    #[test]
    fn test_pending_text_retains_partial_line() {
        let mut parser = StreamParser::new();

        // Feed "agm: 1" with no newline — no events yet (partial line).
        let events = parser.push_chunk("agm: 1");
        assert!(events.is_empty(), "no events for partial line");
        assert_eq!(parser.lines_consumed(), 0);

        // Feed ".0\n" — still no HeaderComplete (header not complete yet).
        let events = parser.push_chunk(".0\n");
        // One line consumed (agm: 1.0) — no HeaderComplete yet (no node seen).
        assert_eq!(parser.lines_consumed(), 1);
        assert!(
            !has_event(&events, |e| matches!(e, ParseEvent::HeaderComplete(_))),
            "HeaderComplete should not appear until a node boundary"
        );
    }

    #[test]
    fn test_bytes_consumed_matches_input_length() {
        let mut parser = StreamParser::new();
        let chunks = ["agm: 1.0\n", "package: test.pkg\n", "version: 0.1.0\n"];
        let total: usize = chunks.iter().map(|c| c.len()).sum();
        for chunk in &chunks {
            parser.push_chunk(chunk);
        }
        assert_eq!(parser.bytes_consumed(), total);
    }

    #[test]
    fn test_trailing_newline_added_on_finish_if_missing() {
        // Input ends mid-line — finish() must still emit NodeComplete.
        let input = "agm: 1.0\npackage: test.pkg\nversion: 0.1.0\n\nnode a.one\ntype: facts\nsummary: no trailing newline";
        let (events, result) = feed(input, usize::MAX);
        assert!(result.is_ok(), "expected Ok: {:?}", result);
        assert!(
            has_event(
                &events,
                |e| matches!(e, ParseEvent::NodeComplete(n) if n.id == "a.one")
            ),
            "NodeComplete for a.one not found"
        );
    }

    #[test]
    fn test_crlf_equivalent_to_lf() {
        // Build a CRLF version of the minimal valid file.
        let lf_input = minimal_valid();
        let crlf_input = lf_input.replace('\n', "\r\n");

        let (_, lf_result) = feed(lf_input, usize::MAX);
        let (_, crlf_result) = feed(&crlf_input, usize::MAX);

        match (lf_result, crlf_result) {
            (Ok(lf_file), Ok(crlf_file)) => {
                assert_eq!(
                    lf_file, crlf_file,
                    "CRLF and LF should produce identical AgmFile"
                );
            }
            (Err(lf_errs), Err(crlf_errs)) => {
                // Both error — compare error codes (order may differ).
                let mut lf_codes: Vec<_> = lf_errs.iter().map(|e| e.code).collect();
                let mut cr_codes: Vec<_> = crlf_errs.iter().map(|e| e.code).collect();
                lf_codes.sort_by_key(|c| format!("{c:?}"));
                cr_codes.sort_by_key(|c| format!("{c:?}"));
                assert_eq!(
                    lf_codes, cr_codes,
                    "CRLF and LF should have same error codes"
                );
            }
            _ => panic!("CRLF and LF disagree on Ok/Err"),
        }
    }
}
