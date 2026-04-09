# AGM CLI

A command-line tool and Rust library for parsing, validating, loading, rendering, and orchestrating [AGM (Agent Graph Memory)](docs/spec/agm_spec_v1.0.0.md) files.

## What is AGM?

AGM is a compact, line-oriented text format for representing knowledge as a directed graph. It is designed for AI agent workflows: each node encodes a unit of knowledge (facts, rules, workflows, decisions, etc.) with explicit dependencies, verification contracts, and execution semantics. AGM files can be statically analyzed or orchestrated as runnable graphs.

## Installation

### From crates.io

```bash
cargo install agm-cli
```

### Pre-built binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/JAAvila-Of/agm-cli/releases).

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
```

### From source

```bash
git clone https://github.com/JAAvila-Of/agm-cli.git
cd agm-cli
cargo build --release
# Binary at target/release/agm (or agm.exe on Windows)
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

## Documentation

- [CLI API Reference](docs/api.md) -- Complete command reference with examples, options, and edge cases
- [AGM Specification v1.0.0](docs/spec/agm_spec_v1.0.0.md) -- Full format specification
- [Library API (docs.rs)](https://docs.rs/agm-core) -- Auto-generated Rust API docs for `agm-core`
- [Contributing](CONTRIBUTING.md) -- How to contribute

## Self-Update

Update to the latest version directly from the CLI:

```bash
# Check for updates
agm update --check

# Update to latest
agm update
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, testing, and PR guidelines.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

Copyright 2025-2026 Jose Angel Avila.
