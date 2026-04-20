# agm normalize

The `agm normalize` command rewrites non-canonical field and type names in an
AGM file to their canonical forms as defined in the AGM v1.2.0 specification.

## Why normalization?

Hand-written or generated AGM is often *semantically* correct but *syntactically*
non-canonical. Common synonyms include `type: plan_execution` and `groups:`
instead of `type: orchestration` and `parallel_groups:`, `phases:` instead of
`parallel_groups:`, or `prereq:` / `depends_on:` instead of `depends:`. The
validator rejects all of these, even though the meaning is preserved.

`agm normalize` sits in front of `agm validate` to bridge that gap:

```
$ agm normalize plan.agm | agm validate -
```

It operates strictly on **field names and node types**. It never guesses
semantics, invents values, or autocorrects grammar. If it cannot safely rewrite
something it emits a warning and leaves the input unchanged.

## Usage

```
agm normalize <FILE>
    [--rules <PATH>]              # Additional/override rules YAML
    [--output <PATH>]             # Write output here (default: stdout)
    [--in-place]                  # Rewrite FILE in place (no backup created)
    [--explain]                   # Print the rewrite report on stderr
    [--report-format text|json]   # Report format; default text
    [--no-types]                  # Skip type-level normalization
    [--no-fields]                 # Skip field-level normalization
    [--check]                     # Exit 0 if canonical, 1 if rewrites needed
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0    | Normalized successfully (or `--check` confirmed input was canonical) |
| 1    | `--check` detected at least one rewrite would be made |
| 2    | Parse error in input or I/O error |
| 3    | Rule file error |

## Example

```
$ agm normalize plan.agm --explain

agm: 1.0
package: my.plan
version: 0.1.0

node plan
type: orchestration
summary: ...
parallel_groups: [g1, g2]

Normalize report:
  2 rewrites, 0 warnings
  - [type.orchestration] node plan (line 4)
      - plan_execution
      + orchestration
  - [field.orchestration.parallel_groups] node plan (line 9)
      - groups
      + parallel_groups
```

## Rule files

The built-in rule set is embedded in the binary and covers the most common
LLM output patterns. You can extend or override it with a custom YAML file:

```
$ agm normalize plan.agm --rules my-rules.yaml
```

A project-local `.agm-normalize.yaml` in the current working directory is
automatically merged if present (silent if absent).

### Rule file schema

```yaml
# Map: canonical node type -> [synonyms]
type_aliases:
  orchestration:
    - plan
    - plan_execution
    - execution_plan

# Map: canonical node type -> canonical field -> [synonyms]
field_aliases:
  orchestration:
    parallel_groups:
      - groups
      - phases
  workflow:
    steps:
      - tasks
      - task_list

# Field aliases that apply to every node type
universal_field_aliases:
  depends:
    - depends_on
    - prereq
    - needs
  related_to:
    - related
    - see
    - relates_to
```

When you supply a `--rules` file, its entries *replace* (not append to) the
corresponding built-in entries. Use this to override a built-in synonym list
rather than augment it.

## Collision handling

When both the canonical name and a synonym are present on the same node:

- **Equal values** — the synonym is silently dropped; no rewrite is recorded.
- **Different values** — the canonical value is kept; the synonym is dropped; a
  `CollisionKeepingCanonical` warning is emitted.

## In-place mode

`--in-place` rewrites the file directly. No `.bak` copy is created (matching
`rustfmt` behaviour). If you need a backup, copy the file yourself before
running.

## Built-in rules (v1.2.0)

### Type synonyms

| Canonical | Synonyms |
|-----------|---------|
| `orchestration` | `plan`, `plan_execution`, `execution_plan` |

### Field synonyms — orchestration

| Canonical | Synonyms |
|-----------|---------|
| `parallel_groups` | `groups`, `phases` |
| `depends` | `depends_on`, `prereq`, `needs` |

### Field synonyms — workflow

| Canonical | Synonyms |
|-----------|---------|
| `code_blocks` | `steps_with_code`, `code_steps` |
| `steps` | `tasks`, `task_list` |

### Field synonyms — ticket

| Canonical | Synonyms |
|-----------|---------|
| `labels` | `tags_list` |

### Universal field synonyms (all node types)

| Canonical | Synonyms |
|-----------|---------|
| `depends` | `depends_on`, `prereq`, `needs` |
| `related_to` | `related`, `see`, `relates_to` |

## Library API

```rust
use agm_core::normalize::{NormalizeConfig, normalize_text};

let config = NormalizeConfig::default();  // uses built-in rules
let (canonical_agm, report) = normalize_text(&raw_input, &config)?;

println!("Applied {} rewrites", report.rewrites.len());
println!("{}", canonical_agm);
```

For consumers that already have an AST:

```rust
use agm_core::normalize::{NormalizeConfig, normalize_ast};

let mut file = agm_core::parser::parse(&source)?;
let report = normalize_ast(&mut file, &NormalizeConfig::default());
```
