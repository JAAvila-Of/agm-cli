# agm repair — Syntax-Level Repair Guide

`agm repair` applies a curated set of conservative text-level rewrite rules to
AGM files that contain syntax errors commonly produced by language models. It
operates on raw text **before** the parser sees the file, making it complementary
to `agm normalize` which works on the parsed AST.

## When to Use repair vs normalize

| Problem | Tool |
|---------|------|
| Smart quotes, CRLF, tabs, asterisk bullets | `agm repair` |
| Wrapped in a ` ```agm ``` ` fence | `agm repair` |
| Prose preamble before the header | `agm repair` |
| Non-canonical field names (`depends_on` → `depends`) | `agm normalize` |
| Non-canonical node types | `agm normalize` |
| Both syntax and field issues | `agm fix` (runs both) |

## Quick Start

```bash
# Repair to stdout (dry-run equivalent)
agm repair broken.agm

# Repair and overwrite file
agm repair --in-place broken.agm

# See what would be changed without modifying anything
agm repair --check broken.agm   # exits 1 if rewrites needed

# Repair and show report
agm repair --explain broken.agm

# Run both repair and normalize
agm fix broken.agm --in-place --explain
```

## Built-in Rules

Rules are applied in the canonical order shown below. All rules except
`R-BARE-CODE-BLOCK` are enabled by default.

| Rule ID | Default | Purpose |
|---------|---------|---------|
| `R-ZERO-WIDTH` | on | Strip BOM (U+FEFF) and zero-width characters (U+200B, U+200C, U+200D) |
| `R-CRLF` | on | Normalize `\r\n` and bare `\r` to `\n` |
| `R-TABS-TO-SPACES` | on | Convert leading tabs to 2-space indentation (spec P004) |
| `R-TRAILING-WS` | on | Strip trailing whitespace from every line |
| `R-SMART-QUOTES` | on | Replace typographic quotes (`"`, `"`, `'`, `'`, `«`, `»`) with ASCII |
| `R-MISSING-AGM-FENCE` | on | Strip wrapping ` ```agm ``` ` code fence |
| `R-PROSE-BEFORE-HEADER` | on | Strip leading prose before the AGM header |
| `R-BULLET-STAR` | on | Replace `*`/`+` bullets with `-` where contextually safe |
| `R-BARE-CODE-BLOCK` | **off** | Wrap bare indented code blocks in explicit fences (risky) |

### Rule Details

**`R-ZERO-WIDTH`**: Strips the Unicode byte-order mark and zero-width
joiner/non-joiner characters that some editors or language models insert.

**`R-CRLF`**: Normalizes all line endings to Unix LF. Records a single
`RepairRecord` per file (not per line) for efficiency.

**`R-TABS-TO-SPACES`**: Converts leading tabs to 2-space indentation. Only
touches tabs in the leading whitespace of each line; mid-line tabs (which may
be meaningful inside scalar values) are preserved.

**`R-TRAILING-WS`**: Strips trailing spaces and tabs from every line. Records
one entry per affected line.

**`R-SMART-QUOTES`**: Replaces typographic quote characters outside
triple-backtick fences. Characters inside code fences (e.g., ```` ``` ````
blocks) are preserved. Tracks fence state with a simple toggle on lines
matching `^\s*` ``` .*$`.

**`R-MISSING-AGM-FENCE`**: Detects when the entire input is wrapped in a
`` ` ```agm ` `` or `` ` ``` ` `` fence (first non-blank line is the opening
fence, last non-blank line is the closing fence) and strips both fence lines.
This handles the common pattern where some model providers wrap their AGM
output in a code fence.

**`R-PROSE-BEFORE-HEADER`**: Strips leading lines that appear before the AGM
header. The heuristic looks for a valid AGM marker (`agm:`, `package:`,
`version:`, `# `, or `node `) within the first 20 non-blank lines. If no
marker is found within that window, the input is left unchanged (conservative).

**`R-BULLET-STAR`**: Replaces `*` and `+` bullets with `-`. Only applies when
the preceding non-blank line ends with `:` (a field assignment) or is itself a
`-` bullet. This avoids touching `*` used in markdown emphasis or other
non-list contexts.

**`R-BARE-CODE-BLOCK`** (disabled by default): Wraps bare indented code blocks
after a `code:` field in explicit triple-backtick fences. Requires a language
identifier on the first line and at least 3 indented lines. This rule is risky
and must be explicitly enabled:
```bash
agm repair --enable-only R-BARE-CODE-BLOCK file.agm
```

## Safety Net

By default, `agm repair` re-parses the repaired text after applying all rules.
If the result still fails to parse, the **original is returned unchanged** and
exit code 2 is returned.

```
$ agm repair borderline.agm
error: repair applied rules but the result still fails to parse; original preserved.
  hint: run `agm repair borderline.agm --no-safety-net --explain` to see the attempted rewrite.
```

To see what the rules would have done regardless of parse success:

```bash
agm repair borderline.agm --no-safety-net --explain
```

## CLI Reference

```
agm repair <FILE>
    [-o <OUTPUT>]           Write to file instead of stdout (conflicts with --in-place)
    [--in-place]            Rewrite file in place (conflicts with --output)
    [--confirm]             Prompt before --in-place write (requires TTY)
    [--explain]             Print repair report to stderr
    [--report-format text|json]   Format for --explain output (default: text)
    [--disable-rule <ID>]   Disable a specific rule (repeatable; conflicts with --enable-only)
    [--enable-only <IDs>]   Enable only these comma-separated rule IDs
    [--no-safety-net]       Skip re-parse validation after repair
    [--check]               Exit 1 if any rewrites would be applied (no output written)
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success — no rewrites needed or repair succeeded |
| 1 | `--check`: rewrites would be applied |
| 2 | Safety net: repaired text still fails to parse (original preserved) |
| 2 | Unknown rule ID in `--disable-rule` or `--enable-only` |
| 3 | File I/O error or `--confirm` without an interactive terminal |

### Rule ID Flags

Disable individual rules:
```bash
agm repair --disable-rule R-SMART-QUOTES --disable-rule R-TRAILING-WS file.agm
```

Run only specific rules:
```bash
agm repair --enable-only R-CRLF,R-TRAILING-WS file.agm
```

`--enable-only` and `--disable-rule` conflict; use one or the other.

Unknown rule IDs produce exit code 2 with an error message listing valid IDs.

## agm fix — Repair + Normalize Umbrella

`agm fix` runs `agm repair` followed by `agm normalize` in a single pass:

```bash
agm fix broken.agm --explain
```

Output sections when `--explain` is used:

```
Repair stage:
  7 rewrites (3 rules): R-CRLF ×1, R-SMART-QUOTES ×4, R-TRAILING-WS ×2
  R-CRLF             ×1 (whole file)
  R-SMART-QUOTES     ×4 (lines 5, 12, 18, 23)
  R-TRAILING-WS      ×2 (lines 7, 30)

Normalize stage:
  2 rewrites, 0 warnings
  ...
```

The `--check` flag exits 1 if **either** stage would produce rewrites.

Per design decision D25: if the repair stage fails (safety net triggered),
the normalize stage is **skipped** and the original is preserved.

`agm fix` accepts all `agm repair` flags plus normalize-specific flags:
- `--rules <PATH>` — YAML normalize rules override file
- `--no-types` — skip type-level normalization
- `--no-fields` — skip field-level normalization

## Stable Rule IDs

Rule IDs are stable across releases. Renaming or removing a rule ID is
a **breaking change** and would require a major version bump. Extension of
the rule set (new rules) is additive and non-breaking.

To add custom rules, the rule set is code-level only in v1. YAML-based rule
extensibility is planned for a future version.

## Idempotency

All built-in rules are designed to be idempotent: running `agm repair` twice
on the same file produces the same output as running it once, and the second
run reports zero rewrites.

This property is verified by tests for every rule and every fixture.

## Performance Note

`agm repair` operates on raw text with no AST construction (except for the
optional safety-net re-parse). It is suitable for batch processing of large
numbers of files.
