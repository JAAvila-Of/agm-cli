# Builder API — Quick Start

`agm-core` 1.3.0 ships a fluent builder API for constructing and emitting
AGM nodes from Rust. Builders run the same validation passes as
`agm validate` so you always get a spec-compliant `Node`.

## Overview

```
Builder::new(id)
    .field(value)   ← setters return Self by value (fluent chain)
    .build()        ← runs Standard validation, returns Result<Node, BuildError>
```

All builders live in `agm_core::builder` and are re-exported from the crate
root under that module:

```rust
use agm_core::builder::{
    TicketBuilder, WorkflowBuilder, OrchestrationBuilder,
    FactsBuilder, RulesBuilder, DecisionBuilder, MemoryEntryBuilder,
    CodeBlockBuilder, VerifyCheckBuilder, BuildError,
};
```

## Creating a ticket node

```rust
use agm_core::builder::TicketBuilder;
use agm_core::model::fields::{Priority, Stability};
use agm_core::model::ticket::{TicketAction, SddPhase};

let node = TicketBuilder::new("octopus.ticket.oauth-login")
    .summary("add OAuth2 login endpoint")
    .title("Add OAuth2 login endpoint")
    .description("Implement /login with Google OAuth2 and JWT issuance.")
    .priority(Priority::High)
    .action(TicketAction::Create)
    .sdd_phase(SddPhase::Propose)
    .labels(["auth", "api"])
    .stability(Stability::Medium)
    .build()?;

// Render to canonical AGM text
let text = node.render_node_only();
println!("{text}");
```

## Creating a workflow node

```rust
use agm_core::builder::{WorkflowBuilder, CodeBlockBuilder, VerifyCheckBuilder};
use agm_core::model::code::{CodeAction};

let patch = CodeBlockBuilder::replace()
    .target("src/auth/mod.rs")
    .lang("rust")
    .anchor("// TODO: auth")
    .body("pub mod oauth;")
    .build()?;

let check = VerifyCheckBuilder::command("cargo test --lib")
    .expect("exit_code_0")
    .build()?;

let node = WorkflowBuilder::new("auth.workflow.add-oauth")
    .summary("implement OAuth2 login")
    .steps(["scaffold module", "add route handler", "write tests"])
    .code_blocks([patch])
    .verify([check])
    .build()?;
```

## Creating an orchestration node

```rust
use agm_core::builder::OrchestrationBuilder;
use agm_core::model::orchestration::ParallelGroup;

let groups = vec![
    ParallelGroup {
        name: "backend".to_owned(),
        nodes: vec!["auth.workflow.add-oauth".to_owned()],
        requires: None,
    },
    ParallelGroup {
        name: "frontend".to_owned(),
        nodes: vec!["ui.workflow.login-form".to_owned()],
        requires: Some(vec!["backend".to_owned()]),
    },
];

let node = OrchestrationBuilder::new("auth.plan")
    .summary("OAuth2 rollout plan")
    .parallel_groups(groups)
    .build()?;
```

## Helper builders

### `CodeBlockBuilder`

Shortcut constructors match the `CodeAction` variants:

| Constructor | Action |
|-------------|--------|
| `CodeBlockBuilder::create()` | `Create` |
| `CodeBlockBuilder::append()` | `Append` |
| `CodeBlockBuilder::prepend()` | `Prepend` |
| `CodeBlockBuilder::replace()` | `Replace` |
| `CodeBlockBuilder::insert_before(anchor)` | `InsertBefore` |
| `CodeBlockBuilder::insert_after(anchor)` | `InsertAfter` |
| `CodeBlockBuilder::full()` | `Full` |

`build()` enforces structural invariants:
- Non-`Full` / non-`Create` actions require `target`.
- `Replace` requires exactly one of `anchor` or `old`.
- `InsertBefore` / `InsertAfter` require `anchor`.

You can also use the ergonomic shortcuts on `CodeBlock`:

```rust
use agm_core::model::code::CodeBlock;

let block = CodeBlock::insert_before("fn main")
    .lang("rust")
    .body("use std::env;")
    .build()?;
```

### `VerifyCheckBuilder`

```rust
use agm_core::builder::VerifyCheckBuilder;

let cmd   = VerifyCheckBuilder::command("cargo test").expect("exit_code_0").build()?;
let exist = VerifyCheckBuilder::file_exists("src/lib.rs").build()?;
let has   = VerifyCheckBuilder::file_contains("src/lib.rs", "fn hello").build()?;
let lacks = VerifyCheckBuilder::file_not_contains("src/lib.rs", "unsafe").build()?;
let stat  = VerifyCheckBuilder::node_status("auth.login", "completed").build()?;
```

### `MemoryEntryBuilder`

```rust
use agm_core::builder::MemoryEntryBuilder;
use agm_core::model::memory::MemoryAction;

let entry = MemoryEntryBuilder::new("auth.current_user", "auth", MemoryAction::Upsert)
    .value("jane@example.com")
    .build()?;
```

## Rendering nodes

Every `Node` has two inherent render methods added in 1.3.0:

| Method | Output |
|--------|--------|
| `render_canonical()` | Full AGM text including a scratch `agm: / package: / version:` header |
| `render_node_only()` | Just the `node <id>` block — no header |

`render_node_only` is what you typically want when inserting a node into an
existing file or sending it to an LLM tool-call response.

```rust
let canonical_file_text = node.render_canonical();
let node_block_only     = node.render_node_only();
```

## Validation modes

`build()` runs at `EnforcementLevel::Standard` by default.
Use `build_with(level)` to choose a different level:

```rust
use agm_core::model::schema::EnforcementLevel;

// Strict: all optional conventions become errors
let node = TicketBuilder::new("my.ticket")
    .summary("test")
    .title("test ticket")
    .description("testing strict mode")
    .priority(Priority::Low)
    .build_with(EnforcementLevel::Strict)?;

// Permissive: only hard spec violations block build
let node = TicketBuilder::new("draft.ticket")
    .summary("wip")
    .build_with(EnforcementLevel::Permissive)?;
```

## `build_unchecked` — skipping validation

Use `build_unchecked()` when you are constructing nodes that will be
validated later as part of a multi-node file (e.g., nodes that reference
each other via `depends` or `related_to`):

```rust
// Cross-node references fail V004 in single-node mode.
// build_unchecked() only checks that the node ID is non-empty.
let node = WorkflowBuilder::new("auth.workflow")
    .summary("implement auth")
    .depends(["auth.constraints", "auth.rules"])   // external references
    .build_unchecked()?;

// Combine with other nodes into a file, then validate the file as a whole.
let file = AgmFile { header, nodes: vec![node, constraints, rules] };
let diagnostics = validator::validate(&file, "", "<my-builder>", &ValidateOptions::default());
```

## Handling `BuildError`

```rust
use agm_core::builder::BuildError;

match node_result {
    Ok(node) => { /* use node */ }
    Err(BuildError::Validation(diag)) => {
        // diag is Box<DiagnosticCollection> — same type as agm validate output
        for d in diag.diagnostics() {
            eprintln!("[{}] {}", d.severity, d.message);
        }
    }
    Err(BuildError::Precondition(msg)) => {
        eprintln!("Builder pre-condition failed: {msg}");
    }
}
```

`BuildError` also exposes two helper predicates:
- `err.is_validation()` — true when the variant is `Validation`
- `err.is_precondition()` — true when the variant is `Precondition`

And a direct accessor:
- `err.diagnostics()` → `Option<&DiagnosticCollection>`

## All available builders

| Builder | Node type | Required fields |
|---------|-----------|-----------------|
| `TicketBuilder` | `ticket` | `summary`, `title`, `description`, `priority` |
| `WorkflowBuilder` | `workflow` | `summary` |
| `OrchestrationBuilder` | `orchestration` | `summary`, `parallel_groups` |
| `FactsBuilder` | `facts` | `summary` |
| `RulesBuilder` | `rules` | `summary`, `items` |
| `DecisionBuilder` | `decision` | `summary`, `rationale` |
| `MemoryEntryBuilder` | — | `key`, `topic`, `action` (builds `MemoryEntry`, not a `Node`) |

Helper builders:

| Builder | Product |
|---------|---------|
| `CodeBlockBuilder` | `CodeBlock` |
| `VerifyCheckBuilder` | `VerifyCheck` |

## See also

- [AGM Specification v1.2.0](spec/agm_spec_v1.2.0.md)
- [JSON Schema Generation](schemas.md)
- [`agm_core::builder` (docs.rs)](https://docs.rs/agm-core/latest/agm_core/builder/)
