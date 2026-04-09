# AGM CLI -- API Reference

Complete reference for the `agm` command-line tool and the `agm-core` library.

---

## Table of Contents

- [Exit Codes](#exit-codes)
- [Global Options](#global-options)
- [Commands](#commands)
  - [validate](#agm-validate)
  - [lint](#agm-lint)
  - [load](#agm-load)
  - [render](#agm-render)
  - [graph](#agm-graph)
  - [run](#agm-run)
  - [status](#agm-status)
  - [retry](#agm-retry)
  - [state](#agm-state)
  - [mem](#agm-mem)
  - [context](#agm-context)
  - [verify](#agm-verify)
  - [update](#agm-update)
- [Concepts](#concepts)
  - [Load Modes](#load-modes)
  - [Render Formats](#render-formats)
  - [Enforcement Levels](#enforcement-levels)
  - [Error Output Formats](#error-output-formats)
  - [State Sidecar Files](#state-sidecar-files)
  - [Memory Sidecar Files](#memory-sidecar-files)
- [Library API (agm-core)](#library-api-agm-core)

---

## Exit Codes

All commands follow a consistent exit code convention:

| Code | Meaning |
|------|---------|
| `0` | Success (including warnings-only) |
| `1` | Validation errors, node failures, or invalid arguments |
| `2` | I/O error, file not found, or no state available |

---

## Global Options

```
agm [OPTIONS] <COMMAND>
```

| Flag | Description |
|------|-------------|
| `--version` | Print version and exit |
| `--help` | Print help and exit |

---

## Commands

### `agm validate`

Validate an AGM file against the specification. Reports parse errors and validation diagnostics.

```
agm validate <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Values | Default | Description |
|--------|--------|---------|-------------|
| `--enforcement` | `strict`, `standard`, `permissive` | `standard` | Type-schema enforcement level |
| `--errors-format` | `text`, `json` | `text` | Diagnostic output format |

**Output:**
- **text mode**: Diagnostics go to stderr with source context (miette rich output). On clean validation: `"<file>: OK"` to stderr.
- **json mode**: JSON array of diagnostic objects to stdout. Each object contains `code`, `severity`, `message`, and optionally `file`, `line`, `node`.

**Examples:**

```bash
# Basic validation
agm validate myfile.agm

# Strict enforcement with JSON output
agm validate myfile.agm --enforcement strict --errors-format json

# Permissive mode (fewer type errors)
agm validate myfile.agm --enforcement permissive
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | File is valid (or warnings-only) |
| `1` | Parse error or validation errors |
| `2` | File not found or I/O error |

**Edge cases:**
- Empty files produce parse error `AGM-P008`, exit `1`.
- Warnings (non-error diagnostics) are printed but do not cause a non-zero exit.
- Imports are not resolved during validation (`import_resolver: None`).

---

### `agm lint`

Run full spec validation plus additional quality heuristics. Superset of `validate`.

```
agm lint <FILE> [OPTIONS]
```

**Arguments and options:** Same as `validate`.

**Additional lint checks (severity: Info):**

| Code | Trigger | Description |
|------|---------|-------------|
| `L001` | Summary > 100 chars | `"Summary of node '<id>' is N chars; consider shortening"` |
| `L002` | Duplicate summaries | `"Nodes '<id1>' and '<id2>' have identical summaries"` |
| `L003` | Node has > 15 optional fields | `"Node '<id>' has N populated fields; consider splitting"` |

**Output:** Same as `validate`, with additional lint diagnostics appended. On clean: `"<file>: OK (lint)"`.

**Examples:**

```bash
agm lint myfile.agm
agm lint myfile.agm --errors-format json
```

---

### `agm load`

Load nodes at a specific expansion level. Output is always JSON to stdout.

```
agm load <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--mode` | `summary`, `operational`, `executable`, `full` | `summary` | Load mode (field expansion level) |
| `--profile` | string | none | Named load profile from file header. Overrides `--mode` |
| `--nodes` | string | none | Comma-separated node IDs to include |

**Priority:** `--profile` > `--nodes` > default (all nodes).

**Examples:**

```bash
# Load all nodes at summary level
agm load myfile.agm

# Load at full detail
agm load myfile.agm --mode full

# Load specific nodes at executable level
agm load myfile.agm --mode executable --nodes "auth_setup,db_init"

# Use a named load profile defined in the file header
agm load myfile.agm --profile production

# Built-in debug profile: failed/blocked nodes + their deps at executable level
agm load myfile.agm --profile debug
```

**Load profile filter language:**

Profiles are defined in the AGM file header under `load_profiles:`. Each profile has a filter expression composed of AND-joined clauses:

| Clause | Example | Description |
|--------|---------|-------------|
| `*` | `*` | Matches all nodes |
| `priority in [...]` | `priority in [critical, high]` | Match by priority |
| `type in [...]` | `type in [workflow, rules]` | Match by node type |
| `execution_status in [...]` | `execution_status in [failed, blocked]` | Match by status |
| `tags in [...]` | `tags in [auth, security]` | Match if any tag present |
| `code is present` | `code is present` | Match nodes with code blocks |

**Exit codes:**

| Code | When |
|------|------|
| `0` | Success |
| `2` | File error, invalid mode, unknown profile, or parse error |

**Edge cases:**
- Invalid `--mode` value: `"error: invalid load mode: \"<value>\"; expected one of: summary, operational, executable, full"`, exit `2`.
- `--profile` with no `load_profiles` in header: `"error: no load profiles defined in file header"`, exit `2`.
- Unknown profile name: `"error: unknown load profile: \"<name>\""`, exit `2`.
- `--nodes` accepts whitespace around commas: `" node1 , node2 "` works.
- Unrecognized filter clauses in profiles emit a warning to stderr but are treated as always-true.

---

### `agm render`

Render an AGM file to another format.

```
agm render <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Values | Default | Description |
|--------|--------|---------|-------------|
| `--format` | `json`, `json-canonical`, `markdown`, `agm`, `dot`, `mermaid` | `markdown` | Output format |

**Examples:**

```bash
# Render as Markdown (default)
agm render myfile.agm

# JSON output
agm render myfile.agm --format json

# Canonical JSON per spec section S37
agm render myfile.agm --format json-canonical

# Round-trippable canonical AGM text
agm render myfile.agm --format agm

# Graphviz DOT graph
agm render myfile.agm --format dot

# Mermaid diagram
agm render myfile.agm --format mermaid

# Pipe to a file
agm render myfile.agm --format dot > graph.dot
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | Success |
| `1` | Parse error |
| `2` | File not found |

---

### `agm graph`

Output the dependency graph or topological order.

```
agm graph <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Values | Default | Description |
|--------|--------|---------|-------------|
| `--format` | `dot`, `mermaid` | `dot` | Graph output format (ignored with `--topo`) |
| `--topo` | flag | `false` | Print topological order instead of graph |

**Examples:**

```bash
# DOT graph (pipe to Graphviz)
agm graph myfile.agm | dot -Tpng -o graph.png

# Mermaid diagram
agm graph myfile.agm --format mermaid

# Topological sort (one node ID per line)
agm graph myfile.agm --topo
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | Success |
| `1` | Cycle detected (with `--topo`), or parse/validation error |
| `2` | File not found |

**Edge cases:**
- `--topo` with a cyclic graph: prints cycle error to stderr, exits `1`.
- `--format` is ignored when `--topo` is set.

---

### `agm run`

Execute nodes in dependency order via a shell agent backend.

```
agm run <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--node` | string | none | Execute only this node (and its unsatisfied deps) |
| `--group` | string | none | Execute only this orchestration group |
| `--dry-run` | flag | `false` | Print execution plan without running |
| `--concurrency` | integer | `1` | Max concurrent node executions |
| `--timeout` | seconds | `300` | Per-node timeout |
| `--agent` | string | `"shell"` | Agent backend (currently only `shell` is supported) |
| `--working-dir` | path | current dir | Working directory for file operations |
| `--fail-fast` | flag | `false` | Stop on first node failure |

**Execution modes:**
- **Default (topological):** Nodes execute in dependency order. Independent nodes can run in parallel up to `--concurrency`.
- **Orchestrated (`--group`):** Follows `parallel_groups` defined in an orchestration node. Respects group `strategy` (Sequential/Parallel), `requires` ordering, and group-level `max_concurrency`.

**Examples:**

```bash
# Execute all nodes sequentially
agm run workflow.agm

# Dry run to see execution plan
agm run workflow.agm --dry-run

# Parallel execution with 4 workers
agm run workflow.agm --concurrency 4

# Execute only a specific node (and its deps)
agm run workflow.agm --node setup_db

# Execute an orchestration group
agm run workflow.agm --group deploy_pipeline

# Fast-fail mode with 60s timeout
agm run workflow.agm --fail-fast --timeout 60

# Custom working directory
agm run workflow.agm --working-dir /tmp/workspace
```

**Output (dry-run):**
```
Dry run: would execute N nodes in order:
  1. node_id_1
  2. node_id_2
  ...
```

**Output (live run):**
```
Run complete: 5 executed, 4 succeeded, 1 failed, 0 skipped, 0 blocked (12.3s)
  [ok]   setup_db (2.1s, shell)
  [ok]   init_config (0.5s, shell)
  [ok]   run_migrations (3.2s, shell)
  [FAIL] deploy_app (6.5s, shell)
  [--]   run_tests (0.0s, shell)
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | All nodes succeeded (zero failed, zero blocked) |
| `1` | Any node failed or blocked; `--group` with no orchestration node; scheduler error |
| `2` | File not found, parse/validation error, or runtime init failure |

**Edge cases:**
- `--group` requires an `orchestration` type node in the file. If none exists: `"error: --group requires an orchestration node in the file"`, exit `1`.
- `--agent` is accepted but currently ignored -- only the `shell` backend is implemented.
- State is persisted to a `.agm.state` sidecar file after execution.
- On `--fail-fast`, remaining nodes are left in their current state (not marked as blocked).

---

### `agm status`

Show execution status of nodes by reading the state sidecar.

```
agm status <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--json` | flag | `false` | Output as JSON |
| `--node` | string | none | Show status for a single node |

**Examples:**

```bash
# Show status table for all nodes
agm status workflow.agm

# Single node status
agm status workflow.agm --node setup_db

# JSON output (pipe to jq)
agm status workflow.agm --json
agm status workflow.agm --json | jq '.nodes'
```

**Output (all nodes, text):**
```
Status: 3/5 completed (60%)

Node                      Status         Agent          Time
----                      ------         -----          ----
setup_db                  completed      shell          --
init_config               completed      shell          --
run_migrations            completed      shell          --
deploy_app                failed         shell          --
run_tests                 blocked        --             --
```

**Output (single node, text):**
```
Node: setup_db
Status: completed
Executed by: shell
Executed at: 2026-04-08T15:30:00Z
Retry count: 0
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | All nodes completed or skipped |
| `1` | Any node failed or blocked; or node not found |
| `2` | No state file found, state load error, or execution still in progress |

**Edge cases:**
- No state sidecar file: `"No execution state found for <file>"`, exit `2`.
- `--node` with unknown ID: `"error: node '<id>' not found in state"`, exit `1`.

---

### `agm retry`

Retry failed node(s). Transitions `Failed -> Ready`, then re-executes.

```
agm retry <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--node` | string | none | Node ID to retry |
| `--all-failed` | flag | `false` | Retry all failed nodes |
| `--working-dir` | path | current dir | Working directory |
| `--timeout` | seconds | `300` | Per-node timeout |

One of `--node` or `--all-failed` must be specified.

**Examples:**

```bash
# Retry a specific failed node
agm retry workflow.agm --node deploy_app

# Retry all failed nodes
agm retry workflow.agm --all-failed

# With custom timeout
agm retry workflow.agm --all-failed --timeout 600
```

**Output:**
```
Retry complete: 1 executed, 1 succeeded, 0 failed (3.2s)
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | All retried nodes succeeded; or no failed nodes found |
| `1` | Any retried node failed; node not in `failed` state; node not found |
| `2` | File error or state save failure |

**Edge cases:**
- `--node` on a non-failed node: `"error: node '<id>' is in state '<status>', not 'failed'"`, exit `1`.
- `--all-failed` with no failed nodes: `"No failed nodes to retry."`, exit `0`.
- Neither flag: `"error: specify --node <id> or --all-failed"`, exit `1`.
- Retries always run with `concurrency: 1` and `fail_fast: false`.

---

### `agm state`

Manage execution state sidecars. Has five subcommands.

#### `agm state list`

List all node states.

```
agm state list <FILE> [--json]
```

**Output (text):** One line per node: `"<id>: <status>"`
**Output (JSON):** Pretty-printed `StateFile` object.

```bash
agm state list workflow.agm
agm state list workflow.agm --json
```

#### `agm state get`

Get state for a specific node.

```
agm state get <FILE> --node <ID>
```

**Output:**
```
Node: setup_db
Status: completed
Executed by: shell
Executed at: 2026-04-08T15:30:00Z
Retry count: 0
Log: Migration completed successfully
```

Fields `Executed by`, `Executed at`, and `Log` show `--` when not set.

#### `agm state export`

Export state to stdout.

```
agm state export <FILE> [--format json|agm]
```

**Examples:**
```bash
# Export as JSON
agm state export workflow.agm --format json > state.json

# Export as canonical AGM state format
agm state export workflow.agm --format agm > workflow.agm.state
```

#### `agm state import`

Import state from a file.

```
agm state import <FILE> --from <STATE_FILE>
```

```bash
agm state import workflow.agm --from backup.agm.state
```

**Output:** `"Imported state from <from> to <target_path>"`

#### `agm state reset`

Reset execution state.

```
agm state reset <FILE> [OPTIONS]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--node` | string | none | Reset only this node |
| `--keep-completed` | flag | `false` | Preserve completed nodes |
| `--yes` | flag | `false` | Skip confirmation prompt |

**Examples:**
```bash
# Reset all nodes (with confirmation)
agm state reset workflow.agm

# Reset without prompt
agm state reset workflow.agm --yes

# Reset only non-completed nodes
agm state reset workflow.agm --keep-completed --yes

# Reset a single node
agm state reset workflow.agm --node deploy_app --yes
```

**Interactive prompt (without `--yes`):**
```
Reset execution state for workflow.agm? [y/N]
```

---

### `agm mem`

Manage memory sidecars. Has five subcommands.

#### `agm mem list`

List memory entries.

```
agm mem list <FILE> [OPTIONS]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--topic` | string | none | Filter by topic |
| `--scope` | `project`, `global` | both | Filter by scope |
| `--json` | flag | `false` | JSON output |

**Examples:**
```bash
# List all entries
agm mem list workflow.agm

# Filter by topic
agm mem list workflow.agm --topic auth

# Project-scope only, JSON output
agm mem list workflow.agm --scope project --json
```

**Output (text):**
```
Key                            Topic                Value
---                            -----                -----
db_connection_string           config               postgresql://localhost:5432/...
auth_token                     auth                 eyJhbGciOiJIUzI1NiIsInR5cCI6...
```

Values are truncated to 40 characters in the table. Use `mem get` for full values.

#### `agm mem get`

Get a specific memory entry by key.

```
agm mem get <FILE> --key <KEY>
```

Searches project store first, then global store.

```bash
agm mem get workflow.agm --key db_connection_string
```

**Output:**
```
Key: db_connection_string
Topic: config
Scope: project
Value:
postgresql://localhost:5432/mydb?sslmode=require
```

Exit `1` if key not found in either store.

#### `agm mem export`

Export project memory to stdout.

```
agm mem export <FILE> [--format json|agm]
```

```bash
agm mem export workflow.agm --format json > memory.json
agm mem export workflow.agm --format agm > workflow.agm.mem
```

Note: only exports the **project** store, not global.

#### `agm mem import`

Import memory from a file.

```
agm mem import <FILE> --from <MEM_FILE>
```

```bash
agm mem import workflow.agm --from backup.agm.mem
```

#### `agm mem gc`

Garbage-collect expired memory entries (duration-based TTLs).

```
agm mem gc <FILE>
```

```bash
agm mem gc workflow.agm
```

**Output:** `"GC complete: 3 expired entries removed, 0 orphans removed"`

---

### `agm context`

Build and display the agent context (prompt) that would be sent to an agent for a specific node.

```
agm context <FILE> --node <ID> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--node` | string | required | Node ID to build context for |
| `--json` | flag | `false` | Output as JSON with metadata |
| `--token-count` | flag | `false` | Show token estimate and section breakdown |
| `--working-dir` | path | current dir | Working directory for file resolution |

**Examples:**

```bash
# View the full prompt
agm context workflow.agm --node setup_db

# Token budget analysis
agm context workflow.agm --node setup_db --token-count

# JSON output with metadata
agm context workflow.agm --node setup_db --json
```

**Output (default):** Raw prompt text to stdout.

**Output (`--token-count`):**
```
Token estimate: 1247
Sections: 4
  system_hint (120 chars)
  target_node (2340 chars)
  dependencies (890 chars)
  memory (156 chars)
```

**Output (`--json`):**
```json
{
  "node_id": "setup_db",
  "token_estimate": 1247,
  "sections": [
    {"name": "system_hint", "chars": 120},
    {"name": "target_node", "chars": 2340},
    {"name": "dependencies", "chars": 890},
    {"name": "memory", "chars": 156}
  ],
  "prompt": "..."
}
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | Success |
| `1` | Node not found |
| `2` | File error or runtime init failure |

---

### `agm verify`

Run verification checks defined in node `verify:` fields.

```
agm verify <FILE> [OPTIONS]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `FILE` | yes | Path to the `.agm` file |

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--node` | string | none | Verify a specific node |
| `--all` | flag | `false` | Verify all nodes that have `verify:` checks |
| `--json` | flag | `false` | JSON output |
| `--working-dir` | path | current dir | Working directory |
| `--timeout` | seconds | `60` | Per-check timeout |

One of `--node` or `--all` must be specified.

**Examples:**

```bash
# Verify a specific node
agm verify workflow.agm --node setup_db

# Verify all nodes with checks
agm verify workflow.agm --all

# JSON output
agm verify workflow.agm --all --json

# Custom timeout
agm verify workflow.agm --node deploy_app --timeout 120
```

**Output (JSON):**
```json
[
  {
    "node_id": "setup_db",
    "all_passed": true,
    "checks": [
      {"passed": true, "message": "file exists: /tmp/db.sqlite", "duration_ms": 12},
      {"passed": true, "message": "command exited 0", "duration_ms": 340}
    ]
  }
]
```

**Verify check types:**

| Type | What it checks |
|------|----------------|
| `command` | Runs a shell command; passes if exit code is 0 |
| `file_exists` | Checks that a file path exists |
| `file_contains` | Checks that a file contains a pattern (literal or regex) |
| `file_not_contains` | Checks that a file does NOT contain a pattern |
| `node_status` | Checks that another node has a specific execution status |

**Exit codes:**

| Code | When |
|------|------|
| `0` | All checks passed |
| `1` | Any check failed; or invalid arguments |
| `2` | No verify checks found in target node(s) |

**Edge cases:**
- `--node` on a node without `verify:` field: `"No verify checks found."`, exit `2`.
- `--all` with no nodes having `verify:`: `"No verify checks found."`, exit `2`.
- All checks run to completion (no early-stop on failure).

---

### `agm update`

Update agm to the latest version from GitHub Releases.

```
agm update [--check]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--check` | flag | `false` | Check for updates without installing |

**Examples:**

```bash
# Check if an update is available
agm update --check

# Update to latest version
agm update
```

**Output (`--check`, update available):**
```
Current version: 0.1.0
Latest version:  0.2.0

Run `agm update` to install the latest version.
```

**Output (`--check`, up to date):**
```
agm 0.1.0 is up to date.
```

**Output (update successful):**
```
Updating agm from v0.1.0...
Updated to v0.2.0.
```

**Exit codes:**

| Code | When |
|------|------|
| `0` | Success |
| `1` | Network error, update failure, or self-update feature not compiled in |

**Notes:**
- Downloads from `github.com/JAAvila-Of/agm-cli/releases`.
- Auto-detects platform and architecture.
- Shows download progress bar during update.
- Requires the `self-update` feature flag (enabled by default). Builds without it print a message with the manual download URL.

---

## Concepts

### Load Modes

Each mode is a superset of the previous. Controls which fields are included when loading nodes.

| Mode | Fields Included |
|------|----------------|
| `summary` | `id`, `type`, `summary`, `priority`, `stability`, `depends`, `tags` |
| `operational` | Summary fields + `items`, `steps`, `fields`, `input`, `output` |
| `executable` | Operational fields + `code`, `verify`, `agent_context`, execution state fields |
| `full` | All fields including `detail`, `rationale`, `tradeoffs`, `resolution`, relational fields |

### Render Formats

| Format | Description | Use case |
|--------|-------------|----------|
| `json` | Standard JSON serialization of the AST | Programmatic consumption, piping to `jq` |
| `json-canonical` | Canonical JSON per AGM spec S37 | Deterministic output, hashing, diffing |
| `markdown` | Readable Markdown document | Documentation, human review |
| `agm` | Canonical AGM text format (round-trippable) | Normalization, reformatting |
| `dot` | Graphviz DOT graph | Visualization with `dot`, `neato`, etc. |
| `mermaid` | Mermaid diagram syntax | Embedding in Markdown, GitHub rendering |

### Enforcement Levels

Controls how strictly type schemas are enforced during validation.

| Level | Behavior |
|-------|----------|
| `strict` | All type schema rules enforced; unknown fields are errors |
| `standard` | Balanced enforcement; recommended for most use cases |
| `permissive` | Minimal enforcement; unknown fields are warnings |

### Error Output Formats

| Format | Destination | Description |
|--------|-------------|-------------|
| `text` | stderr | Rich terminal output via miette with source context, colors, and clickable links |
| `json` | stdout | JSON array of diagnostic objects for programmatic consumption |

**JSON diagnostic object schema:**
```json
{
  "code": "AGM-V003",
  "severity": "error",
  "message": "Node 'auth' references unknown dependency 'missing_node'",
  "file": "myfile.agm",
  "line": 42,
  "node": "auth"
}
```

### State Sidecar Files

Execution state is persisted in `.agm.state` sidecar files alongside the `.agm` file:

- `workflow.agm` -> `workflow.agm.state`

The sidecar uses the AGM native text format with `state <node_id>` blocks containing `execution_status`, `executed_by`, `executed_at`, `retry_count`, and `execution_log` fields.

State files are created automatically by `agm run` and can be managed with `agm state` subcommands.

### Memory Sidecar Files

Memory entries are persisted in `.agm.mem` sidecar files:

- **Project scope:** `workflow.agm` -> `workflow.agm.mem`
- **Global scope:** System-wide file shared across all AGM files

The sidecar uses the AGM native text format with `entry <key>` blocks containing `topic`, `scope`, `ttl`, `value`, `created_at`, and `updated_at` fields.

**Memory scopes:**

| Scope | Lifetime | Storage |
|-------|----------|---------|
| `node` | Single node execution | In-memory only (cleared after each node) |
| `session` | Single run | In-memory only (dropped when process exits) |
| `project` | Persistent | `.agm.mem` sidecar file |
| `global` | Persistent, cross-project | Global `.agm.mem` file |

**TTL values:**

| TTL | Behavior |
|-----|----------|
| `permanent` | Never expires |
| `session` | Expires when the run completes |
| `duration:<ISO8601>` | Expires after the specified duration (e.g., `duration:P7D` = 7 days, `duration:PT1H` = 1 hour) |

---

## Library API (agm-core)

The `agm-core` crate provides the parsing, validation, loading, graph, and rendering engine as a Rust library.

Full auto-generated API documentation is hosted at **[docs.rs/agm-core](https://docs.rs/agm-core)**.

Generate locally:

```bash
cargo doc --open -p agm-core --no-deps
```

### Module Overview

| Module | Description |
|--------|-------------|
| `parser` | Line-oriented parser: source text to AST |
| `validator` | Spec-compliance validation producing diagnostics |
| `loader` | Node filtering by load mode and profile |
| `graph` | Dependency graph: topological sort, cycle detection, queries |
| `renderer` | Output rendering: JSON, Markdown, DOT, Mermaid, canonical AGM, sidecar formats |
| `model` | AST types: nodes, fields, code blocks, orchestration, schemas, state, memory |
| `schema` | Type schema enforcement and registry |
| `import` | Import resolution and filesystem constraints |
| `memory` | Memory model types and validation |
| `error` | Error codes, diagnostics, and formatted output |

### Quick Example

```rust
use agm_core::parser::parse;
use agm_core::validator::validate;
use agm_core::loader::{load, LoadMode};
use agm_core::graph::build_graph;
use agm_core::renderer::{render, RenderFormat};

// Parse
let source = std::fs::read_to_string("myfile.agm")?;
let file = parse(&source).expect("parse failed");

// Validate
let diagnostics = validate(&file, None, Default::default());
let has_errors = diagnostics.iter().any(|d| d.is_error());

if !has_errors {
    // Load at operational level
    let loaded = load(&file, LoadMode::Operational);

    // Build dependency graph
    let graph = build_graph(&file)?;

    // Render as JSON
    let json = render(&file, RenderFormat::Json);
    println!("{json}");
}
```
