# agm corpus — Cacheable System-Prompt Corpora

`agm corpus` emits a system-prompt-ready corpus in one of three flavors
targeting one of three provider formats. The primary use case is seeding an
agent's system prompt with the AGM format reference so that the agent can
produce valid AGM output reliably.

---

## Usage

```
agm corpus [FLAGS]

FLAGS:
  --flavor <F>          full (default) | standard | grammar-only
  --for <TARGET>        anthropic | openai | vanilla (default: vanilla)
  --min-tokens <N>      pad with examples until token estimate >= N
  --no-version          omit the "# AGM spec version: 1.2.0" header
  --output <PATH>       write to file (default: stdout)
  --format text|json    default: text; json wraps body + token metadata
  --count-only          print only the estimated token count
```

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | `--min-tokens` cannot be reached; bank exhausted |
| 2 | I/O error writing to `--output` |

---

## Flavors

Three corpus flavors are available. Choose based on token budget and how much
context the agent needs.

### `full` (default)

- Full grammar summary, semantic notes, validation rules, and 6 varied examples
- Covers all node types, loading modes, field reference, and executable fields
- Token range: approximately 1800–2600 tokens (without padding)
- Use when: maximum comprehension is needed and you have token budget

### `standard`

- Abridged grammar reference and 3 examples
- Covers the core fields and loading modes without exhaustive detail
- Token range: approximately 900–1500 tokens
- Use when: token budget is moderate and the agent only needs the core grammar

### `grammar-only`

- Grammar summary only with 1 minimal example
- Covers required fields, node types, and common field patterns compactly
- Token range: approximately 300–600 tokens
- Use when: token budget is tight or the agent just needs a quick reference

---

## Provider Targets

### `vanilla` (default)

No wrapping is applied. The corpus body is emitted as-is. Use this when:
- The system prompt is assembled programmatically and no role preamble is needed
- You want to inject the corpus inside a larger system prompt you control

### `anthropic`

Prepends an assistant role preamble before the corpus body:

```
You are an AGM-emitting assistant. When asked to produce knowledge, emit
valid AGM v1.2.0 text inside a fenced ```agm block. Follow the grammar
below strictly. Do not invent fields. When given a tool schema, prefer
the tool call.

---

<corpus body>
```

**Cache eligibility for Anthropic:**
The Anthropic API caches system prompts that exceed approximately 2048 tokens.
To enable caching, set the `cache_control` parameter on the system block in
your API request:

```json
{
  "system": [
    {
      "type": "text",
      "text": "<corpus body from agm corpus --for anthropic>",
      "cache_control": { "type": "ephemeral" }
    }
  ]
}
```

The cache is valid for 5 minutes and is refreshed on each cache hit. Only the
prefix up to and including the `cache_control` marker is cached; content after
it is not.

To reliably exceed the 2048-token threshold, use `--min-tokens 2048`:

```sh
agm corpus --for anthropic --min-tokens 2048
```

The `full` flavor (≈2500 tokens) typically exceeds the threshold without padding.

### `openai`

Prepends a tool-preference role preamble before the corpus body:

```
You are an AGM-emitting assistant. When the host provides a `create_ticket`,
`plan_execution`, or similar tool, always use the tool rather than free-form
AGM text. Otherwise, emit AGM inside ```agm fences. Follow the grammar.

---

<corpus body>
```

**Cache eligibility for OpenAI:**
The OpenAI API supports automatic prefix caching for context that stays
stable across requests. No special API parameter is needed — OpenAI
automatically caches matching prefixes. Keep the corpus at the start of the
system message and keep it stable between calls to maximize cache hit rates.

---

## Token Budgets and Padding

The `--min-tokens` flag guarantees the corpus reaches a minimum token count
by appending additional AGM examples from a built-in bank.

```sh
# Ensure the corpus is at least 2048 tokens for Anthropic caching
agm corpus --for anthropic --min-tokens 2048

# Check the resulting token count before using
agm corpus --for anthropic --min-tokens 2048 --count-only
```

The example bank contains 12 examples covering all major node types. If the
bank is exhausted before reaching the target, the command exits with code 1:

```
error: cannot reach 20000-token target; bank exhausted at 8400 tokens.
       Lower --min-tokens or add examples.
```

### Typical token counts

| Flavor | Vanilla | Anthropic (+preamble) |
|--------|---------|-----------------------|
| `full` | ~2500 | ~2550 |
| `standard` | ~1100 | ~1150 |
| `grammar-only` | ~500 | ~550 |

These are estimates using the `cl100k_base` tokenizer. The Anthropic and
OpenAI tokenizers may differ slightly. A 10% safety margin is recommended
when targeting the 2048-token caching threshold.

---

## Output Formats

### Text (default)

Emits the corpus body as plain text to stdout:

```sh
agm corpus --for anthropic > system_prompt.md
```

### JSON

Emits a JSON object with the body and metadata:

```sh
agm corpus --format json --for anthropic
```

Output shape:

```json
{
  "body": "...",
  "estimated_tokens": 2548,
  "flavor": "full",
  "target": "anthropic"
}
```

### Count-only

For scripting, `--count-only` emits just the integer token count:

```sh
agm corpus --count-only --for anthropic
# 2509
```

---

## Stable Output

The corpus output is deterministic for a given (`flavor`, `target`,
`min-tokens`) tuple. The body does not change between runs unless:
- The template files are updated
- The example bank is updated
- The AGM format version changes

This stability is important for cache effectiveness — the same stable text
will result in cache hits on subsequent API calls.

---

## Quick Start Examples

### Anthropic: generate and use the corpus

```sh
# Generate a corpus with at least 2048 tokens for the Anthropic cache
agm corpus --for anthropic --min-tokens 2048 > corpus.md

# Check the token count
agm corpus --for anthropic --min-tokens 2048 --count-only

# Use the corpus in an API call (pseudo-code)
# system = [{ "type": "text", "text": open("corpus.md").read(),
#             "cache_control": { "type": "ephemeral" } }]
```

### OpenAI: minimal standard corpus

```sh
# Generate a standard corpus for OpenAI (prefix cache eligible by default)
agm corpus --for openai --flavor standard > system_prompt.md
```

### Scripting: count-only guard

```sh
TOKENS=$(agm corpus --for anthropic --count-only)
if [ "$TOKENS" -lt 2048 ]; then
  echo "Warning: corpus below Anthropic cache threshold ($TOKENS tokens)"
fi
```

### JSON output for programmatic use

```sh
# Parse token count from JSON output
agm corpus --format json --for anthropic | jq .estimated_tokens
```
