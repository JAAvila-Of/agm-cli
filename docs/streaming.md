# Streaming Parser (`StreamParser`)

`agm_core::parser::stream` provides a push-based, chunk-oriented AGM parser for
consumers that receive AGM text incrementally — for example, from a streaming
model response — and want to render nodes to the user before the full document
has arrived.

---

## When to use streaming vs batch

| Situation | Recommendation |
|-----------|---------------|
| Full AGM text available in memory | `parser::parse` (batch) |
| Small files (< ~10 nodes) | `parser::parse` — overhead is negligible |
| Receiving text chunk-by-chunk (e.g. from a streaming LLM response) | `StreamParser` |
| Displaying nodes incrementally as an agent's model output arrives | `StreamParser` |
| Validating a file on disk | `parser::parse` followed by `validator::validate` |

The batch parser is simpler and slightly faster for small inputs. Use streaming
only when perceived latency of the first rendered node matters.

---

## Public API

```rust
use agm_core::parser::{StreamParser, ParseEvent};
use agm_core::model::file::AgmFile;
use agm_core::error::AgmError;

// 1. Create a parser.
let mut parser = StreamParser::new();

// 2. Feed chunks as they arrive.
let events: Vec<ParseEvent> = parser.push_chunk("agm: 1.0\npackage: my.pkg\nversion: 1.0.0\n");
let more_events = parser.push_chunk("\nnode auth.login\ntype: workflow\nsummary: Login flow\n");

// 3. Signal end-of-stream.
let (final_events, result) = parser.finish();

// 4. Inspect the result.
match result {
    Ok(file) => println!("Parsed {} nodes", file.nodes.len()),
    Err(errors) => eprintln!("{} hard errors", errors.len()),
}
```

### `push_chunk(&mut self, chunk: &str) -> Vec<ParseEvent>`

Feeds a chunk of AGM text. Returns any events completed by that chunk. The
chunk may end mid-line or mid-node; incomplete state is buffered automatically.
Chunks may be of any size — a single byte is valid.

### `finish(self) -> (Vec<ParseEvent>, Result<AgmFile, Vec<AgmError>>)`

Signals end-of-stream. Flushes any buffered partial line and the final node,
emits `EndOfFile`, and assembles the complete `AgmFile`.

Returns `Err(errors)` if any hard parse error was seen (same criterion as
`parser::parse`). The events vector always contains `EndOfFile` regardless of
whether the result is `Ok` or `Err`.

### Accessors

```rust
parser.bytes_consumed() -> usize   // total bytes fed via push_chunk
parser.lines_consumed() -> usize   // complete lines processed so far
parser.header_complete() -> bool   // true after HeaderComplete was emitted
```

---

## Event contract

```rust
#[non_exhaustive]
pub enum ParseEvent {
    HeaderComplete(Box<Header>),
    NodeStarted { id: String, line: usize },
    NodeComplete(Box<Node>),
    Warning(AgmError),
    Error(AgmError),
    EndOfFile,
}
```

### `HeaderComplete(header)`

Emitted **once**, when the first `node` line is encountered and the preceding
lines are flushed through `parse_header`. The boxed `Header` is the same value
that will appear in `AgmFile.header` (barring subsequent errors that produce an
empty-string fallback).

If the input contains no nodes at all (header-only or empty file), this event
is **not** emitted. The header is still parsed internally.

### `NodeStarted { id, line }`

An **advisory** early signal. Emitted as soon as a `node <id>` line is
consumed. The node is not yet fully parsed. Do not use this for authoritative
node data — wait for `NodeComplete`.

Useful for: displaying a placeholder or progress indicator immediately.

### `NodeComplete(node)`

The node is fully parsed. `node.id` matches the `id` from the preceding
`NodeStarted` event. The boxed `Node` is `PartialEq`-comparable and fully
populated.

### `Warning(err)` / `Error(err)`

Parse-time diagnostics. `Error` events correspond to hard errors
(`err.is_error() == true`). `Warning` events are advisory. Both are
also accumulated in the error list returned by `finish`.

### `EndOfFile`

The last event, always emitted by `finish`. Signals that no more events will
be produced.

---

## Ordering guarantees

1. `HeaderComplete` (if emitted) precedes all `NodeStarted` and `NodeComplete`
   events.
2. For each node `X`: `NodeStarted { id: X }` strictly precedes the matching
   `NodeComplete` where `node.id == X`.
3. `EndOfFile` is the last event in the stream.
4. `Error`/`Warning` events may appear between any other events, at the point
   where the error was detected.

---

## CRLF handling

`StreamParser` strips a single trailing `\r` from each line before
classification. This mirrors the behavior of `str::lines()` used by the batch
parser. CRLF and LF inputs produce equivalent `AgmFile` output.

---

## Error handling and equivalence with batch

For any input, `StreamParser` produces the same `Ok(AgmFile)` or `Err(Vec<AgmError>)`
as `parser::parse(whole_input)`. The error *set* is identical (same error codes
and line numbers); the ordering may differ because the stream parser emits
errors per-node boundary while the batch parser collects all errors before
returning.

Missing required header fields (P001) still appear as `ParseEvent::Error`
events.

An empty file (no nodes) produces a `ParseEvent::Error(P008)` and
`finish` returns `Err`.

---

## Code example: rendering nodes incrementally

```rust
use agm_core::parser::{StreamParser, ParseEvent};

fn render_incremental(chunks: impl Iterator<Item = String>) {
    let mut parser = StreamParser::new();

    for chunk in chunks {
        for event in parser.push_chunk(&chunk) {
            match event {
                ParseEvent::HeaderComplete(h) => {
                    println!("Package: {} v{}", h.package, h.version);
                }
                ParseEvent::NodeStarted { id, .. } => {
                    println!("  [loading {}...]", id);
                }
                ParseEvent::NodeComplete(node) => {
                    println!("  {} — {}", node.id, node.summary);
                }
                ParseEvent::Error(err) => {
                    eprintln!("  parse error: {err}");
                }
                ParseEvent::Warning(warn) => {
                    eprintln!("  warning: {warn}");
                }
                ParseEvent::EndOfFile => {}
                // `#[non_exhaustive]` — handle future variants:
                _ => {}
            }
        }
    }

    let (_, result) = parser.finish();
    match result {
        Ok(file) => println!("Done — {} nodes total", file.nodes.len()),
        Err(errors) => eprintln!("Parse failed: {} errors", errors.len()),
    }
}
```

---

## Gotchas

- **`HeaderComplete` is not emitted for header-only or empty files.** The
  header is parsed internally (so the `AgmFile` header fields are populated),
  but the event is only emitted when the parser transitions from header to the
  first node. If you need to detect a header in all cases, inspect the
  `AgmFile` returned by `finish()`.

- **The final node is only flushed on `finish()`.** Always call `finish()` to
  get the last `NodeComplete` and the complete `AgmFile`.

- **`NodeStarted` is advisory.** The id is extracted from the `node <id>` line
  but the node body has not been parsed yet. Validation results are only
  available after `NodeComplete`.

- **Synchronous only.** The API is fully sync. To use in an async context, wrap
  `push_chunk` calls in `spawn_blocking` or a thread-pool executor. The parser
  is `Send` (it contains only `String`, `Vec`, and primitive types).

- **`finish()` takes `self` by value.** A finished parser cannot be reused.
  Construct a new `StreamParser::new()` for each document.

- **`#[non_exhaustive]` on `ParseEvent`.** Match arms must include a wildcard
  (`_ => {}`) to remain compatible with future variants such as `Comment`.

---

## Performance notes

`StreamParser` is slightly slower than `parser::parse` for small inputs (< ~10
nodes) due to per-line buffering overhead. For large documents received
incrementally, the latency benefit of showing the first node earlier typically
outweighs this cost. Both parsers reuse the same `lexer`, `header`, and `node`
submodules — no grammar logic is duplicated.
