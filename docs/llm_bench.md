# `agm llm-bench` — LLM Emission Compliance Suite

## Purpose

`agm llm-bench` is a standardized benchmark for measuring how well an LLM inference
endpoint can emit structurally correct, semantically valid AGM (Agent Graph Memory) nodes
from natural-language prompts.

It answers: *"Given this model and endpoint, what fraction of prompted outputs are
spec-compliant AGM?"*

The suite ships 12 built-in prompt fixtures across three node types (`ticket`, `workflow`,
`orchestration`), including adversarial cases with ambiguous or contradictory descriptions.

---

## Quick Start

### Run with cassettes (no network required)

```bash
agm llm-bench \
  --model demo-m1 \
  --cassettes crates/agm-cli/tests/llm/cassettes
```

### Run against a live endpoint

```bash
export AGM_MESSAGES_ENDPOINT=https://your.provider/v1/messages
export AGM_MESSAGES_KEY=sk-...

agm llm-bench \
  --model your-model-id \
  --provider messages \
  --live
```

For Chat-Completions-style endpoints:

```bash
export AGM_CHAT_ENDPOINT=https://your.provider/v1/chat/completions
export AGM_CHAT_KEY=sk-...

agm llm-bench \
  --model your-model-id \
  --provider chat \
  --live
```

---

## Providers

`--provider messages` (default)
: Messages-style HTTP API. Sends a `system` prompt + `messages[]` array.
  Optionally includes a `tools[]` JSON Schema definition when the fixture
  specifies an expected node type.

`--provider chat`
: Chat-Completions-style HTTP API. Sends `messages[]` with `role=system` and
  `role=user`. Optionally includes function-style `tools[]`.

**No vendor names are hardcoded.** Endpoints and API keys come entirely from
environment variables — see the table in §Environment Variables below.

---

## Environment Variables

| Variable | Default for | Description |
|----------|-------------|-------------|
| `AGM_MESSAGES_ENDPOINT` | `--provider messages` | Full URL of the Messages-style API endpoint |
| `AGM_MESSAGES_KEY` | `--provider messages` | API key value |
| `AGM_CHAT_ENDPOINT` | `--provider chat` | Full URL of the Chat-Completions endpoint |
| `AGM_CHAT_KEY` | `--provider chat` | API key value |

Override the key variable name with `--api-key <VAR>`.

---

## All Flags

```
agm llm-bench [OPTIONS]

  --model <M>                 Model identifier (required unless --cost-only)
  --provider messages|chat    API shape (default: messages)
  --fixtures <DIR>            Custom fixture directory (default: built-in 12 cases)
  --case <GLOB>               Filter cases, e.g. "ticket/*" or "*/a"
  --format text|markdown|json Report format (default: text)
  --output <PATH>             Write report to file instead of stdout
  --concurrency <N>           Max concurrent requests (default: 2)
  --max-retries <N>           Retries on transient errors (default: 2)
  --timeout-secs <N>          Per-request timeout (default: 60)
  --normalize                 Apply normalize pass before evaluating
  --cassettes <DIR>           Enable cassette replay/record from this directory
  --record                    Force re-record cassettes (implies --cassettes)
  --live                      Skip cassettes; always hit the live endpoint
  --api-key <VAR>             Env var name for the API key
  --max-tokens-out <N>        Max output tokens hint sent to provider
  --cost-per-1k-in <f64>      USD per 1k input tokens (optional)
  --cost-per-1k-out <f64>     USD per 1k output tokens (optional)
  --cost-only                 Print cost estimate only (requires both --cost-per-* flags)
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All cases passed |
| 1 | Suite ran; one or more cases failed |
| 2 | Suite error (config error, missing fixtures, `--cost-only` without pricing flags) |
| 3 | Missing API key or endpoint env var |
| 4 | `--live` but network unreachable |

---

## Reading the Report

### Text format (default)

```
Model:  demo-m1
Cases:  12

  ticket/a                  PASS            (210 in, 180 out, 1100 ms)
  ticket/b                  PASS            (220 in, 185 out, 1050 ms)
  ticket/c                  PASS            (195 in, 160 out, 980 ms)
  ticket/d                  PASS            (240 in, 190 out, 1250 ms)
  ...

Compliance:        10/12 (83.3%)
Normalize-fixable: 11/12 (91.7%)
Schema match:      10/12 (83.3%)
Validate:          10/12 (83.3%)
Avg tokens in:     218
Avg tokens out:    172
p50 / p95 latency: 1100 ms / 1300 ms
```

### Compliance buckets

| Bucket | Meaning |
|--------|---------|
| `PASS` | Output is valid AGM, passes validate, passes schema, meets expectation |
| `NORM_FIXABLE` | Invalid raw; valid after `agm normalize` — model produced near-canonical output |
| `SCHEMA_ONLY` | Matches JSON Schema but fails `agm validate` |
| `VALIDATE_ONLY` | Passes `agm validate` but does not match JSON Schema |
| `FAIL` | Parses but fails both validate and schema |
| `ERROR` | Provider or parse error; response could not be evaluated |

### Compliance rate

The **compliance rate** counts only `PASS` buckets.

The **normalize-fixable rate** counts `PASS` + `NORM_FIXABLE` — outputs that can
be made valid by the normalize layer without human intervention.

---

## Cost Estimation

Cost estimation is opt-in. Pass both flags to enable it:

```bash
agm llm-bench \
  --model demo-m1 \
  --cost-per-1k-in 3.00 \
  --cost-per-1k-out 15.00 \
  --cassettes ...
```

To estimate cost without running the suite:

```bash
agm llm-bench \
  --model demo-m1 \
  --cost-only \
  --cost-per-1k-in 3.00 \
  --cost-per-1k-out 15.00
```

No pricing table is baked into the binary. You supply the rates from your provider's
current pricing page.

---

## How Cassettes Work

Cassettes are pre-recorded provider responses stored as JSON files on disk.

**Layout:**

```
<cassettes-dir>/
  v1.0.0/
    <provider>/        # "messages" or "chat"
      <model>/         # e.g. "demo-m1"
        ticket_a.json
        ticket_b.json
        ...
```

**Lookup key:** `SHA-256("<provider>:<model>:<full-prompt-text>")`. If the hash in
a cassette file does not match the current prompt, the cassette is treated as a miss.

**Cassette modes:**

| Flag | Behavior |
|------|----------|
| `--cassettes <DIR>` (default) | Replay mode: use cassette if present, error on miss |
| `--cassettes <DIR> --record` | Record mode: always call live API and overwrite cassettes |
| Neither | Disabled: always call live API, no cassette I/O |

---

## How Cassettes Are Produced

### Built-in cassettes (committed in repo)

The 24 cassettes under `crates/agm-cli/tests/llm/cassettes/v1.0.0/` are
**synthesized by hand**. They were not recorded from a real API call.

They exercise all compliance buckets and are designed to be deterministic, reproducible,
and free of real user data. Do not treat them as performance measurements for any real
inference endpoint.

### Re-recording against a real endpoint

To record fresh cassettes against a real provider:

```bash
export AGM_MESSAGES_ENDPOINT=https://...
export AGM_MESSAGES_KEY=sk-...

agm llm-bench \
  --model your-model-id \
  --cassettes path/to/cassettes \
  --record
```

This overwrites existing cassettes. The new files will contain real latencies and
token counts. Review the cassettes before committing: cassette files include the
first 120 characters of each prompt, so do not record against prompts containing
secrets.

**Privacy note:** cassette files include `prompt_preview` (first 120 chars of the
prompt) and `response_text`. Never record cassettes from prompts that contain
credentials, PII, or confidential data.

---

## Built-in Fixtures

The suite includes 12 cases — 4 per node type:

| Case ID | Type | Adversarial |
|---------|------|-------------|
| `ticket/a` | ticket | No — clear bug report |
| `ticket/b` | ticket | No — clear feature request |
| `ticket/c` | ticket | Yes — vague/ambiguous description |
| `ticket/d` | ticket | Yes — mixed prose narrative |
| `workflow/a` | workflow | No — CI/CD pipeline |
| `workflow/b` | workflow | No — incident response |
| `workflow/c` | workflow | Yes — incoherent terminology |
| `workflow/d` | workflow | Yes — contradictory requirements |
| `orchestration/a` | orchestration | No — data pipeline |
| `orchestration/b` | orchestration | No — code review system |
| `orchestration/c` | orchestration | Yes — vague multi-agent setup |
| `orchestration/d` | orchestration | Yes — partial spec with narrative |

Adversarial cases (c, d of each type) have relaxed expectations — they require
only that a syntactically valid node of the correct type is produced.

---

## Known-Good Compliance Rates (Built-in Suite)

The following rates are produced by running the built-in suite against the
**synthesized cassettes** in `tests/llm/cassettes/`. They are reference numbers
for the suite itself, not performance claims about any real inference endpoint.

| Metric | messages/demo-m1 | chat/demo-m1 |
|--------|-----------------|--------------|
| Compliance (PASS) | ~83% | ~83% |
| Normalize-fixable | ~83% | ~83% |
| Schema match | ~83% | ~83% |
| Validate | ~83% | ~83% |

To measure real compliance rates, record cassettes against a live endpoint
as described in "Re-recording against a real endpoint" above.

---

## Custom Fixtures

To use your own fixtures, create a directory with the layout:

```
my-fixtures/
  ticket/
    my_case/
      prompt.txt
      expected.yaml
  workflow/
    ...
```

`expected.yaml` shape:

```yaml
expected_type: ticket        # node type string
must_validate: true          # require agm validate to pass
must_match_schema: false     # require JSON Schema match
required_fields:
  - field: summary
  - field: status
    value: active            # optional exact-value check
```

Run with:

```bash
agm llm-bench \
  --model your-model \
  --fixtures my-fixtures/ \
  --cassettes my-cassettes/
```

---

## See Also

- `agm normalize` — normalize non-canonical AGM output
- `agm schema` — emit JSON Schema for AGM node types
- `agm validate` — validate an AGM file against the spec
- `docs/normalize.md` — normalize rule set documentation
- `docs/schemas.md` — JSON Schema documentation
