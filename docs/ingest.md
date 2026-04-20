# `agm ingest` — Tool-Call JSON to Canonical AGM

`agm ingest` converts the JSON argument objects produced by LLM tool-calls into
canonical AGM text. It is the shortest path from a completed tool-call to a
persisted `.agm` node.

---

## Synopsis

```
agm ingest <TYPE>
    --package <PKG>
    [--id <NODE_ID>]
    [--file <PATH>]
    [--no-normalize]
    [--no-schema-check]
    [--enforcement strict|standard|permissive]
    [--output <PATH>]
    [--version <SEMVER>]
    [--header-title <TEXT>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `<TYPE>` | (required) | Node type: `ticket`, `workflow`, `facts`, `rules`, `decision`, `orchestration`, `entity`, `exception`, `example`, `glossary`, `anti_pattern` |
| `--package` | (required) | Package name for the generated AGM file header |
| `--id` | `""` | Node id (single mode) or id prefix (batch mode) |
| `--file` | stdin | Read JSON from this file instead of stdin |
| `--no-normalize` | off | Skip field-name normalization |
| `--no-schema-check` | off | Skip JSON Schema pre-validation |
| `--enforcement` | `standard` | Enforcement level for post-build validation |
| `--output` | stdout | Write AGM output to this file |
| `--version` | `0.1.0` | Package version in the generated file header |
| `--header-title` | (none) | Optional title in the generated file header |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Ingested successfully; AGM written to output |
| `1` | Schema check or post-build validation failure |
| `2` | Input is not valid JSON |
| `3` | Missing required flag or unknown node type |

---

## Quick start

### Single node from stdin

```bash
echo '{
  "type": "ticket",
  "summary": "add OAuth2 login",
  "title": "Add OAuth2 Login",
  "description": "Implement the OAuth2 PKCE flow.",
  "priority": "high"
}' | agm ingest ticket \
      --package octopus.tickets \
      --id octopus.ticket.oauth
```

Emits:

```
agm: 1.0
package: octopus.tickets
version: 0.1.0

node octopus.ticket.oauth
type: ticket
summary: add OAuth2 login
title: Add OAuth2 Login
description: Implement the OAuth2 PKCE flow.
priority: high
```

### From a file

```bash
agm ingest ticket \
    --package octopus.tickets \
    --id octopus.ticket.oauth \
    --file args.json
```

### Batch (JSON array)

```bash
cat batch.json | agm ingest ticket \
    --package octopus.tickets \
    --id octopus.batch
```

Each array element becomes a node. Element-level `"node"` or `"id"` fields
override the synthesized ID; otherwise nodes are named `{prefix}.N` (where `N`
is a synthesized suffix — provide explicit `"node"` fields for human-readable
IDs that pass V021 pattern validation).

---

## Integration with Anthropic tool-calls

Define the AGM ticket schema as an Anthropic tool:

```bash
agm schema ticket --for anthropic-tool-use > create_ticket.json
```

Pass the tool definition to Claude. When the model invokes it, pipe
`input` from the tool-call result through `agm ingest`:

```python
import subprocess, json

def handle_tool_call(tool_name: str, tool_input: dict) -> str:
    """Convert a tool-call result to canonical AGM."""
    result = subprocess.run(
        ["agm", "ingest", tool_name,
         "--package", "myproject.tickets",
         "--id", f"myproject.{tool_name}.auto"],
        input=json.dumps(tool_input).encode(),
        capture_output=True,
    )
    if result.returncode != 0:
        raise ValueError(result.stderr.decode())
    return result.stdout.decode()
```

### Full example (Python + anthropic SDK)

```python
import anthropic, subprocess, json

client = anthropic.Anthropic()

# Fetch the schema as an Anthropic tool definition
schema_proc = subprocess.run(
    ["agm", "schema", "ticket", "--for", "anthropic-tool-use"],
    capture_output=True, text=True, check=True,
)
ticket_tool = json.loads(schema_proc.stdout)

messages = [{"role": "user", "content": "Create a ticket to add rate limiting."}]
response = client.messages.create(
    model="claude-opus-4-5",
    max_tokens=1024,
    tools=[ticket_tool],
    messages=messages,
)

for block in response.content:
    if block.type == "tool_use" and block.name == "create_ticket":
        agm_text = handle_tool_call("ticket", block.input)
        print(agm_text)
```

---

## Integration with OpenAI tool-calls

```bash
agm schema ticket --for openai-tool > ticket_function.json
```

```python
import openai, subprocess, json

client = openai.OpenAI()

with open("ticket_function.json") as f:
    ticket_function = json.load(f)

response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Create a ticket for adding rate limiting."}],
    tools=[ticket_function],
    tool_choice="auto",
)

for choice in response.choices:
    msg = choice.message
    if msg.tool_calls:
        for tc in msg.tool_calls:
            args = json.loads(tc.function.arguments)
            result = subprocess.run(
                ["agm", "ingest", tc.function.name,
                 "--package", "myproject.tickets",
                 "--id", "myproject.ticket.auto"],
                input=json.dumps(args).encode(),
                capture_output=True,
            )
            print(result.stdout.decode())
```

---

## Field synthesis

Before schema validation and node construction, the ingest pipeline enriches
the raw JSON with three automatic injections:

1. **`type`** — copied from the CLI subcommand (e.g. `ticket`). If JSON already
   contains `"type"` and it conflicts with the subcommand, ingest returns a
   clear type-mismatch error rather than silently overriding either value.
2. **`node`** — set to the resolved node ID (`--id` or JSON `node`/`id` field).
   Only injected when neither `"node"` nor `"id"` is present in the JSON.
3. **`summary`** — synthesized from `"title"` when `"summary"` is absent.
   This preserves LLM ergonomics: tool callers only need a human-readable
   `title`; the `summary` field required by AGM is derived automatically.

All three injections apply unconditionally; there is no flag to disable them.

---

## Normalization

By default, `agm ingest` runs the normalize layer (news_1) after building the
node. This means field-name synonyms emitted by models — such as `depends_on`
instead of `depends`, or `groups` instead of `parallel_groups` — are silently
rewritten to canonical form before validation.

Disable normalization with `--no-normalize` if you need to audit raw model
output.

---

## Schema validation

By default, the JSON input is validated against the AGM JSON Schema (news_2)
for the given node type before the builder runs. This provides fast, precise
error messages tied to the raw tool-call output — before any transformation
occurs.

```
$ echo '{"type":"ticket","summary":"x","title":"t","description":"d","priority":"urgent"}' \
    | agm ingest ticket --package p --id n
error: schema check failed: /priority: "urgent" is not one of ["critical","high","normal","low"]
hint: change "urgent" to one of the allowed values; see `agm schema ticket`.
```

Disable with `--no-schema-check` to let the full validator catch issues after
normalization (useful when the model emits field synonyms that the schema does
not recognize).

---

## Enforcement levels

| Level | Behaviour |
|-------|-----------|
| `permissive` | Only hard errors (V001–V003). Useful for ingesting partial drafts. |
| `standard` | Default. Required fields + schema compliance. |
| `strict` | Strict schema: disallows extra fields (V016), enforces all warnings as errors. |

```bash
agm ingest ticket --package p --id n --enforcement strict < args.json
```

---

## Round-trip guarantee

Output from `agm ingest` is canonical AGM that round-trips through `agm validate`:

```bash
agm ingest ticket --package p --id n < args.json | agm validate /dev/stdin
```

The round-trip property holds as long as the input passes post-build validation.
