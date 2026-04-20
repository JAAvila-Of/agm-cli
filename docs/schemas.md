# JSON Schema Generation (`agm schema`)

`agm-core` contains a single source of truth for every AGM node type's structure: the
`schema::registry` module. `agm schema` exposes this as JSON Schema (Draft 2020-12) so
downstream tools — especially LLM tool-use integrations — can stay perfectly in sync with
the spec without writing schemas by hand.

## Quick start

```bash
# Print the vanilla JSON Schema for the ticket type
agm schema ticket

# Generate the Anthropic tool-use definition
agm schema ticket --for anthropic-tool-use

# Generate the OpenAI function-calling definition
agm schema ticket --for openai-tool

# Tighten the schema (reject unknown fields)
agm schema ticket --strict

# Override the tool name
agm schema ticket --for anthropic-tool-use --tool-name create_ticket

# Write all 11 built-in type schemas to a directory
agm schema all --output schemas/

# Write all schemas with Anthropic wrapping (creates schemas/anthropic_tool_use/)
agm schema all --for anthropic-tool-use --output schemas/

# YAML output
agm schema ticket --format yaml
```

## Supported types

All 11 built-in AGM node types have schemas:

| Type | Default tool name | Spec section |
|---|---|---|
| `facts` | `emit_facts` | §13.1 |
| `rules` | `emit_rules` | §13.2 |
| `workflow` | `emit_workflow` | §13.3 |
| `entity` | `emit_entity` | §13.4 |
| `decision` | `emit_decision` | §13.5 |
| `exception` | `emit_exception` | §13.6 |
| `example` | `emit_example` | §13.7 |
| `glossary` | `emit_glossary` | §13.8 |
| `anti_pattern` | `emit_anti_pattern` | §13.9 |
| `orchestration` | `plan_execution` | §13.10 |
| `ticket` | `create_ticket` | §13.11 |

`Custom` node types have no schema; `agm schema custom_xyz` returns exit code 1.

## Dialects

### Vanilla (`--for vanilla`, default)

Raw JSON Schema Draft 2020-12. Use this when integrating with a schema validator or
documentation tool.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "description": "AGM ticket node per spec v1.2.0 §13.11",
  "required": ["type", "summary", "title", "description", "priority"],
  "properties": { ... },
  "additionalProperties": true,
  "$defs": { ... }
}
```

### Anthropic tool-use (`--for anthropic-tool-use`)

Wraps the base schema in the shape expected by the Anthropic Messages API `tools` array:

```json
{
  "name": "create_ticket",
  "description": "Emit a ticket node per AGM spec v1.2.0 §13.11",
  "input_schema": { ... }
}
```

Drop the output directly into your `tools` array:

```python
import subprocess, json

schema = json.loads(
    subprocess.check_output(["agm", "schema", "ticket", "--for", "anthropic-tool-use"])
)
response = client.messages.create(
    model="<your-model>",
    tools=[schema],
    ...
)
```

### OpenAI function-calling (`--for openai-tool`)

Wraps the base schema in the shape expected by the OpenAI Chat Completions API:

```json
{
  "type": "function",
  "function": {
    "name": "create_ticket",
    "description": "Emit a ticket node per AGM spec v1.2.0 §13.11",
    "parameters": { ... }
  }
}
```

## Flags reference

| Flag | Default | Description |
|---|---|---|
| `--format json-schema\|yaml` | `json-schema` | Output format |
| `--for vanilla\|anthropic-tool-use\|openai-tool` | `vanilla` | Dialect wrapper |
| `--include-enums` / `--no-include-enums` | enums on | Whether enum fields emit `enum` clauses |
| `--strict` | off | Set `additionalProperties: false`; useful for strict validation |
| `--tool-name <NAME>` | type default | Override tool name in dialect wrapping |
| `--tool-description <TEXT>` | generated | Override tool description |
| `--output <PATH>` | stdout | Write output to file (or directory for `all`) |
| `--pretty` | true | Pretty-print JSON |

### `--strict` mode

Without `--strict` (default), `additionalProperties: true` allows any extra field. This
matches how the AGM validator's permissive and standard modes work.

With `--strict`, `additionalProperties: false` rejects any field not in the explicit allowed
set for that node type. This is equivalent to `EnforcementLevel::Strict` in the validator.

## Rust API

```rust
use agm_core::schemas::{
    schema_for, ticket_schema, SchemaOptions, SchemaDialect, SchemaGenError,
};
use agm_core::model::fields::NodeType;

// Convenience function — vanilla, default options
let schema: serde_json::Value = ticket_schema();

// Full control
let opts = SchemaOptions {
    dialect: SchemaDialect::AnthropicToolUse,
    include_enums: true,
    strict: false,
    tool_name: Some("create_ticket".to_owned()),
    tool_description: None,
};
let schema = schema_for(&NodeType::Ticket, &opts).unwrap();

// Error handling
match schema_for(&NodeType::Custom("widget".to_owned()), &opts) {
    Err(SchemaGenError::CustomType(s)) => eprintln!("no schema for {s}"),
    _ => {}
}
```

All 11 convenience functions follow the same pattern:

```rust
agm_core::schemas::facts_schema()
agm_core::schemas::rules_schema()
agm_core::schemas::workflow_schema()
agm_core::schemas::entity_schema()
agm_core::schemas::decision_schema()
agm_core::schemas::exception_schema()
agm_core::schemas::example_schema()
agm_core::schemas::glossary_schema()
agm_core::schemas::anti_pattern_schema()
agm_core::schemas::orchestration_schema()
agm_core::schemas::ticket_schema()
```

`agm_core::schemas::universal_fields_schema()` returns a schema covering only the fields
that are allowed on every node type (spec §14.2).

## Schema versioning

Schemas are generated at runtime from the same registry the validator uses. When the AGM
spec changes, re-run `agm schema` to get updated schemas — they will always reflect the
current spec version.

The `description` field in each schema states the spec version:
`"AGM ticket node per spec v1.2.0 §13.11"`.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Unknown or custom node type |
| `2` | Invalid flag combination or I/O error |
