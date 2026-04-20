# Changelog

All notable changes to `agm-cli` and `agm-core` are documented here.

## Unreleased -- targeting 1.3.0

### Added

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

### Documentation

- New: `docs/normalize.md`, `docs/schemas.md`, `docs/builder.md`.
- README extended with sections for normalize, schemas, and the Builder API.
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
