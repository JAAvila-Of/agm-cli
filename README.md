<p align="center">
  <img src="logo.png" alt="AGM CLI logo" width="200" />
</p>

<h1 align="center">AGM CLI</h1>

<p align="center">
  A command-line tool and Rust library for parsing, validating, loading, rendering, and orchestrating <a href="docs/spec/agm_spec_v1.2.0.md">AGM (Agent Graph Memory)</a> files.
</p>

## Origin

`agm-cli` was originally built as an internal tool for the **Octopus**
project, where it served as the backbone for orchestrating AI-agent
workflows over structured knowledge graphs. It has since been extracted
and released as a standalone, general-purpose CLI + library so that any
project can adopt the AGM format and its execution model.

## What is AGM?

AGM is a compact, line-oriented text format for representing knowledge as a directed graph. It is designed for AI agent workflows: each node encodes a unit of knowledge (facts, rules, workflows, decisions, etc.) with explicit dependencies, verification contracts, and execution semantics. AGM files can be statically analyzed or orchestrated as runnable graphs.

## Installation

`agm-cli` is distributed exclusively through [crates.io](https://crates.io/crates/agm-cli).
A working [Rust toolchain](https://rustup.rs) (stable) is required.

```bash
cargo install agm-cli
```

Pin a specific version:

```bash
cargo install agm-cli --version 1.0.0
```

Upgrade to the latest published version:

```bash
cargo install agm-cli --force
```

### From source

```bash
git clone https://github.com/JAAvila-Of/agm-cli.git
cd agm-cli
cargo install --path crates/agm-cli
```

### Uninstall

```bash
cargo uninstall agm-cli
```

## Quick Start

```bash
# Validate an AGM file against the spec
agm validate myfile.agm

# Run extended quality checks
agm lint myfile.agm

# Render to Markdown
agm render myfile.agm --format markdown

# View the dependency graph (DOT format)
agm graph myfile.agm --format dot

# Load nodes at summary level (JSON output)
agm load myfile.agm --mode summary
```

### Cacheable system prompt

Use `agm corpus` to generate a ready-made AGM format reference for an
agent's system prompt. The `--for anthropic` target prepends a role preamble
and the `--min-tokens` flag ensures the output exceeds the provider's
prompt-cache minimum:

```bash
# Generate a full corpus for Anthropic with at least 2048 tokens
agm corpus --for anthropic --min-tokens 2048 > system_prompt.md

# Check the token count
agm corpus --for anthropic --count-only
# 2509

# JSON output with metadata
agm corpus --for anthropic --format json | jq '{tokens: .estimated_tokens}'
```

Pass the content of `system_prompt.md` as the system block in your API
request and set `cache_control: { type: ephemeral }` on that block to enable
prompt caching. See [docs/corpus.md](docs/corpus.md) for the full reference.

### Ticket node example (AGM v1.2)

```agm
agm: 1.2
package: my.project
version: 1.0.0

node auth.ticket.add-login
type: ticket
title: Add OAuth2 login endpoint
description: Implement /login with Google OAuth2 and JWT token issuance.
priority: high
action: create
sdd_phase: propose
labels: [auth, api]
summary: add OAuth2 login endpoint
```

## Commands

| Command | Description |
|---------|-------------|
| `validate` | Validate an AGM file against the specification |
| `lint` | Run extended quality checks (validate + heuristics) |
| `load` | Load nodes at a specific expansion level (JSON output) |
| `render` | Render to another format (JSON, Markdown, DOT, Mermaid, canonical AGM) |
| `graph` | Output the dependency graph (DOT or Mermaid) |
| `run` | Execute nodes in dependency order via an agent backend |
| `status` | Show execution status of nodes |
| `retry` | Retry failed node executions |
| `state` | Manage execution state (list, get, export, import, reset) |
| `mem` | Manage memory sidecars (list, get, export, import, gc) |
| `context` | Build and display agent context for a node |
| `verify` | Run verification checks on nodes |
| `normalize` | Normalize non-canonical synonyms to canonical AGM field and type names |
| `schema` | Emit JSON Schema (Draft 2020-12) for a built-in AGM node type |
| `corpus` | Emit a cacheable, provider-aware AGM system-prompt corpus |
| `update` | Update agm to the latest version |

## Library Usage

The `agm-core` crate provides the parsing, validation, loading, graph, and rendering engine as a library:

```rust
use agm_core::parser::parse;
use agm_core::validator::validate;
use agm_core::renderer::{render, RenderFormat};

let source = std::fs::read_to_string("myfile.agm").unwrap();
let file = parse(&source).expect("parse error");
let diagnostics = validate(&file);

if diagnostics.iter().all(|d| !d.is_error()) {
    let output = render(&file, RenderFormat::Json);
    println!("{output}");
}
```

### Builder API

`agm-core` ships a fluent builder API for constructing spec-compliant
nodes from Rust. Builders run the same validation as `agm validate`.

```rust
use agm_core::builder::{TicketBuilder, WorkflowBuilder, CodeBlockBuilder, VerifyCheckBuilder};
use agm_core::model::fields::Priority;
use agm_core::model::ticket::{TicketAction, SddPhase};
use agm_core::model::code::CodeAction;

// Build a ticket node
let ticket = TicketBuilder::new("my.ticket.oauth-login")
    .summary("add OAuth2 login endpoint")
    .title("Add OAuth2 login endpoint")
    .description("Implement /login with Google OAuth2 and JWT issuance.")
    .priority(Priority::High)
    .action(TicketAction::Create)
    .sdd_phase(SddPhase::Propose)
    .labels(["auth", "api"])
    .build()?;

// Render the node as canonical AGM text (no header)
println!("{}", ticket.render_node_only());

// Build a code-block patch and a verify check
let patch = CodeBlockBuilder::replace()
    .target("src/auth/mod.rs")
    .lang("rust")
    .old("// TODO: auth")
    .body("pub mod oauth;")
    .build()?;

let check = VerifyCheckBuilder::command("cargo test --lib")
    .expect("exit_code_0")
    .build()?;

// Build a workflow that carries the patch and the verify check
let workflow = WorkflowBuilder::new("auth.workflow.add-oauth")
    .summary("implement OAuth2 login")
    .steps(["scaffold module", "add route handler", "write tests"])
    .code_blocks([patch])
    .verify([check])
    .build()?;
```

See [docs/builder.md](docs/builder.md) for the full builder reference.

## Generate tool-use schemas

`agm schema` generates JSON Schema (Draft 2020-12) for any built-in node type, optionally
wrapped for Anthropic tool-use or OpenAI function-calling. Schemas are derived from the same
registry the validator uses, so they always stay in sync with the spec.

```bash
# Raw JSON Schema for the ticket node type
agm schema ticket

# Anthropic tool-use shape (drop into your tools array)
agm schema ticket --for anthropic-tool-use --tool-name create_ticket

# OpenAI function-calling shape
agm schema ticket --for openai-tool

# Strict mode: additionalProperties: false
agm schema ticket --strict

# Generate all 11 built-in type schemas into a directory
agm schema all --output schemas/

# Generate all schemas wrapped for Anthropic tool-use
agm schema all --for anthropic-tool-use --output schemas/
```

Rust API:

```rust
use agm_core::schemas::{ticket_schema, schema_for, SchemaOptions, SchemaDialect};
use agm_core::model::fields::NodeType;

// Convenience function (vanilla, default options)
let schema = ticket_schema();

// Full control
let opts = SchemaOptions {
    dialect: SchemaDialect::AnthropicToolUse,
    strict: true,
    ..Default::default()
};
let schema = schema_for(&NodeType::Ticket, &opts).unwrap();
```

See [docs/schemas.md](docs/schemas.md) for the full reference.

## `agm ingest` — Tool-Call JSON to Canonical AGM

`agm ingest` is the shortest path from a completed LLM tool-call to a
persisted canonical `.agm` node.

```bash
# Single node from stdin
echo '{
  "type": "ticket",
  "summary": "add OAuth2 login",
  "title": "Add OAuth2 Login",
  "description": "Implement the OAuth2 PKCE flow.",
  "priority": "high"
}' | agm ingest ticket --package myproject.tickets --id myproject.ticket.oauth

# From a file
agm ingest ticket --package myproject.tickets --id myproject.ticket.oauth \
    --file args.json

# Batch (JSON array) — nodes are id'd by "node" field or synthesized prefix
cat batch.json | agm ingest ticket --package myproject.tickets --id myproject.batch
```

Rust API:

```rust
use agm_core::ingest::{ingest_one, IngestConfig};
use agm_core::model::fields::NodeType;
use agm_core::model::schema::EnforcementLevel;
use serde_json::json;

let v = json!({
    "type": "ticket",
    "summary": "add login",
    "title": "Add Login",
    "description": "Implement login.",
    "priority": "high"
});

let node = ingest_one(
    NodeType::Ticket,
    "myproject.ticket.login",
    v,
    &IngestConfig::default(),
).unwrap();
```

See [docs/ingest.md](docs/ingest.md) for the full reference, including
Anthropic and OpenAI tool-call integration examples.

## Documentation

- [CLI API Reference](docs/api.md) -- Complete command reference with examples, options, and edge cases
- [AGM Specification v1.2.0](docs/spec/agm_spec_v1.2.0.md) -- Full format specification
- [Normalize Layer](docs/normalize.md) -- How to rewrite non-canonical AGM input to canonical form
- [Builder API](docs/builder.md) -- Fluent Rust builder API for constructing AGM nodes programmatically
- [JSON Schema Generation](docs/schemas.md) -- How to generate and use node-type schemas
- [Ingest](docs/ingest.md) -- Convert tool-call JSON args into canonical AGM text
- [Library API (docs.rs)](https://docs.rs/agm-core) -- Auto-generated Rust API docs for `agm-core`
- [Contributing](CONTRIBUTING.md) -- How to contribute

## Updating

New versions are published to crates.io. To upgrade:

```bash
cargo install agm-cli --force
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, testing, and PR guidelines.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

Copyright 2025-2026 Jose Angel Avila.
