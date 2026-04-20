# Changelog

All notable changes to `agm-cli` and `agm-core` are documented here.

## Unreleased -- targeting 1.3.0

### Added

- **`agm llm-bench` CLI subcommand**: runs a standardized LLM emission compliance
  suite against any Messages-style or Chat-Completions-style HTTP inference endpoint.
  Ships 12 built-in prompt fixtures (4 per node type: `ticket`, `workflow`,
  `orchestration`; 2 adversarial per type) plus 24 synthesized cassettes for
  fully-offline CI runs. Compliance buckets: `PASS`, `NORM_FIXABLE`, `SCHEMA_ONLY`,
  `VALIDATE_ONLY`, `FAIL`, `ERROR`. Reports: `--format text|markdown|json`.
  Concurrency via `--concurrency <N>` (scoped threads, no `rayon`).
  Cassette modes: `Replay` (default when `--cassettes` given), `Record` (with
  `--record`), `Disabled` (with `--live`). Cost estimation via opt-in
  `--cost-per-1k-in` / `--cost-per-1k-out` flags. No vendor names or API keys
  are hardcoded; endpoints and keys come entirely from env vars
  (`AGM_MESSAGES_ENDPOINT`, `AGM_MESSAGES_KEY`, `AGM_CHAT_ENDPOINT`,
  `AGM_CHAT_KEY`). Exit codes: 0 pass, 1 failures, 2 config error,
  3 missing credentials, 4 network unreachable. Docs: `docs/llm_bench.md`.

- **`agm corpus` CLI subcommand**: emits a cacheable, provider-aware AGM
  system-prompt corpus in three flavors (`full`, `standard`, `grammar-only`)
  targeting three provider formats (`anthropic`, `openai`, `vanilla`).
  Token budget enforcement via `--min-tokens` pads the output with examples
  from a built-in bank (12 validated AGM examples) until the estimated token
  count meets the target. Token estimation uses the `cl100k_base` BPE
  tokenizer (`tiktoken-rs`). Flags: `--flavor`, `--for`, `--min-tokens`,
  `--no-version`, `--output`, `--format text|json`, `--count-only`. Exit
  codes: 0 success, 1 bank exhausted, 2 I/O error. Docs: `docs/corpus.md`.

- **Normalize layer** (`agm_core::normalize`): rewrites non-canonical synonyms
  (type aliases like `plan_execution` -> `orchestration`, field renames like
  `depends_on` -> `requires` inside `parallel_groups`, `groups` / `phases` ->
  `parallel_groups`) to canonical form before validation. Ships with a
  built-in YAML rule set, supports custom rule sets via `--rules <PATH>`,
  and produces a per-rewrite report with span information.

- **`agm normalize` CLI subcommand**: `--output`, `--in-place`, `--explain`,
  `--report-format text|json`, `--no-types`, `--no-fields`, `--check`.

- **Schemas module** (`agm_core::schemas`): emits JSON Schema (Draft 2020-12)
  for every built-in node type, derived from the same registry the validator
  uses. Includes dialect wrappers for Anthropic tool-use and OpenAI
  function-calling, plus `--strict` (additionalProperties: false) and
  per-type or `all` emission modes.

- **`agm schema` CLI subcommand**: `--format json-schema|yaml`,
  `--for vanilla|anthropic-tool-use|openai-tool`, `--include-enums` /
  `--no-include-enums`, `--strict`, `--tool-name`, `--tool-description`,
  `--output`, `--pretty`. Per-type emission and `agm schema all --output <DIR>`
  for bulk generation.

- **Programmatic Builder API** (`agm_core::builder`): fluent constructors
  for spec-compliant nodes -- `TicketBuilder`, `WorkflowBuilder`,
  `OrchestrationBuilder`, `FactsBuilder`, `RulesBuilder`, `DecisionBuilder`,
  `MemoryEntryBuilder`, plus helper builders `CodeBlockBuilder` and
  `VerifyCheckBuilder`. Calling `.build()` runs the same validation passes
  as `agm validate` (`ValidationScope::SingleNode`) and returns
  `Result<Node, BuildError>`. All node builders expose universal relation
  setters (`depends`, `related_to`, `replaces`, `conflicts`, `see_also`)
  and `tags`. List setters deduplicate entries while preserving
  first-appearance order.

- **`Node::render_canonical()` and `Node::render_node_only()`**: convenience
  methods to serialize a single node to canonical AGM text (with or without
  the placeholder header).

- **`ValidationScope`** on `ValidateOptions` (`File` | `SingleNode`,
  default `File`). `SingleNode` skips cross-node reference checks (V004
  `load_nodes`, V009 `verify: node_status`, V005 cycles, V013/V018/V019
  cross-node compatibility) so isolated nodes can be validated without
  spurious errors. CLI commands default to `File` -- behaviour unchanged.

- **`NODE_ID_PATTERN`** constant in `agm_core::model::fields`, shared by
  parser (P002) and validator (V021) to eliminate regex drift.

- **Explicit YAML block-scalar indent indicator** (`body: |2`) emitted by
  the canonical renderer for code-block bodies, preserving leading
  whitespace inside body content across parse / render round-trips. Lexer
  and parsers honour the indicator on input; legacy files without it
  continue to parse via the existing inference path.

### Fixed

- `CodeBlockBuilder::replace().anchor(...)` now returns `BuildError::Precondition`
  immediately. `anchor` is reserved for `insert_before` / `insert_after`
  per spec section 23.4; only `old` is a valid discriminator for
  `replace`. Previously the builder accepted it and the validator rejected
  the constructed node with V008.

- Code-block body round-trips now preserve leading whitespace in body
  content (e.g. indented struct fields, docstring asterisks). Achieved via
  the explicit block-scalar indent indicator on emission and matching
  strip semantics on parse.

- **`agm ingest` CLI subcommand** and **`agm_core::ingest`** module: converts
  tool-call JSON args (single object or array) into validated canonical AGM
  text. Pipeline: JSON Schema pre-check → field-name normalization → builder
  construction (`build_unchecked`) → post-build Standard/Strict validation →
  canonical render. Supports `--no-normalize`, `--no-schema-check`,
  `--enforcement strict|standard|permissive`, `--output`, `--version`,
  `--header-title`. Batch mode synthesizes IDs as `{prefix}.{i}` when
  individual elements carry no `"node"` field.

- **`extra()` escape hatch** on all 7 node builders (`TicketBuilder`,
  `WorkflowBuilder`, `OrchestrationBuilder`, `FactsBuilder`, `RulesBuilder`,
  `DecisionBuilder`, `MemoryEntryBuilder`): routes unknown JSON fields into
  `node.extra_fields` so model-emitted non-spec fields are preserved for
  audit rather than discarded.

- **`extra_fields`** on `MemoryEntry`: `BTreeMap<String, FieldValue>` with
  `#[serde(flatten)]` — unknown fields in memory entries round-trip through
  serialization.

- **`MemoryEntryBuilder::extra()`** setter for unknown memory-entry fields.

- **`.agm.mem` First-Class SDK** (`agm_core::memory::store`): `FilesystemMemoryStore`
  with HMAC-SHA256 signing, atomic writes, and three merge strategies.
  New public types: `MemoryStore` trait, `FilesystemConfig`, `FilesystemMemoryStore`,
  `SigningMode` (Disabled / Enabled / EnabledWithRotation), `VerifyMode`
  (Permissive / IfPresent / Strict), `SignatureEnvelope` (TrailingComment / SidecarFile),
  `MergeStrategy` (LatestWins / Union / Reject), `MergeOutcome`, `MemoryStoreError`.
  Key resolution supports `env:VAR`, `file:/path`, `hex:<literal>`, and `generate`
  (generates a random key and prints it to stderr). Signature covers the full
  canonical `render_mem` output; verification uses constant-time comparison.
  Writes are atomic via `NamedTempFile::persist`. New dependency: `hmac = "0.12"`,
  `sha2 = "0.10"`, `hex = "0.4"`, `getrandom = "0.2"`, `tempfile = "3"`.
  Optional `zeroize` feature (default-on) zeroes key material on drop.

- **`agm mem sign` CLI subcommand**: sign a `.agm.mem` file in-place.
  Flags: `--key <KEY_SPEC>`, `--envelope trailing-comment|sidecar-file`.

- **`agm mem verify` CLI subcommand**: verify HMAC-SHA256 integrity of a
  `.agm.mem` file. Exit codes: 0=valid, 1=tampered, 2=missing+strict, 3=error.
  Flags: `--key <KEY_SPEC>`, `--envelope`, `--verify-mode permissive|if-present|strict`.

- **`agm mem import`** extended: `--strategy latest-wins|union|reject` merge
  strategy flag; `--sign <KEY_SPEC>` to sign the merged output; `--envelope`.

- **`agm mem export`** extended: `--sign <KEY_SPEC>` and `--envelope` flags.

### Documentation

- New: `docs/normalize.md`, `docs/schemas.md`, `docs/builder.md`, `docs/ingest.md`,
  `docs/memory_sdk.md`.
- README extended with sections for normalize, schemas, Builder API, `agm ingest`,
  and "Signing `.agm.mem` Sidecars".
- Rustdoc on `validator::node::NODE_ID_RE` clarifies V021 as defence-in-depth:
  the parser's P002 catches invalid IDs at parse time; V021 still fires
  when a `Node` is constructed programmatically (e.g. via `serde_json::from_str`,
  direct struct construction, or `build_unchecked`) and bypasses the parser.
- Spec errata (section 11.2): node-ID pattern now documents `[a-z0-9_]`
  per segment, aligning the spec with the long-standing parser/validator
  behaviour.
- `AgmFile` and `renderer::json::render_json` Rustdoc documents the flat
  JSON output shape (header fields at the top level via
  `#[serde(flatten)]`) as the stable public contract.
