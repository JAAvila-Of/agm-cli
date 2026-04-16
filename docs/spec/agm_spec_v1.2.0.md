# AGM Specification v1.2.0
## Agent Graph Memory

**Status:** Stable  
**Previous versions:** v0.1-draft, v0.2-draft, v1.0.0, v1.1.0  

---

## Changelog — v1.2.0 (additive)

- Added §13.11 `ticket` node type.
- Added schema row for `ticket` in §14.1.
- Added §14.4 ticket enums (`action`, `sdd_phase`, `title`, `prompt`).
- Added new diagnostic codes V029–V032 for ticket validation (see Appendix D).
- Clarified independence of `agm:` spec version and package `version:`.
- Compiler NL cues updated to map ticket phrases to `type: ticket`.
- No breaking changes. No field renamed or removed.

---


**Audience:** architects, agent-platform engineers, retrieval engineers, tooling authors, agent orchestration runtimes  
**Primary goal:** define a compact, semantically explicit, agent-optimized textual format for representing knowledge as a graph of nodes with progressive expansion, stable references, efficient loading, executable orchestration for agentic workflows, and persistent memory across executions.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Problem Statement](#2-problem-statement)
3. [Design Goals](#3-design-goals)
4. [Non-Goals](#4-non-goals)
5. [Core Mental Model](#5-core-mental-model)
6. [Conceptual Architecture](#6-conceptual-architecture)
7. [Normative Language](#7-normative-language)
8. [File Model](#8-file-model)
9. [Package Model](#9-package-model)
10. [Import Model](#10-import-model)
11. [Node Model](#11-node-model)
12. [Canonical Field Set](#12-canonical-field-set)
13. [Node Types](#13-node-types)
14. [Type Schemas](#14-type-schemas)
15. [Relationships Between Nodes](#15-relationships-between-nodes)
16. [Loading Model](#16-loading-model)
17. [Token-Efficiency Principles](#17-token-efficiency-principles)
18. [Syntax Specification](#18-syntax-specification)
19. [Parsing Rules](#19-parsing-rules)
20. [Validation Rules](#20-validation-rules)
21. [Error Model](#21-error-model)
22. [Semantics by Field](#22-semantics-by-field)
23. [Code Blocks](#23-code-blocks)
24. [Verification Contracts](#24-verification-contracts)
25. [Agent Context](#25-agent-context)
26. [Execution State](#26-execution-state)
27. [Orchestration Model](#27-orchestration-model)
28. [Memory Model](#28-memory-model)
29. [Recommended Authoring Conventions](#29-recommended-authoring-conventions)
30. [Versioning and Lifecycle](#30-versioning-and-lifecycle)
31. [Spec Versioning Policy](#31-spec-versioning-policy)
32. [Conflict Resolution](#32-conflict-resolution)
33. [Temporal and Contextual Applicability](#33-temporal-and-contextual-applicability)
34. [Retrieval and Indexing Guidance](#34-retrieval-and-indexing-guidance)
35. [Compilation from Markdown and Other Sources](#35-compilation-from-markdown-and-other-sources)
36. [Renderer Guidance](#36-renderer-guidance)
37. [JSON Canonical Form](#37-json-canonical-form)
38. [Complete Examples](#38-complete-examples)
39. [Edge Cases and Failure Modes](#39-edge-cases-and-failure-modes)
40. [Security Considerations](#40-security-considerations)
41. [Performance Considerations](#41-performance-considerations)
42. [Implementation Roadmap](#42-implementation-roadmap)
43. [Reference Templates](#43-reference-templates)
44. [Appendix A: Informal Grammar](#44-appendix-a-informal-grammar)
45. [Appendix B: Example AST Shape](#45-appendix-b-example-ast-shape)
46. [Appendix C: CLI Reference](#46-appendix-c-cli-reference)
47. [Appendix D: Error Code Registry](#47-appendix-d-error-code-registry)
48. [Appendix E: Conformance Test Requirements](#48-appendix-e-conformance-test-requirements)

---

## 1. Executive Summary

AGM (**Agent Graph Memory**) is a textual knowledge representation format designed for AI agents, retrieval pipelines, and agent runtimes that need:

- lower token consumption than long-form Markdown
- higher semantic density
- stable references to reusable knowledge units
- progressive expansion from short summary to full detail
- graph-aware loading instead of document-wide loading
- compatibility with human authoring and automated compilation
- executable orchestration of multi-agent workflows
- inline code as a first-class citizen alongside semantic metadata
- verification contracts that validate execution outcomes
- selective agent context loading for token-efficient prompt construction

AGM does **not** aim to replace Markdown as a universal documentation format. It aims to become the **agent-facing representation** of knowledge and the **orchestration protocol** for agentic execution.

The primary unit in AGM is the **node**, not the document. A node is a semantically meaningful, independently addressable knowledge unit such as:

- a rule
- a workflow
- an entity
- a decision
- an exception
- a glossary term
- an anti-pattern
- an example
- an orchestration plan

Each node has a stable identity, a type, a required summary, and optional fields that allow it to encode operational, explanatory, contextual, and executable information.

AGM v0.2 introduces four major capabilities beyond v0.1:

1. **Code blocks** (`code:`) — inline source code with target file, language, and action metadata, transforming nodes from descriptions of what to do into executable instructions.
2. **Verification contracts** (`verify:`) — post-execution checks that allow runtimes to confirm whether a node was implemented correctly.
3. **Agent context** (`agent_context:`) — declarations of what context an agent needs to execute a node, enabling token-efficient prompt construction.
4. **Orchestration model** — a new `orchestration` node type with parallel groups, execution state tracking, and scheduling semantics for multi-agent coordination.

---

## 2. Problem Statement

Markdown is widely used in AI workflows because it is easy to write, portable, and human-readable. However, in agentic systems Markdown often creates the following issues:

### 2.1 Weak semantic separation
Markdown visually structures content, but usually does not make semantic roles explicit. Rules, facts, workflows, examples, and temporary notes often coexist inside the same section.

### 2.2 High repetition cost
The same constraints or architectural facts are repeated across multiple prompts, files, or summaries.

### 2.3 Poor selective loading
Markdown encourages loading large chunks or entire sections even when only one rule or one workflow matters.

### 2.4 Weak identity and referentiality
Sections in Markdown rarely behave like stable knowledge objects with durable identifiers.

### 2.5 No built-in progressive expansion
A long explanation and a short operational representation are often bundled together.

### 2.6 Mixed human and machine concerns
Markdown is optimized primarily for people. Agents benefit from stronger structure and less rhetorical overhead.

### 2.7 No executable semantics
Markdown embeds code blocks as display elements, but they carry no metadata about target files, intended actions, or verification criteria. An agent reading a Markdown implementation plan must infer where code should go and how to validate it.

### 2.8 No orchestration primitives
Markdown has no concept of parallel execution, dependency-driven scheduling, or execution state tracking. Multi-agent workflows require these primitives to coordinate work efficiently.

AGM exists to address these shortcomings without forcing teams into heavyweight ontology systems or overly rigid schemas.

---

## 3. Design Goals

AGM is designed to satisfy the following goals.

### 3.1 Summary-first consumption
An agent should often be able to decide relevance using only a node's `summary`.

### 3.2 One node, one primary idea
The node should be the unit of retrieval, loading, reuse, and versioning.

### 3.3 Progressive expansion
Knowledge should be representable in layers:
- essential
- operational
- explanatory

### 3.4 Explicit relationships
Knowledge graphs should be first-class:
- dependency
- conflict
- replacement
- relatedness
- applicability

### 3.5 Token efficiency
The default representation should minimize rhetorical noise while preserving semantics.

### 3.6 Human-authorable
AGM should remain writable by humans without specialized tooling, even if tooling is strongly encouraged.

### 3.7 Compile-target friendly
AGM should be a reasonable output target for compilers from Markdown, ADRs, YAML, source code, or tickets.

### 3.8 Runtime friendly
AGM should support loaders, validators, indexers, and retrievers.

### 3.9 Code as first-class content
Implementation plans should embed source code with target metadata, making nodes directly executable rather than merely descriptive.

### 3.10 Orchestration native
AGM should natively express parallelism, execution state, verification, and agent context requirements so that runtimes can schedule and coordinate multi-agent work.

---

## 4. Non-Goals

AGM intentionally does **not** aim to be:

- a programming language
- a database replacement
- a universal ontology language
- a highly formal theorem-proving language
- a full substitute for human-friendly long-form documentation
- a binary format
- a graph database query language
- a CI/CD pipeline definition (though it can inform one)
- a full build system or task runner

If a system needs rich relational inference, rule engines, or ontology reasoning, AGM can coexist with those tools but should not try to become them.

---

## 5. Core Mental Model

The central mental model is:

> A knowledge base is a graph of semantically typed nodes, each optimized for short identification, partial loading, and controlled expansion.

Each node should answer three progressively richer questions:

1. **What is it?** → `summary`
2. **How does it operate?** → `items`, `steps`, `fields`, `input`, `output`
3. **Why / when / with what tradeoffs?** → `detail`, `rationale`, `tradeoffs`, `resolution`, `notes`
4. **How is it executed and verified?** → `code`, `verify`, `agent_context`, `target`, `execution_status`

This leads to four knowledge layers:

### 5.1 Essential layer
Minimal representation used for navigation and selection.
- `node`
- `type`
- `summary`
- optionally `priority`, `stability`, `depends`

### 5.2 Operational layer
Enough to act, implement, or reason procedurally.
- `items`
- `steps`
- `fields`
- `input`
- `output`

### 5.3 Explanatory layer
Enough to teach, justify, troubleshoot, or compare.
- `detail`
- `rationale`
- `tradeoffs`
- `resolution`
- `examples`
- `notes`

### 5.4 Executable layer
Enough for a runtime to execute, verify, and track progress.
- `code`
- `verify`
- `agent_context`
- `target`
- `execution_status`
- `executed_by`
- `executed_at`
- `execution_log`

---

## 6. Conceptual Architecture

AGM fits into a broader pipeline like this:

1. **Source documents**
   - Markdown
   - ADRs
   - issue tickets
   - code comments
   - structured schemas
2. **Extractor / compiler**
   - identifies semantic units
   - generates nodes
   - assigns types and relationships
3. **Validator**
   - checks integrity, references, cycles, field compatibility
4. **Indexer**
   - indexes summaries, fields, tags, relationships
5. **Loader**
   - loads `summary`, `operational`, `executable`, or `full`
6. **Scheduler**
   - reads dependency graph and parallel groups
   - identifies nodes whose dependencies are satisfied
   - dispatches ready nodes to agents respecting concurrency limits
7. **Context builder**
   - for each node to execute, constructs agent prompt from `agent_context`
   - loads only referenced nodes and files, not the entire package
   - injects `system_hint` for stack/framework guidance
8. **Agent runtime**
   - selects nodes based on task
   - expands only when needed
   - executes `code` blocks against target files
9. **Verifier**
   - runs `verify` checks after agent execution
   - updates `execution_status` on the node
   - unblocks dependent nodes or flags failures
10. **Renderer**
    - optionally emits Markdown, HTML, JSON, or system prompt bundles

---

## 7. Normative Language

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** in this document are to be interpreted as follows:

- **MUST / MUST NOT**: mandatory for compliance
- **SHOULD / SHOULD NOT**: recommended unless there is a justified reason otherwise
- **MAY**: optional

---

## 8. File Model

An AGM file contains:

1. a **header**
2. one or more **node blocks**

A minimal valid file:

```agm
agm: 1
package: demo.sample
version: 0.1.0

node demo.rule
type: rules
summary: do not expose secrets to the browser
items:
  - no_browser_secrets
```

### 8.1 File-level concerns
The file itself is not the main unit of consumption. It is a container for nodes. A loader MAY load one file, multiple files, or a package assembled from many files.

### 8.2 Multi-file packages
A package MAY be split into multiple `.agm` files as long as:
- package identity is preserved
- node IDs remain unique within the package namespace
- imports and references can be resolved

---

## 9. Package Model

The header defines package-wide metadata.

### 9.1 Required header fields

#### `agm`
Format version.
- MUST be present
- MUST be a version string matching `MAJOR.MINOR` (e.g., `agm: 1.0`)

#### `package`
Logical namespace of the file or package.
- MUST be present
- MUST match the pattern `[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)*`
- SHOULD be stable across related files

#### `version`
Version of the package content, not necessarily the language version.
- MUST be present
- MUST follow semver `MAJOR.MINOR.PATCH` format

### 9.2 Optional header fields

#### `title`
Human-readable package title.

#### `owner`
Owning team, person, or system.

#### `imports`
List of imported packages or external namespaces.

#### `default_load`
Recommended loading mode.
Suggested values:
- `summary`
- `operational`
- `full`

#### `description`
Brief package-level explanation.

#### `tags`
Package-wide tags.

#### `status`
Suggested values:
- `active`
- `draft`
- `deprecated`

#### `load_profiles`
Named loading profiles with filter expressions and estimated token budgets. Allows runtimes to load only relevant subsets of a package.

Example:

```agm
load_profiles:
  minimal:
    filter: "priority in [critical] AND type in [decision, rules]"
    estimated_tokens: 1200
  operational:
    filter: "priority in [critical, high] AND type in [workflow, decision, facts]"
    estimated_tokens: 3800
  executable:
    filter: "priority in [critical, high] AND type in [workflow, orchestration]"
    estimated_tokens: 4200
  full:
    filter: "*"
    estimated_tokens: 6500
  debug:
    filter: "execution_status in [failed, blocked] + transitive_deps"
    estimated_tokens: variable
```

Profiles are informational. Runtimes MAY use them for budget-aware loading. The `estimated_tokens` values SHOULD be updated by tooling when the package changes.

#### `target_runtime`
Optional identifier for the runtime that executes this package.

Example:

```agm
target_runtime: octopus
```

### 9.3 Example package header

```agm
agm: 1
package: auth.platform
version: 0.2.0
title: Authentication Platform Core Knowledge
owner: security-platform
imports: [shared.security, shared.http]
default_load: summary
status: active
description: core architecture, rules, workflows and decisions for authentication flows
tags: [auth, security, oidc]
```

---

## 10. Import Model

AGM packages MAY reference nodes from other packages. The import model defines how cross-package references are resolved.

### 10.1 Import declaration

Imports are declared in the package header:

```agm
imports: [shared.security@^1.0.0, shared.http@2.0.0]
```

### 10.2 Import syntax

Each import entry MUST follow the format:

```
package_name@version_constraint
```

Where:
- `package_name` MUST match the `package` field of the imported package
- `version_constraint` MUST follow semver range syntax: exact (`1.0.0`), caret (`^1.0.0`), tilde (`~1.0.0`), or wildcard (`1.*`)
- If no version constraint is specified, the latest available version is used

### 10.3 Resolution

A runtime or tooling MUST resolve imports using the following search order:
1. Local file system paths configured in the project (e.g., `.agm/packages/`)
2. A package registry if configured
3. Fail with error `AGM-E020: unresolved import`

### 10.4 Cross-package references

Once imported, nodes from external packages can be referenced in relationship fields:

```agm
depends: [shared.security.auth.rules]
```

A validator MUST verify that cross-package references resolve to existing nodes in the imported package. A validator MUST verify that the imported package version satisfies the declared constraint.

### 10.5 Namespace isolation

Node IDs are scoped to their package. Two packages MAY have nodes with the same local ID without conflict. When referencing a node from an imported package, the full qualified ID (package + node) MUST be used.

### 10.6 Circular imports

Circular imports (package A imports B which imports A) MUST be rejected by validators.

### 10.7 Import-only loading

A loader MAY load only the summary layer of imported packages to minimize token consumption. Full expansion of imported nodes SHOULD only occur when explicitly requested.

---

---

## 11. Node Model

A node is the atomic or near-atomic knowledge unit in AGM.

### 11.1 Required node fields
Each node MUST include:
- `node`
- `type`
- `summary`

### 11.2 Node identity
Node identity is expressed in the line:

```agm
node auth.login
```

The identity:
- MUST be unique within the effective package scope
- SHOULD be stable over time
- MUST match the pattern `[a-z][a-z0-9]*([.-][a-z][a-z0-9]*)*`
- SHOULD reflect semantic hierarchy

### 11.3 Recommended node size
A node SHOULD represent one primary idea. It MAY include supporting detail, but SHOULD NOT become a mini-book.

A good node:
- can be summarized in one line
- has a coherent single purpose
- does not require loading unrelated subtopics

### 11.4 Overloaded nodes
If a node contains:
- multiple major workflows
- many unrelated edge cases
- too many examples
- mutually inconsistent concerns

it SHOULD be split.

---

## 12. Canonical Field Set

This section defines the canonical AGM fields.

### 12.1 Required fields
- `node`
- `type`
- `summary`

### 12.2 Strongly recommended control fields
- `priority`
- `stability`

### 12.3 Relationship fields
- `depends`
- `related_to`
- `replaces`
- `conflicts`
- `see_also`

### 12.4 Operational fields
- `items`
- `steps`
- `fields`
- `input`
- `output`

### 12.5 Explanatory fields
- `detail`
- `rationale`
- `tradeoffs`
- `resolution`
- `examples`
- `notes`

### 12.6 Executable fields
- `code`
- `verify`
- `agent_context`
- `target`

### 12.7 Execution state fields
- `execution_status`
- `executed_by`
- `executed_at`
- `execution_log`
- `retry_count`

### 12.8 Orchestration fields
- `parallel_groups`

### 12.9 Context fields
- `scope`
- `applies_when`
- `valid_from`
- `valid_until`
- `confidence`
- `status`
- `tags`
- `aliases`
- `keywords`

---

## 13. Node Types

AGM defines a core set of node types.

### 13.1 `facts`
Used for relatively objective, descriptive, stable statements.

Typical use:
- stack descriptions
- capability inventories
- environmental facts
- protocol facts

Typical fields:
- `items`
- `detail`

Example:

```agm
node auth.stack
type: facts
summary: next15 frontend; net8 backend; identityserver4 local idp
items:
  - frontend=next15
  - backend=net8
  - local_idp=identityserver4
```

### 13.2 `rules`
Used for invariants, constraints, policies, or requirements.

Typical fields:
- `items`
- `detail`

Example:

```agm
node auth.constraints
type: rules
summary: no browser tokens; sensitive calls from server only
items:
  - no_client_tokens
  - server_side_sensitive_calls
```

### 13.3 `workflow`
Used for ordered, procedural behavior.

Typical fields:
- `input`
- `output`
- `steps`
- `detail`

Example:

```agm
node auth.login
type: workflow
input: [host, return_url]
output: [redirect_url, sid_cookie]
summary: resolve tenant -> redirect to provider -> callback -> create sid
steps:
  - resolve tenant
  - redirect to provider
  - validate callback
  - create sid
```

### 13.4 `entity`
Used for domain concepts or schema-like structures.

Typical fields:
- `fields`
- `detail`

Example:

```agm
node auth.session
type: entity
summary: opaque sid maps to server-side token pair and metadata
fields:
  - sid: opaque string
  - user_id: string
  - expires_at: datetime
```

### 13.5 `decision`
Used for architectural, product, or procedural decisions.

Typical fields:
- `rationale`
- `tradeoffs`
- `replaces`
- `conflicts`

### 13.6 `exception`
Used for edge cases, exceptional flows, deviations, or constraints under special conditions.

Typical fields:
- `detail`
- `resolution`

### 13.7 `example`
Used for examples, sample payloads, sample flows, or sample scenarios.

Typical fields:
- `detail`
- `depends`

### 13.8 `glossary`
Used for concise definitions of terms.

Typical fields:
- `detail`
- `aliases`

### 13.9 `anti_pattern`
Used for patterns to avoid.

Typical fields:
- `detail`
- `related_to`
- `conflicts`

### 13.10 `orchestration`
Used for coordinating multi-agent execution of workflow nodes.

An orchestration node defines execution groups, parallelism strategy, and scheduling constraints. It does not contain implementation logic itself; it references other nodes that do.

Typical fields:
- `parallel_groups`
- `depends`
- `detail`

Example:

```agm
node rollout.orchestration
type: orchestration
stability: medium
priority: critical
summary: coordinate backend-first rollout with parallel frontend work after backend stabilizes
parallel_groups:
  - group: 1-schema
    nodes: [migration.025.schema, migration.025.registration]
    strategy: sequential
  - group: 2-backend-models
    nodes: [backend.model.kanban-column, backend.trait.repository]
    strategy: parallel
  - group: 3-backend-impl
    nodes: [backend.repo, backend.commands.validation, backend.commands.get-by-phase]
    strategy: parallel
    requires: [2-backend-models]
  - group: 4-frontend
    nodes: [frontend.types, frontend.commands, frontend.store]
    strategy: sequential
    requires: [3-backend-impl]
  - group: 5-ui
    nodes: [frontend.column-config, frontend.new-task, frontend.badge]
    strategy: parallel
    requires: [4-frontend]
  - group: 6-testing
    nodes: [testing.rust, testing.frontend, testing.integration]
    strategy: parallel
    requires: [5-ui]
```

#### Parallel group fields

| Field | Required | Description |
|---|---|---|
| `group` | yes | Unique group identifier within the orchestration node |
| `nodes` | yes | List of node IDs to execute in this group |
| `strategy` | yes | `sequential` or `parallel` |
| `requires` | no | List of group IDs that must complete before this group starts |
| `max_concurrency` | no | Maximum number of agents for parallel groups (default: unlimited) |

### 13.11 `ticket`
Used for ticket-like artifacts: units of work, proposals, or mutations to existing tickets.
A ticket represents a single unit of work; execution decomposition belongs on a separate
`workflow` node linked via `depends`.

Tickets MAY carry `execution_status` for lifecycle tracking (`pending` → `in_progress` → `completed`).
The `agm-cli` runtime does NOT transition the status automatically; the owning backend does.

`ticket_id` is an opaque identifier from the ticket backend's namespace. It is not validated
as resolving to a node within the current AGM graph.

Typical fields:
- `title`
- `description`
- `priority`
- `action`
- `sdd_phase`
- `prompt`
- `assignee`
- `labels`
- `ticket_id` (when action ≠ create)

Example — create:

```agm
node octopus.ticket.add-auth
type: ticket
title: Add OAuth2 login flow
description:
  Add Google OAuth2 login to the dashboard, including session handling
  and CSRF protection on the callback.
priority: high
action: create
sdd_phase: backlog
labels: [auth, security]
prompt:
  Design a minimal OAuth2 flow that reuses our existing session cookies.
  Emit a workflow node for the implementation steps.
summary: add OAuth2 login with CSRF-safe callback
```

Example — edit an existing ticket:

```agm
node octopus.ticket.add-auth.edit-1
type: ticket
title: Add OAuth2 login flow
description: scope reduced to provider=Google only
priority: medium
action: edit
ticket_id: octopus.ticket.add-auth
summary: reduce scope of auth ticket to Google only
```

#### Relationship to `workflow`
A ticket MAY link to a `workflow` node via `depends` when multi-step decomposition is
needed. Tickets themselves MUST NOT use `steps:` or `parallel_groups:` — those belong
on `workflow` and `orchestration` respectively.

### 13.12 Extending types
Projects MAY define additional node types. If they do:
- the type SHOULD be documented
- tooling SHOULD either support it or degrade gracefully
- authors SHOULD avoid unbounded proliferation of near-duplicate types

---

## 14. Type Schemas

Each node type has a schema that defines which fields are required, recommended, allowed, and disallowed. Validators MUST enforce required fields. Validators SHOULD warn on missing recommended fields. Validators MUST reject disallowed fields.

### 14.1 Schema definitions

| Type | Required | Recommended | Allowed | Disallowed |
|---|---|---|---|---|
| `facts` | `summary` | `items`, `stability`, `priority` | `detail`, `tags`, `fields` | `steps`, `rationale`, `resolution`, `parallel_groups` |
| `rules` | `summary`, `items` | `stability`, `priority` | `detail`, `scope`, `applies_when` | `steps`, `fields`, `parallel_groups` |
| `workflow` | `summary` | `steps`, `input`, `output`, `stability`, `priority` | `detail`, `code`, `code_blocks`, `verify`, `agent_context`, `target` | `fields`, `parallel_groups` |
| `entity` | `summary`, `fields` | `stability`, `priority` | `detail`, `related_to` | `steps`, `rationale`, `resolution`, `parallel_groups`, `code` |
| `decision` | `summary`, `rationale` | `stability`, `priority`, `tradeoffs` | `detail`, `depends`, `conflicts`, `replaces` | `steps`, `fields`, `parallel_groups`, `code` |
| `exception` | `summary` | `resolution`, `stability`, `priority` | `detail`, `depends` | `steps`, `fields`, `rationale`, `parallel_groups`, `code` |
| `example` | `summary` | `depends` | `detail`, `code`, `code_blocks` | `rationale`, `resolution`, `parallel_groups` |
| `glossary` | `summary` | `aliases` | `detail`, `related_to` | `steps`, `fields`, `rationale`, `resolution`, `code`, `parallel_groups` |
| `anti_pattern` | `summary` | `detail`, `conflicts` | `related_to`, `resolution` | `steps`, `fields`, `rationale`, `code`, `parallel_groups` |
| `orchestration` | `summary`, `parallel_groups` | `detail` | `depends` | `steps`, `items`, `fields`, `code`, `code_blocks`, `verify`, `rationale`, `resolution` |
| `ticket` | `summary`, `title`, `description`, `priority` | `action`, `sdd_phase`, `labels` | `prompt`, `assignee`, `ticket_id`, `detail`, `agent_context`, `code_blocks` | `steps`, `parallel_groups`, `fields`, `input`, `output`, `rationale`, `resolution` |

### 14.2 Universal fields

The following fields are allowed on all node types and are never listed in schemas above:
- `type`, `summary` (required on all)
- `priority`, `stability`, `confidence`, `status`, `tags`, `keywords`, `aliases`
- `scope`, `applies_when`, `valid_from`, `valid_until`
- `depends`, `related_to`, `replaces`, `conflicts`, `see_also`
- `notes`
- `execution_status`, `executed_by`, `executed_at`, `execution_log`, `retry_count`
- `memory`

### 14.3 Schema enforcement levels

| Level | Behavior |
|---|---|
| `strict` | Validators MUST reject nodes with disallowed fields or missing required fields |
| `standard` | Validators MUST reject missing required fields. Validators MUST warn on disallowed fields. |
| `permissive` | Validators MUST warn on missing required fields. Disallowed fields are silently accepted. |

The default enforcement level is `standard`. Projects MAY configure enforcement level via a `.agmrc` configuration file or CLI flag.

### 14.4 Ticket field enums (v1.2.0)

#### 14.4.1 `action`
Declares the intent of the ticket emission.

| Value | Meaning | `ticket_id` required? |
|---|---|---|
| `create` | New ticket (default when absent) | no |
| `edit`   | Modify an existing ticket | yes |
| `close`  | Close an existing ticket | yes |
| `archive`| Archive an existing ticket | yes |
| `split`  | Split an existing ticket into child tickets | yes |
| `link`   | Link two existing tickets | yes |

Validators MUST reject a ticket with `action ∈ {edit, close, archive, split, link}` that
lacks `ticket_id`.

#### 14.4.2 `sdd_phase`
Suggests the SDD pipeline phase the ticket belongs to. Default: `backlog`.

Valid values: `backlog`, `explore`, `propose`, `spec`, `design`, `tasks`, `apply`, `verify`, `archive`.

The value is a hint. A backend MAY override it based on project policy.

#### 14.4.3 `title`
Human-readable ticket title. MUST be ≤ 200 characters. Distinct from `summary`, which remains
the AGM-universal one-line identity.

#### 14.4.4 `prompt`
Optional multi-line block string — the execution prompt intended for downstream agents.
Supports the standard AGM block literal syntax (`prompt:\n  <indented text>`).

Consumers SHOULD redact `prompt` before logging and SHOULD NOT render tickets into prompts
intended for third-party LLM providers without explicit user consent.

#### 14.4.5 Spec version vs package version (v1.2.0 clarification)

The `agm:` header field identifies the **specification version** the file targets. The `version:`
field identifies the **package content version** and is independent of the spec version. A consumer
MAY upgrade their `agm:` value to `1.2` to use `type: ticket` without bumping their `version:`.

---

---

## 15. Relationships Between Nodes

Relationships make AGM a graph rather than a flat set of records.

### 15.1 `depends`
Strong semantic dependency.

Use when:
- the node is incomplete without the referenced node
- a loader should usually bring the dependency for operational reasoning

Example:

```agm
depends: [auth.constraints, auth.session]
```

### 15.2 `related_to`
Soft relation.

Use when:
- the node is connected conceptually
- the relation is useful for exploration
- the other node is not required for understanding basic semantics

### 15.3 `replaces`
Indicates semantic replacement or supersession.

Use when:
- a newer decision or workflow replaces an earlier one
- a migration invalidates an old node

### 15.4 `conflicts`
Indicates incompatible coexistence.

Use when:
- two active nodes should not be simultaneously treated as true in the same scope

### 15.5 `see_also`
Suggested navigation targets.

### 15.6 Relationship semantics table

| Field | Strength | Load implication | Typical use |
|---|---:|---:|---|
| `depends` | strong | usually yes | prerequisites |
| `related_to` | medium | no | conceptual links |
| `replaces` | strong | no | versioning |
| `conflicts` | strong | no | incompatibility |
| `see_also` | weak | no | discoverability |

---

## 16. Loading Model

AGM is designed for partial loading.

### 16.1 Summary load
Loads:
- `node`
- `type`
- `summary`
- optionally `priority`, `stability`, `depends`, `tags`

Use cases:
- retrieval candidate ranking
- navigation
- overview generation

### 16.2 Operational load
Loads summary load plus:
- `input`
- `output`
- `items`
- `steps`
- `fields`

Use cases:
- implementation guidance
- procedural reasoning
- structured answer generation

### 16.3 Full load
Loads operational load plus:
- `detail`
- `rationale`
- `tradeoffs`
- `resolution`
- `examples`
- `notes`

Use cases:
- deep analysis
- architecture discussion
- debugging
- educational explanation

### 16.4 Executable load
Loads operational load plus:
- `code`
- `verify`
- `agent_context`
- `target`
- `execution_status`

Use cases:
- agent execution of implementation nodes
- orchestration scheduling
- verification after execution

This load mode is typically used per-node by a scheduler, not for bulk loading.

### 16.5 Expansion policy
A runtime SHOULD prefer:
1. summary
2. operational if needed
3. executable for agent dispatch
4. full only when justified

This is one of the primary token-efficiency mechanisms.

### 16.6 Profile-based loading
When `load_profiles` is defined in the package header, runtimes MAY use profile filters to select subsets of nodes for loading. Profile filters are evaluated against node metadata fields (`priority`, `type`, `stability`, `execution_status`, `tags`).

A runtime MUST:
- default to the `default_load` profile if no specific profile is requested
- support the `debug` profile for post-failure inspection
- recalculate `estimated_tokens` when package content changes

---

## 17. Token-Efficiency Principles

AGM is not merely a schema. It encodes an efficiency strategy.

### 17.1 Summary-first
Every node MUST have a dense, discriminative `summary`.

### 17.2 Reference over repetition
If a rule exists once, other nodes SHOULD reference it rather than restate it.

### 17.3 Examples off the default path
Examples SHOULD often live in separate nodes or separate fields that are not part of summary load.

### 17.4 One major idea per node
Node granularity directly affects retrieval precision and token waste.

### 17.5 Stable identity
Stable IDs let runtimes cache and reuse knowledge without re-reading all explanatory text.

### 17.6 Compact field semantics
AGM SHOULD favor direct, high-signal language over rhetorical prose.

Bad:
> It is important to note that one must be very careful not to expose access tokens.

Better:
> access tokens MUST remain server-side

### 17.7 Split stable from volatile
High-stability knowledge SHOULD be separated from volatile operational exceptions.

---

## 18. Syntax Specification

This section defines a line-oriented textual syntax for AGM v0.1.

### 18.1 General form
AGM is a UTF-8 text format composed of:
- scalar fields
- list fields
- indented multiline blocks
- node declarations

### 18.2 Scalar field
Form:

```agm
key: value
```

Examples:

```agm
type: workflow
priority: critical
summary: resolve tenant -> redirect -> callback -> create sid
```

### 18.3 Inline list
Form:

```agm
key: [a, b, c]
```

Examples:

```agm
tags: [auth, security, oidc]
depends: [auth.constraints, auth.session]
```

### 18.4 Indented list
Form:

```agm
items:
  - first item
  - second item
```

### 18.5 Indented block
Form:

```agm
detail:
  First line of detail.
  Second line of detail.
```

All indented lines belong to the block until:
- indentation ends, or
- a new field at the parent indentation appears, or
- a new `node ...` begins

### 18.6 Node declaration
Form:

```agm
node some.identifier
```

This begins a new node and closes the previous one.

### 18.7 Comments
#-prefixed lines are comments. Comments: `#` if a parser chooses to. If comments are supported:
- they SHOULD be ignored by semantic loaders
- they SHOULD NOT appear inside multiline blocks unless explicitly escaped or treated literally by the parser design

For strict portability, teams MAY choose to prohibit comments in v0.1.

### 18.8 Blank lines
Blank lines MAY appear between fields and nodes.

---

## 19. Parsing Rules

### 19.1 Whitespace
Indentation MUST use spaces. Tabs MUST be rejected by the parser (error `AGM-P004`).

### 19.2 Field detection
A line at parent indentation with the pattern `name:` starts a field.
A line matching `node ` starts a node declaration.

### 19.3 Multiline block termination
A multiline block ends when indentation returns to the parent field level.

### 19.4 Duplicate fields
A parser MUST reject duplicate fields within the same node (error `AGM-P006`).

### 19.5 Unknown fields
Unknown fields MAY be preserved by permissive parsers, but validators SHOULD flag them unless project extensions document them.

### 19.6 Ordering
Fields MAY appear in any order, but a renderer SHOULD use a canonical order for readability.

Recommended order:
1. `type`
2. control fields (`status`, `stability`, `priority`, `confidence`)
3. relation fields
4. `input`, `output`
5. `summary`
6. operational fields
7. explanatory fields
8. context fields

---

## 20. Validation Rules

### 20.1 File validation
A valid file MUST contain:
- `agm`
- `package`
- `version`
- at least one node

### 20.2 Node validation
Each node MUST contain:
- `type`
- `summary`

### 20.3 ID uniqueness
Node IDs MUST be unique within the effective package scope.

### 20.4 Reference validation
References in:
- `depends`
- `related_to`
- `replaces`
- `conflicts`
- `see_also`

MUST resolve to:
- a node in the package, or
- an imported package node, if cross-package resolution is enabled

### 20.5 Cycle validation
Hard cycles in `depends` MUST be rejected (error `AGM-V005`).

Soft cycles in `related_to` MAY be allowed.

### 20.6 Field compatibility
Validation SHOULD warn on suspicious combinations, for example:
- `workflow` without `steps`
- `entity` without `fields`
- `decision` without `rationale`
- `exception` without `resolution`

### 20.7 Summary quality
Validation SHOULD warn when:
- summary is empty
- summary is excessively long
- summary appears identical to detail
- summary contains only generic phrases with low discriminative value

### 20.8 Temporal validity
If both `valid_from` and `valid_until` exist, validators MUST verify that `valid_from <= valid_until` (error `AGM-V007`).

### 20.9 Status conflicts
A node marked `superseded` or `deprecated` SHOULD generally include `replaces`, `superseded_by`, or a similar relation if the project profile requires it.

### 20.10 Code block validation
Validation SHOULD check:
- `code:` and `code_blocks:` MUST include `lang`, `target`, `action`, and `body`
- `action: replace` MUST include `old`
- `action: insert_before` and `action: insert_after` MUST include `anchor`
- `lang` SHOULD be a recognized language identifier
- `target` SHOULD be a relative path (no leading `/` or `..` traversal)
- `body` MUST NOT be empty

### 20.11 Verify field validation
Validation SHOULD check:
- each verify entry MUST include `type`
- `type: command` MUST include `run` and `expect`
- `type: file_exists` and `type: file_contains` MUST include `file`
- `type: file_contains` and `type: file_not_contains` MUST include `pattern`
- `type: node_status` MUST include `node` and `status`
- referenced nodes in `type: node_status` MUST exist in the package

### 20.12 Agent context validation
Validation SHOULD check:
- `load_nodes` references MUST resolve to existing nodes in the package
- `load_files` paths SHOULD be relative
- `max_tokens` SHOULD be a positive integer

### 20.13 Orchestration validation
Validation SHOULD check:
- `orchestration` nodes MUST include `parallel_groups`
- each group MUST include `group`, `nodes`, and `strategy`
- `strategy` MUST be `sequential` or `parallel`
- node references in `nodes` MUST resolve to existing nodes
- group references in `requires` MUST resolve to groups within the same orchestration node
- `requires` MUST NOT contain cycles between groups
- `max_concurrency` MUST be a positive integer when present

### 20.14 Execution state validation
Validation SHOULD check:
- `execution_status` MUST be one of: `pending`, `ready`, `in_progress`, `completed`, `failed`, `blocked`, `skipped`
- `executed_at` SHOULD be a valid ISO 8601 timestamp
- `retry_count` SHOULD be a non-negative integer
- a node with `execution_status: completed` SHOULD have `executed_by` and `executed_at` set

---

## 21. Error Model

AGM defines a standard set of error codes for parsers, validators, and runtimes. Consistent error codes enable tooling interoperability and clear diagnostics.

### 21.1 Error structure

Every error MUST include:
- `code`: a unique error identifier (format: `AGM-XNNN`)
- `severity`: `error`, `warning`, or `info`
- `message`: human-readable description
- `location`: file path, line number, and optionally node ID

### 21.2 Severity levels

| Severity | Meaning | Parser behavior |
|---|---|---|
| `error` | Invalid AGM — MUST be rejected | Parser MUST stop or collect all errors before failing |
| `warning` | Valid but suspicious — SHOULD be reviewed | Parser MUST continue; SHOULD report |
| `info` | Informational — no action required | Parser MUST continue; MAY report |

### 21.3 Error code registry

Error codes follow the pattern `AGM-XNNN` where X is the category letter and NNN is a three-digit number.

| Prefix | Category |
|---|---|
| `AGM-P` | Parse errors (syntax) |
| `AGM-V` | Validation errors (semantics) |
| `AGM-R` | Runtime errors (execution) |
| `AGM-I` | Import errors (cross-package) |

See [Appendix D: Error Code Registry](#47-appendix-d-error-code-registry) for the complete list.

### 21.4 Error output format

Tooling SHOULD output errors in a consistent format:

```
file.agm:42 [AGM-V003] error: duplicate node ID "auth.login"
file.agm:87 [AGM-V010] warning: workflow "deploy.step3" has no steps field
```

Tooling MAY also support structured output (JSON) for programmatic consumption:

```json
{
  "code": "AGM-V003",
  "severity": "error",
  "message": "duplicate node ID",
  "file": "file.agm",
  "line": 42,
  "node": "auth.login"
}
```

---

---

## 22. Semantics by Field

This section defines the meaning and intended use of each canonical field.

### 22.1 `summary`
The shortest semantically useful representation of the node.

Requirements:
- MUST exist
- SHOULD be dense
- SHOULD be discriminative
- SHOULD avoid filler language

### 22.2 `type`
The semantic role of the node. See [Node Types](#12-node-types).

### 22.3 `priority`
Relative importance for loading when budget is constrained.

Suggested values:
- `critical`
- `high`
- `normal`
- `low`

### 22.4 `stability`
Expected rate of change.

Suggested values:
- `high`
- `medium`
- `low`
- `volatile`

Use cases:
- cache strategy
- context reuse decisions
- freshness prioritization

### 22.5 `confidence`
Reliability or certainty.

Suggested values:
- `high`
- `medium`
- `low`
- `inferred`
- `tentative`

### 22.6 `status`
Lifecycle status.

Suggested values:
- `active`
- `draft`
- `deprecated`
- `superseded`

### 22.7 `depends`
Strong semantic prerequisites.

### 22.8 `related_to`
Soft related navigation.

### 22.9 `replaces`
The node supersedes another node or set of nodes.

### 22.10 `conflicts`
Indicates semantic incompatibility.

### 22.11 `see_also`
Reader or loader hint for related exploration.

### 22.12 `items`
Compact unordered or lightly ordered points. Often used for `facts`, `rules`, or `glossary`.

### 22.13 `steps`
Ordered procedural sequence. Intended mainly for `workflow`.

### 22.14 `fields`
Entity or schema attributes.

Recommended form:
```agm
fields:
  - field_name: type or description
```

### 22.15 `input`
Expected input arguments or prerequisites.

### 22.16 `output`
Expected outputs, results, or emissions.

### 22.17 `detail`
Longer descriptive explanation. Explanatory, not mandatory for short loading.

### 22.18 `rationale`
Why a decision or approach exists.

### 22.19 `tradeoffs`
Costs, downsides, and benefits.

### 22.20 `resolution`
How to handle an exception or edge case.

### 22.21 `examples`
Illustrative examples. Often better as dedicated `example` nodes if large.

### 22.22 `notes`
Supplemental comments that should not be interpreted as primary semantics.

### 22.23 `scope`
Applicability scope such as:
- environment
- tenant profile
- product tier
- deployment mode

### 22.24 `applies_when`
Condition-like applicability hint. In v0.1 this SHOULD remain simple text or simple scalar predicates.

### 22.25 `valid_from`, `valid_until`
Temporal applicability bounds.

### 22.26 `aliases`
Synonyms or alternate names for retrieval.

### 22.27 `keywords`
Additional retrieval cues.

---

## 23. Code Blocks

*(New in v0.2)*

Code blocks make AGM nodes executable. A node MAY contain one or more `code:` fields that embed source code with metadata about where and how to apply it.

### 23.1 Purpose

Without code blocks, an agent reading an AGM implementation plan must generate code from prose descriptions. This introduces hallucination risk and requires the agent to infer target files, languages, and insertion points. Code blocks eliminate this gap by providing the actual code alongside semantic metadata.

### 23.2 Syntax

A code block uses an indented structured field:

```agm
code:
  lang: rust
  target: src-tauri/src/storage/sqlite/kanban_column_repo.rs
  action: append
  body: |
    fn get_by_sdd_phase(&self, project_id: &str, phase: &str) -> Result<KanbanColumn, StorageError> {
        let conn = self.pool.get()?;
        conn.query_row(
            "SELECT * FROM kanban_columns WHERE project_id = ?1 AND sdd_phase = ?2",
            params![project_id, phase],
            Self::row_to_column,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                entity: "kanban_column",
                id: format!("{project_id}/sdd_phase:{phase}"),
            },
            other => StorageError::Sqlite(other),
        })
    }
```

### 23.3 Code block fields

| Field | Required | Description |
|---|---|---|
| `lang` | yes | Programming language identifier (e.g., `rust`, `typescript`, `sql`, `svelte`, `html`) |
| `target` | yes | Relative file path where this code should be applied |
| `action` | yes | How to apply the code to the target file |
| `body` | yes | The actual source code, using YAML block scalar syntax (`\|`) |
| `anchor` | no | A string or pattern in the target file indicating where to insert (used with `insert_before`, `insert_after`) |
| `old` | no | The existing code to replace (used with `replace` action) |

### 23.4 Action values

| Action | Meaning |
|---|---|
| `create` | Create the target file with this content. Fail if file already exists unless `overwrite: true` is set. |
| `append` | Append the code to the end of the target file. |
| `prepend` | Insert the code at the beginning of the target file. |
| `replace` | Replace the text matched by `old` with `body`. Fail if `old` is not found or matches more than once. |
| `insert_before` | Insert the code before the line matched by `anchor`. |
| `insert_after` | Insert the code after the line matched by `anchor`. |
| `full` | Replace the entire file content with `body`. |

### 23.5 Multiple code blocks

A node MAY contain multiple code blocks when it touches several files. Use `code_blocks:` as an indented list:

```agm
code_blocks:
  - lang: rust
    target: src-tauri/src/models/kanban_column.rs
    action: insert_after
    anchor: "pub struct KanbanColumn {"
    body: |
      pub sdd_phase: Option<String>,
  - lang: rust
    target: src-tauri/src/storage/traits.rs
    action: insert_before
    anchor: "}"
    body: |
      fn get_by_sdd_phase(&self, project_id: &str, phase: &str) -> Result<KanbanColumn, StorageError>;
```

### 23.6 Code block guidelines

- Code SHOULD be syntactically valid in the declared language.
- `target` MUST be a relative path from the project root (no leading `/` or `..` traversal).
- `body` SHOULD contain only the code to apply, not surrounding context (surrounding context belongs in `anchor` or `old`).
- Nodes with code blocks SHOULD also have `steps` or `summary` that describe the intent in natural language, so that the code block supplements rather than replaces semantic understanding.
- Code blocks MUST NOT contain secrets, credentials, or sensitive configuration values. Use placeholders like `${ENV_VAR}` instead.

### 23.7 Relationship to examples

The existing `example` node type remains appropriate for illustrative code that demonstrates usage patterns. Code blocks (`code:`) are for **implementation code** — code that should be applied to a codebase. The distinction:

- `example` node → "here is how you would use this API"
- `code:` field → "here is the code to write to `src/repo.rs`"

---

## 24. Verification Contracts

*(New in v0.2)*

Verification contracts allow runtimes to confirm whether a node was executed correctly. They transform AGM from a descriptive format into a verifiable one.

### 24.1 Purpose

After an agent executes a node's code blocks, the runtime needs to know whether the execution succeeded. Verification contracts provide machine-checkable assertions that go beyond "the agent said it's done."

### 24.2 Syntax

```agm
verify:
  - type: command
    run: cargo check --manifest-path src-tauri/Cargo.toml
    expect: exit_code_0
  - type: command
    run: cargo test --manifest-path src-tauri/Cargo.toml test_get_by_sdd_phase
    expect: exit_code_0
  - type: file_contains
    file: src-tauri/src/storage/sqlite/kanban_column_repo.rs
    pattern: "fn get_by_sdd_phase"
  - type: file_exists
    file: src-tauri/src/storage/sqlite/migrations/025_column_sdd_phase.sql
```

### 24.3 Verification types

| Type | Fields | Description |
|---|---|---|
| `command` | `run`, `expect` | Execute a shell command and check exit code or output |
| `file_exists` | `file` | Assert that a file exists at the given path |
| `file_contains` | `file`, `pattern` | Assert that a file contains a string or regex pattern |
| `file_not_contains` | `file`, `pattern` | Assert that a file does not contain a pattern |
| `node_status` | `node`, `status` | Assert that another node has reached a given execution status |

### 24.4 Expect values for `command` type

| Value | Meaning |
|---|---|
| `exit_code_0` | Command exits with code 0 |
| `exit_code_nonzero` | Command exits with non-zero code (useful for negative tests) |
| `output_contains: <text>` | Stdout contains the specified text |
| `output_matches: <regex>` | Stdout matches the specified regex |

### 24.5 Verification execution

A runtime MUST:
1. Execute all `verify` checks after the node's code blocks have been applied.
2. Treat the node as `completed` only if all checks pass.
3. Set the node to `failed` if any check fails, with the failing check recorded in `execution_log`.
4. NOT proceed to dependent nodes when verification fails.

### 24.6 Optional verification

Verification is not required. Nodes without `verify:` fields are considered complete when the agent reports completion. However, nodes with `verify:` fields MUST have all checks pass for automated orchestration to proceed.

---

## 25. Agent Context

*(New in v0.2)*

Agent context declarations tell the runtime what information an agent needs to execute a specific node. This enables token-efficient prompt construction by loading only relevant content.

### 25.1 Purpose

In a full AGM package with 50+ nodes, an agent executing one specific workflow node does not need the entire package in its context window. The `agent_context` field declares the minimum context required, allowing the runtime to construct a focused prompt.

### 25.2 Syntax

```agm
agent_context:
  load_nodes: [current.frontend.new-task-dialog, frontend.store.columns, arch.auto-placement.frontend]
  load_files:
    - path: src/lib/components/board/NewTaskDialog.svelte
      range: full
    - path: src/lib/types/sdd.ts
      range: full
    - path: src/lib/stores/columns.svelte.ts
      range: [1, 50]
  system_hint: "You are modifying a Svelte 5 component using runes ($state, $derived, $effect). The project uses Tailwind CSS v4."
```

### 25.3 Agent context fields

| Field | Required | Description |
|---|---|---|
| `load_nodes` | no | List of node IDs whose content should be included in the agent's context |
| `load_files` | no | List of project files to include, with optional line ranges |
| `system_hint` | no | Free-text hint for the agent about the execution environment, stack, or conventions |
| `max_tokens` | no | Approximate token budget for this node's execution context |

### 25.4 File range syntax

The `range` field in `load_files` supports:
- `full` — load the entire file
- `[start, end]` — load lines start through end (1-indexed, inclusive)
- `function: <name>` — load the function or method with the given name (requires language-aware tooling)

### 25.5 Context construction behavior

A runtime MUST:
1. Start with the target node's own content (summary, steps, code blocks).
2. Add content from `load_nodes` at operational load level.
3. Add file content from `load_files` with the specified ranges.
4. Prepend `system_hint` as a system-level instruction.
5. Respect `max_tokens` by truncating lower-priority context first.
6. Always include transitive `depends` nodes at summary level, even if not listed in `load_nodes`.

### 25.6 Default behavior without agent_context

When a node has no `agent_context` field, the runtime SHOULD fall back to loading:
- the node itself at full level
- all `depends` nodes at operational level

This preserves backward compatibility with v0.1 packages.

---

## 26. Execution State

*(New in v0.2)*

Execution state fields track the progress of node execution in agentic workflows. They transform the AGM file from a static plan into a live execution record.

### 26.1 Purpose

When a runtime executes an AGM package, it needs to track which nodes are done, which are in progress, and which have failed. Execution state fields provide this tracking directly in the AGM format, making the file itself the source of truth for execution progress.

### 26.2 Execution state fields

| Field | Type | Description |
|---|---|---|
| `execution_status` | scalar | Current execution state of the node |
| `executed_by` | scalar | Identifier of the agent or process that executed the node |
| `executed_at` | scalar | ISO 8601 timestamp of completion or failure |
| `execution_log` | scalar | Path to a log file or inline summary of execution output |
| `retry_count` | scalar | Number of execution attempts (default: 0) |

### 26.3 Execution status values

| Value | Meaning |
|---|---|
| `pending` | Not yet started (default) |
| `ready` | All dependencies satisfied, eligible for scheduling |
| `in_progress` | Currently being executed by an agent |
| `completed` | Execution and verification succeeded |
| `failed` | Execution or verification failed |
| `blocked` | One or more dependencies are in `failed` state |
| `skipped` | Intentionally not executed (e.g., user override or conditional skip) |

### 26.4 State transitions

Valid transitions:

```
pending → ready → in_progress → completed
                              → failed → ready (retry)
pending → blocked (when a dependency fails)
pending → skipped (manual override)
blocked → ready (when failed dependency is retried and succeeds)
```

A runtime MUST NOT transition a node to `in_progress` unless all nodes in its `depends` list are `completed` or `skipped`.

### 26.5 Runtime updates

A runtime MUST:
- update `execution_status` in the AGM file or in a sidecar state file
- set `executed_by` to a meaningful agent identifier (e.g., `agent-rust-01`, `claude-code-session-abc`)
- set `executed_at` upon completion or failure
- increment `retry_count` on each retry attempt
- propagate `blocked` status to all transitive dependents of a `failed` node

### 26.6 Sidecar state files

Runtimes MAY choose to store execution state in a separate sidecar file (e.g., `package.agm.state`) rather than modifying the AGM source file. This preserves the original plan as immutable while tracking progress externally. The sidecar format SHOULD use the same field names and values.

### 26.7 Example

```agm
node migration.025.schema
type: workflow
priority: critical
summary: add sdd_phase column with partial unique index and default seed updates
execution_status: completed
executed_by: agent-rust-01
executed_at: 2026-04-03T14:32:00Z
execution_log: .octopus/logs/migration.025.schema.log
retry_count: 0
```

---

## 27. Orchestration Model

*(New in v0.2)*

The orchestration model defines how a runtime coordinates multi-agent execution of an AGM package. It builds on the dependency graph, execution state, and the new `orchestration` node type.

### 27.1 Purpose

The dependency graph in AGM already implies execution order. However, explicit orchestration provides runtimes with additional guidance: which nodes can run in parallel, what concurrency limits apply, and how groups of nodes relate to each other as execution phases.

### 27.2 Orchestration flow

A runtime executing an AGM package SHOULD follow this loop:

1. **Parse** — read the AGM file and construct the node graph
2. **Validate** — check for cycles, missing references, and field compatibility
3. **Initialize** — set all nodes to `pending`, then compute `ready` for nodes with no unsatisfied dependencies
4. **Schedule** — select `ready` nodes respecting `parallel_groups` constraints and concurrency limits
5. **Build context** — for each scheduled node, construct agent prompt from `agent_context`
6. **Execute** — dispatch to agents
7. **Verify** — run `verify` checks on completed nodes
8. **Update** — set `execution_status`, propagate `blocked` to dependents if failed
9. **Loop** — return to step 4 until all nodes are `completed`, `failed`, `skipped`, or `blocked`

### 27.3 Implicit parallelism from dependencies

Even without an `orchestration` node, a runtime MAY infer parallelism from the dependency graph. Two nodes with no dependency relationship between them are candidates for parallel execution. The `orchestration` node adds explicit grouping and strategy on top of this.

### 27.4 Scheduling without orchestration nodes

When no `orchestration` node exists in a package, a runtime SHOULD:
- use topological sort on `depends` to determine execution order
- execute independent nodes in parallel up to a configurable concurrency limit
- default to sequential execution if concurrency is not configured

### 27.5 Scheduling with orchestration nodes

When an `orchestration` node exists, a runtime SHOULD:
- use `parallel_groups` to determine execution phases
- respect `requires` between groups for ordering
- use `strategy` to decide sequential vs parallel within each group
- respect `max_concurrency` limits within parallel groups

### 27.6 Multiple orchestration nodes

A package MAY contain multiple orchestration nodes for different execution scenarios (e.g., `rollout.full` vs `rollout.backend-only`). A runtime SHOULD allow the user to select which orchestration plan to follow.

### 27.7 Agent assignment

Orchestration does not prescribe which agent executes which node. Assignment is a runtime concern. However, nodes MAY include hints:

```agm
agent_context:
  system_hint: "This node requires a Rust-specialized agent with access to cargo and the project workspace."
```

A runtime MAY use `system_hint`, `lang` from code blocks, or `tags` to route nodes to appropriate agents.

### 27.8 Failure handling

When a node fails:
1. The runtime SHOULD set `execution_status: failed` on the node.
2. All transitive dependents SHOULD be set to `blocked`.
3. The runtime SHOULD surface the failure to the user with context from `execution_log`.
4. The user MAY retry the failed node (resetting to `ready`), skip it (setting to `skipped`), or abort execution.
5. On retry, `retry_count` SHOULD be incremented.

### 27.9 Progress reporting

A runtime SHOULD expose execution progress as a function of node states:

```
progress = (completed + skipped) / total_executable_nodes
```

The `orchestration` node's `parallel_groups` provide natural phase boundaries for progress reporting.

---

## 28. Memory Model

The memory model defines how agents persist and retrieve knowledge acquired during execution. Memory survives across executions and enables agents to learn from prior work.

### 28.1 Purpose

An agent executing a node may discover information not present in the AGM file: a library version, a coding pattern in the codebase, a constraint found during implementation. Without memory, this knowledge is lost between executions. The memory model provides structured persistence.

### 28.2 Memory as a node field

Nodes MAY declare memory operations using the `memory:` field:

```agm
node backend.repo.kanban-column
type: workflow
summary: update repository with sdd_phase support
memory:
  - key: repo.kanban_column.row_mapping_pattern
    topic: rust.repository
    action: upsert
    value: "row_to_column uses get() for all fields including optionals"
    scope: project
  - key: project.sqlite.partial_index_support
    topic: infrastructure
    action: upsert
    value: "SQLite 3.39+ confirmed — partial indexes supported"
    scope: project
    ttl: permanent
```

### 28.3 Memory entry fields

| Field | Required | Description |
|---|---|---|
| `key` | yes | Unique identifier for this memory entry. MUST match `[a-z][a-z0-9_.]*`. |
| `topic` | yes | Categorical tag for retrieval. Dot-delimited. Examples: `rust.models`, `frontend.svelte`, `infrastructure`. |
| `action` | yes | Operation to perform: `get`, `upsert`, `delete`, `list` |
| `value` | conditional | Required for `upsert`. The knowledge to persist. Plain text. MUST NOT exceed 32 768 bytes (32 KiB). |
| `scope` | no | Visibility scope (default: `session`). See §28.5. |
| `ttl` | no | Time-to-live (default: `session`). See §28.6. |

### 28.4 Memory actions

| Action | Behavior | Returns |
|---|---|---|
| `get` | Retrieve value by exact key. | The stored value, or null if not found. |
| `upsert` | Create or update the entry identified by key. | Confirmation with previous value if overwritten. |
| `delete` | Remove the entry identified by key. | Confirmation. No error if key does not exist. |
| `list` | List all memory entries matching the given topic. | Array of `{key, value, topic, scope, created_at, updated_at}`. |

### 28.5 Memory scopes

| Scope | Lifetime | Visibility |
|---|---|---|
| `node` | Current node execution only | Only the executing agent |
| `session` | Current package execution | All agents in the current execution run |
| `project` | Persists across executions of the same package | All executions of this package |
| `global` | Persists across all packages | All packages in the workspace |

Runtimes MUST support `session` scope. Runtimes SHOULD support `project` and `global` scopes. Runtimes MAY support `node` scope for isolation.

### 28.6 Time-to-live

| TTL | Meaning |
|---|---|
| `permanent` | Never expires. Must be explicitly deleted. |
| `session` | Expires when the current execution completes. |
| `duration:<ISO8601>` | Expires after the specified duration (e.g., `duration:P7D` for 7 days). |

Default TTL is `session`.

### 28.7 Memory storage

AGM does not prescribe a storage backend. Runtimes SHOULD persist memory in a structured store (e.g., SQLite, key-value store). The recommended schema:

```sql
CREATE TABLE agm_memory (
    key         TEXT NOT NULL,
    topic       TEXT NOT NULL,
    value       TEXT NOT NULL,
    scope       TEXT NOT NULL DEFAULT 'session',
    ttl         TEXT NOT NULL DEFAULT 'session',
    package     TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (key, package, scope)
);

CREATE INDEX idx_agm_memory_topic ON agm_memory(topic);
```

### 28.8 Semantic retrieval

Beyond exact key lookup, runtimes MAY support semantic retrieval over memory entries using embedding-based search. This enables agents to find relevant memories by topic similarity rather than exact key match.

```agm
memory:
  - action: search
    topic: rust.repository
    query: "how are optional fields handled in row mapping"
    max_results: 5
```

The `search` action is OPTIONAL. Runtimes that do not support it MUST return an empty result set, not an error.

### 28.9 Memory in agent context

The `agent_context` field MAY reference memory topics to include in the agent's prompt:

```agm
agent_context:
  load_nodes: [arch.schema]
  load_memory: [rust.repository, infrastructure]
  system_hint: "Check memory for codebase patterns before generating code."
```

When `load_memory` is specified, the runtime SHOULD retrieve all entries matching the listed topics and include them in the agent's context.

### 28.10 Memory lifecycle

Memory entries follow this lifecycle:
1. **Created** — via `upsert` during node execution
2. **Active** — available for `get`, `list`, and `search`
3. **Expired** — TTL exceeded. Runtime SHOULD remove on next access or via periodic cleanup.
4. **Deleted** — via explicit `delete` action

### 28.11 Memory and execution state interaction

When a node's `execution_status` is reset to `pending` (e.g., for retry), its `node`-scoped memory entries SHOULD be cleared. `session`, `project`, and `global` entries persist through retries.

---

---

## 29. Recommended Authoring Conventions

### 29.1 Naming conventions
Node IDs MUST:
- be lowercase
- use `.` for hierarchy
- move from broad to specific
- remain stable when semantics remain stable

Good examples:
- `auth.login`
- `auth.logout.no_upstream_endsession`
- `billing.invoice.total.compute`
- `rt.permission.edge.membership_drift`

### 29.2 Granularity guidance
A node SHOULD cover:
- one rule set
- one workflow
- one entity
- one edge case
- one decision

A node SHOULD NOT bundle:
- multiple unrelated workflows
- a large book chapter
- all examples in the same domain

### 29.3 Language guidance
Prefer:
- direct verbs
- compact nouns
- explicit constraints
- explicit outputs

Avoid:
- rhetorical filler
- conversational hedging unless uncertainty matters
- long background paragraphs in summary

### 29.4 Stable vs volatile separation
Stable architecture rules SHOULD be separated from temporary operational exceptions.

### 29.5 Examples as separate nodes
Large examples SHOULD be modeled as `example` nodes to keep normal loads lean.

---

## 30. Versioning and Lifecycle

### 30.1 Format version vs package version
- `agm` = language / syntax / spec version
- `version` = content package version

### 30.2 Node lifecycle
Node lifecycle SHOULD use:
- `status`
- `replaces`
- optional project-specific `superseded_by`

### 30.3 Backward compatibility
Small detail edits SHOULD preserve node ID.
Semantic replacement SHOULD usually create a new node and use `replaces`.

### 30.4 Suggested lifecycle examples

```agm
node auth.login.v1
type: workflow
status: superseded
summary: original login flow using browser token storage
```

```agm
node auth.login
type: workflow
status: active
replaces: [auth.login.v1]
summary: server-session login with opaque sid
```

---

## 31. Spec Versioning Policy

### 31.1 Version format

The AGM spec version uses `MAJOR.MINOR.PATCH` semver:
- `MAJOR`: breaking changes to syntax or semantics
- `MINOR`: backward-compatible additions (new fields, new node types)
- `PATCH`: clarifications, typo fixes, example updates

### 31.2 Backward compatibility

A parser implementing spec version X.Y MUST be able to parse files written for any spec version X.Z where Z ≤ Y. That is, parsers MUST be backward-compatible within the same major version.

### 31.3 Forward compatibility

A parser implementing spec version X.Y SHOULD gracefully handle files written for X.Z where Z > Y by:
- Ignoring unknown fields (preserving them if possible)
- Warning on unknown node types (not rejecting)
- Reporting the version mismatch as `AGM-P010`

### 31.4 The `agm` header field

The `agm` header field MUST contain the major version as an integer:

```agm
agm: 1
```

This tells parsers which major version of the syntax to expect. Minor and patch versions are not encoded in the file because they are backward-compatible by definition.

### 31.5 Breaking changes

The following changes constitute a major version bump:
- Removing a required field
- Changing the meaning of an existing field
- Changing syntax rules that would cause existing valid files to become invalid
- Removing a node type from the core set

The following changes are minor version bumps:
- Adding new optional fields
- Adding new node types
- Adding new validation rules (that only warn, not reject)
- Adding new memory actions or verify types

---

---

## 32. Conflict Resolution

Conflicts occur when two active nodes are semantically incompatible in the same scope.

### 32.1 Examples of conflict
- browser token model vs server-session model
- aggregate-first rounding vs line-first rounding
- SignalR as source of truth vs backend as source of truth

### 32.2 Handling conflicts
A validator or loader MUST:
1. detect co-loading of conflicting active nodes
2. inspect `scope`, `status`, and temporal applicability
3. prefer the active non-superseded node when clearly indicated
4. surface ambiguity if conflict cannot be resolved safely

### 32.3 Example

```agm
node auth.browser_token_model
type: anti_pattern
status: active
summary: browser token storage increases exposure
```

```agm
node auth.session_model_decision
type: decision
conflicts: [auth.browser_token_model]
summary: use opaque sid with server-side token storage
```

---

## 33. Temporal and Contextual Applicability

Not all knowledge applies everywhere or forever.

### 33.1 Scope
Use `scope` for simple domains such as:
- environment: dev, qa, prod
- tenant type: cloud, onprem
- plan tier: free, pro, enterprise

Example:

```agm
scope: [prod]
```

### 33.2 Applies_when
Use `applies_when` for slightly more expressive applicability.

Example:

```agm
applies_when: environment=production
```

### 33.3 Temporal bounds

```agm
valid_from: 2026-04-01
valid_until: 2026-07-01
```

Use cases:
- migration windows
- temporary exceptions
- phased rollouts

### 33.4 Resolution order
If nodes conflict, tools SHOULD evaluate:
1. status
2. temporal validity
3. scope match
4. explicit replacement
5. explicit conflict
6. confidence

---

## 34. Retrieval and Indexing Guidance

AGM retrieval should be node-oriented rather than arbitrary chunk-oriented.

### 34.1 Index the summary
At minimum, index:
- node ID
- type
- summary
- tags
- aliases
- keywords

### 34.2 Index operational fields separately
Fields like `items`, `steps`, and `fields` SHOULD be optionally indexed as structured content.

### 34.3 Hybrid retrieval
Best results often come from:
- symbolic lookup by node ID
- lexical search over summaries
- semantic retrieval over summary + operational fields

### 34.4 Ranking recommendations
Rank candidates using:
- node type relevance
- summary similarity
- scope match
- status
- stability
- package relevance

### 34.5 Retrieval units
Default retrieval unit SHOULD be one node.
Large nodes MAY be split into sub-units for embedding, but expansion should still map back to the node.

---

## 35. Compilation from Markdown and Other Sources

A major AGM use case is compilation from human-facing docs.

### 35.1 Why compile?
Manual duplication between `.md` and `.agm` creates drift. Compilation or assisted extraction reduces divergence.

### 35.2 Source patterns to detect

| Source phrase pattern | Likely AGM type |
|---|---|
| "must", "must not", "do not" | `rules` |
| "the flow is", ordered steps | `workflow` |
| "entity", "contains", "fields" | `entity` |
| "we decided", "decision", "chosen because" | `decision` |
| "in case of", "if X fails" | `exception` |
| "example" | `example` |
| "term means" | `glossary` |
| "avoid", "bad practice" | `anti_pattern` |

### 35.3 Suggested compilation pipeline
1. parse sections
2. detect candidate semantic units
3. infer node type
4. generate candidate summary
5. extract operational structure
6. infer relations
7. validate
8. optionally ask for human approval

### 35.4 Example transformation

#### Source Markdown

```md
## Login Constraints

- Access tokens must never be exposed to the browser.
- Sensitive calls must originate from the server.

## Login Flow

1. Resolve tenant by host.
2. Redirect to the provider.
3. Validate callback.
4. Create a server-side session.
```

#### Compiled AGM

```agm
node auth.constraints
type: rules
summary: no browser tokens; sensitive calls from server only
items:
  - no_client_tokens
  - server_side_sensitive_calls
```

```agm
node auth.login
type: workflow
depends: [auth.constraints]
summary: resolve tenant -> redirect -> callback -> create server session
steps:
  - resolve tenant by host
  - redirect to provider
  - validate callback
  - create server-side session
```

### 35.5 Assisted compilation guidance
Human-in-the-loop review SHOULD check:
- node granularity
- summary quality
- missing dependencies
- over-aggressive compression
- lost caveats

---

## 36. Renderer Guidance

AGM can be rendered into multiple views.

### 36.1 Human-readable Markdown
A renderer MAY generate a Markdown handbook grouped by type or package.

### 36.2 Agent context bundle
A renderer MAY output:
- only summaries
- summary + operational fields
- only selected nodes

### 36.3 JSON AST
A renderer MAY emit a normalized machine representation.

### 36.4 Graph view
A renderer MAY emit edges for graph visualization:
- `depends`
- `related_to`
- `conflicts`
- `replaces`

---

## 37. JSON Canonical Form

AGM defines a canonical JSON representation for interoperability with APIs, databases, and tooling that consumes structured data.

### 37.1 Package JSON

```json
{
  "agm": 1,
  "package": "auth.platform",
  "version": "0.2.0",
  "title": "Authentication Platform Core",
  "owner": "security-platform",
  "imports": ["shared.security@^1.0.0"],
  "default_load": "summary",
  "status": "active",
  "tags": ["auth", "security"],
  "nodes": [ ... ]
}
```

### 37.2 Node JSON

All fields map directly. Lists remain arrays. Block fields become strings. Structured fields (`code`, `verify`, `agent_context`, `memory`, `parallel_groups`) become objects or arrays of objects.

```json
{
  "node": "backend.repo",
  "type": "workflow",
  "stability": "medium",
  "priority": "critical",
  "depends": ["arch.schema", "backend.model"],
  "summary": "update repository with sdd_phase support",
  "steps": ["read sdd_phase in row_to_column", "include in create/update"],
  "code": {
    "lang": "rust",
    "target": "src/repo.rs",
    "action": "append",
    "body": "fn get_by_sdd_phase(...) { ... }"
  },
  "verify": [
    {"type": "command", "run": "cargo check", "expect": "exit_code_0"}
  ],
  "memory": [
    {"key": "repo.pattern", "topic": "rust", "action": "upsert", "value": "..."}
  ],
  "execution_status": "pending"
}
```

### 37.3 Conversion rules

| AGM text | JSON type |
|---|---|
| Scalar field | String |
| Inline list `[a, b, c]` | Array of strings |
| Indented list (`- item`) | Array of strings |
| Block field (multiline text) | String (lines joined with `\n`) |
| `code:` / structured fields | Object |
| `code_blocks:` / `verify:` / `memory:` / `parallel_groups:` | Array of objects |
| Boolean-like values (`true`, `false`) | JSON boolean |
| Integer-like values | JSON number |

### 37.4 Round-trip fidelity

A conformant converter MUST preserve round-trip fidelity: `AGM text → JSON → AGM text` MUST produce semantically identical output. Field ordering MAY differ. Whitespace MAY differ. Comments are NOT preserved in JSON.

---

---

## 38. Complete Examples

This section provides complex, realistic examples.

---

### 38.1 Example Set A: Authentication Platform

```agm
agm: 1
package: auth.platform
version: 0.2.0
title: Authentication Platform Core
owner: security-platform
default_load: summary
tags: [auth, security, oidc]

node auth.stack
type: facts
stability: medium
priority: high
tags: [stack, infrastructure]
summary: next15 frontend; net8 backend; identityserver4 local idp; azure external id as external provider
items:
  - frontend=next15
  - backend=net8
  - local_idp=identityserver4
  - external_idp=azure_external_id
detail:
  The frontend uses Next.js App Router.
  The backend uses .NET 8 services.
  IdentityServer4 acts as the local identity provider.
  Azure External ID is used for external federation scenarios.

node auth.constraints
type: rules
stability: high
priority: critical
tags: [security]
summary: no browser tokens; sensitive calls from server only; tenant selected by host
items:
  - no_client_tokens
  - server_side_sensitive_calls
  - host_based_tenant_selection
detail:
  Access and refresh tokens MUST remain server-side.
  The browser stores only an opaque httpOnly sid cookie.
  Sensitive API calls SHOULD originate from trusted server-side paths.
  Tenant resolution MUST fail closed for unknown hosts.

node auth.session
type: entity
stability: medium
priority: high
tags: [domain, session]
summary: opaque sid maps to server-side token pair and session metadata
fields:
  - sid: opaque string
  - user_id: string
  - tenant_id: string
  - access_token: secure server-side secret
  - refresh_token: secure server-side secret
  - expires_at: datetime
detail:
  sid is the only browser-visible session identifier.
  Token material MUST NOT be exposed to client runtime code.
  Session revocation invalidates the sid-to-token mapping.

node auth.tenant.resolve
type: workflow
stability: medium
priority: high
tags: [tenant]
input: [host]
output: [tenant_id, oidc_config]
summary: map request host to tenant and oidc configuration
steps:
  - inspect request host
  - resolve host-to-tenant mapping
  - load oidc configuration for that tenant
  - reject unknown or disabled hosts
detail:
  Resolution should be deterministic for a given host.
  Configuration lookup may be cached but MUST reflect authoritative tenant state.

node auth.login
type: workflow
stability: medium
priority: critical
tags: [login, oidc]
depends: [auth.constraints, auth.session, auth.tenant.resolve]
input: [host, return_url]
output: [redirect_url, sid_cookie]
summary: resolve tenant -> select oidc config -> redirect -> callback -> create sid session
steps:
  - resolve tenant from request host
  - select oidc config for that tenant
  - redirect to identity provider
  - validate callback, correlation, and state
  - persist access and refresh token server-side
  - create sid session
  - emit httpOnly sid cookie
detail:
  Login MUST NOT leak access tokens to the browser.
  Correlation and state validation are mandatory.
  Return URL validation SHOULD prevent open redirect issues.

node auth.refresh
type: workflow
stability: medium
priority: high
depends: [auth.session, auth.constraints]
input: [sid]
output: [refreshed_session_state]
summary: look up session by sid -> refresh upstream token if needed -> update server-side session
steps:
  - resolve sid to server-side session
  - inspect token expiry
  - if needed call refresh flow with upstream provider
  - persist new token material server-side
  - keep browser-visible sid stable unless rotation policy requires otherwise
detail:
  Refresh should remain invisible to the browser.
  Failures SHOULD preserve security over convenience.

node auth.logout
type: workflow
stability: medium
priority: critical
tags: [logout]
depends: [auth.session]
input: [sid]
output: [session_revoked, cookie_cleared]
summary: revoke server session, clear sid cookie, attempt upstream logout when supported
steps:
  - locate session by sid
  - revoke local session
  - clear sid cookie
  - if provider supports end-session then trigger provider logout
  - notify active realtime connections if needed
detail:
  Local revocation MUST complete even if upstream logout is unavailable.
  Client-visible messaging SHOULD distinguish local sign-out from federated sign-out when needed.

node auth.logout.no_upstream_endsession
type: exception
stability: medium
priority: high
depends: [auth.logout]
summary: some providers allow local logout but do not guarantee upstream sign-out
detail:
  The local session can be terminated successfully while the upstream provider session remains alive.
  Reauthentication may silently reuse that provider session.
resolution:
  - show clear logout scope to the user
  - optionally force account chooser
  - document provider limitation
  - do not treat this as a local session revocation failure

node auth.browser_token_model
type: anti_pattern
stability: high
priority: high
tags: [security, anti-pattern]
summary: browser-stored access tokens increase exposure and weaken revocation control
detail:
  localStorage or sessionStorage token persistence increases the exposure surface.
  It also weakens server-side control over revocation and rotation.

node auth.session_model_decision
type: decision
stability: high
priority: critical
tags: [architecture]
depends: [auth.constraints, auth.session]
conflicts: [auth.browser_token_model]
summary: choose opaque sid cookie with server-side token storage instead of browser token model
rationale:
  - lowers token exposure surface
  - centralizes revocation
  - aligns with bff architecture
  - simplifies browser-side trust assumptions
tradeoffs:
  - session lookup becomes mandatory
  - backend complexity increases
  - horizontal scale requires robust session persistence
```

---

### 38.2 Example Set B: Billing / Invoice Domain

```agm
agm: 1
package: billing.invoice
version: 1.0.0
title: Invoice Knowledge Pack
default_load: operational

node invoice.definition
type: entity
stability: high
priority: critical
tags: [invoice, fiscal, domain]
summary: invoice contains issuer, receiver, currency, lines, totals and tax breakdown
fields:
  - issuer: party
  - receiver: party
  - currency: code
  - lines: collection
  - subtotal: money
  - tax_total: money
  - grand_total: money
detail:
  The invoice aggregates line-level commercial information and derives final totals using tax rules.
  Depending on jurisdiction, additional metadata may be required, but the core structure remains stable.

node invoice.validation.rules
type: rules
stability: high
priority: critical
tags: [validation]
summary: totals must reconcile; tax aggregation must match lines; currency is mandatory
items:
  - subtotal_equals_sum_of_lines
  - tax_total_equals_aggregated_tax
  - grand_total_equals_subtotal_plus_tax
  - currency_required
detail:
  Reconciliation rules should be deterministic.
  Validation should surface exact mismatch reasons instead of generic total errors.

node invoice.total.compute
type: workflow
stability: medium
priority: critical
depends: [invoice.definition, invoice.validation.rules]
input: [lines, tax_policy]
output: [subtotal, tax_total, grand_total]
summary: aggregate lines -> compute tax -> reconcile totals
steps:
  - sum line net amounts
  - compute tax per applicable line rule
  - aggregate computed tax
  - compute grand total
  - validate reconciliation
detail:
  The computation pipeline should produce deterministic results given the same lines and tax policy.
  Any rounding strategy must be explicit.

node invoice.exempt_operation
type: glossary
stability: high
priority: high
summary: exempt operation is included in the economic amount but not taxed under the applicable exemption rule
detail:
  Exempt operations contribute to the invoice amount structure but follow different tax treatment than taxable operations.

node invoice.edge.rounding_mismatch
type: exception
stability: medium
priority: high
depends: [invoice.total.compute]
summary: per-line rounding and aggregate rounding can differ by small amounts
detail:
  Rounding at line level may produce a tax total slightly different from rounding after aggregation.
  This is common when line items carry fractional tax values.
resolution:
  - define one canonical rounding strategy
  - keep tolerance explicit
  - log adjustments if adjustments are permitted
  - document jurisdiction-specific rounding expectations

node invoice.total.strategy.decision
type: decision
stability: high
priority: high
depends: [invoice.edge.rounding_mismatch]
summary: use canonical line-first rounding to keep generated totals deterministic
rationale:
  - improves repeatability
  - eases auditability
  - simplifies reconciliation debugging
tradeoffs:
  - may differ from aggregate-first calculations
  - may require explicit disclosure in audit documentation

node invoice.example.exempt_with_taxable_lines
type: example
stability: medium
priority: low
depends: [invoice.definition, invoice.exempt_operation, invoice.total.compute]
summary: sample invoice containing taxable and exempt lines with reconciled totals
detail:
  line 1: taxable service = 100
  line 2: exempt operation = 50
  tax on line 1 = 18
  subtotal = 150
  tax_total = 18
  grand_total = 168
```

---

### 38.3 Example Set C: Realtime Permissions and SignalR-Style Delivery

```agm
agm: 1
package: realtime.permissions
version: 0.4.0
title: Realtime Permission Delivery Model
default_load: summary

node rt.permission.model
type: facts
stability: medium
priority: critical
summary: realtime events target users, meetings, roles and optional permission groups
items:
  - target_user_group
  - target_meeting_group
  - target_role_group
  - optional_permission_group

node rt.permission.groups
type: entity
stability: medium
priority: high
summary: group identities follow stable naming patterns for user, role, meeting and permission scopes
fields:
  - user_group: user:{userId}
  - meeting_group: meeting:{clientId}:{meetId}
  - role_group: role:{scope}:{roleName}
  - permission_group: permission:{clientId}:{permissionKey}

node rt.permission.broadcast
type: workflow
stability: medium
priority: critical
depends: [rt.permission.model, rt.permission.groups]
input: [event_name, payload, target_selector]
output: [event_dispatched]
summary: resolve target group -> broadcast event -> rely on authoritative backend auth for membership correctness
steps:
  - identify event target scope
  - resolve matching group name
  - publish event to group
  - do not infer authorization truth from group membership alone
detail:
  Group-based fan-out is an optimization for event delivery.
  It MUST NOT replace authoritative backend permission checks for critical operations.

node rt.permission.group_usage_rule
type: rules
stability: high
priority: critical
summary: permission groups are valid for event fan-out but not as the sole source of authorization truth
items:
  - group_membership_can_accelerate_delivery
  - backend_remains_authoritative
  - membership_must_be_reconciled_on_permission_change
detail:
  Permission groups are useful for routing and fan-out efficiency.
  They become dangerous when treated as durable authorization truth without reconciliation.

node rt.permission.edge.membership_drift
type: exception
stability: medium
priority: high
depends: [rt.permission.group_usage_rule]
summary: signalr, redis and database can temporarily diverge in effective membership
detail:
  Permission changes may be persisted before all active realtime connections are reconciled.
  Temporary drift can appear between the authoritative database and active group membership.
resolution:
  - publish permission-changed events
  - re-evaluate memberships on reconnect
  - run reconciliation jobs for stale connections
  - keep critical authorization checks in backend

node rt.permission.antipattern.signalr_as_auth_source
type: anti_pattern
stability: high
priority: critical
summary: using realtime group membership as the only authorization source risks stale or incorrect access decisions
detail:
  Group membership is a delivery optimization, not a durable source of truth for permissions.

node rt.permission.decision.hybrid_delivery
type: decision
stability: high
priority: critical
depends: [rt.permission.group_usage_rule, rt.permission.edge.membership_drift]
conflicts: [rt.permission.antipattern.signalr_as_auth_source]
summary: use permission groups for efficient fan-out while keeping backend authorization authoritative
rationale:
  - reduces per-event filtering cost
  - keeps delivery efficient
  - avoids turning realtime state into primary authorization truth
tradeoffs:
  - requires reconciliation logic
  - increases operational complexity
  - requires observability to detect drift
```

---

### 38.4 Example Set D: Product Knowledge with Temporal Scope

```agm
agm: 1
package: product.pricing
version: 0.3.0

node pricing.plan.matrix
type: facts
stability: medium
priority: critical
summary: plans are free, pro and enterprise with increasing limits and capabilities
items:
  - free
  - pro
  - enterprise

node pricing.rule.enterprise_only_feature_x
type: rules
stability: medium
priority: high
scope: [enterprise]
summary: feature_x is only available for enterprise customers
items:
  - feature_x_requires_enterprise_tier

node pricing.rule.promo_q2_upgrade_discount
type: rules
stability: low
priority: normal
valid_from: 2026-04-01
valid_until: 2026-06-30
summary: q2 promotional upgrade discount applies to pro annual upgrades
items:
  - promo_window_q2_2026
  - annual_upgrade_only
  - pro_plan_target
```

---

### 38.5 Example Set E: Migration Example with Replacements

```agm
agm: 1
package: infra.caching
version: 2.0.0

node cache.strategy.v1
type: decision
status: superseded
summary: use local in-memory cache only for session resolution
rationale:
  - simplest initial deployment
tradeoffs:
  - weak multi-instance coherence

node cache.strategy.v2
type: decision
status: active
replaces: [cache.strategy.v1]
summary: use local cache with distributed backing store for session resolution
rationale:
  - preserves fast path
  - enables multi-instance coherence
tradeoffs:
  - higher operational complexity
  - more moving parts
```

---

### 38.6 Example Set F: Implementation Plan with Code, Verification, and Orchestration

```agm
agm: 1
package: kanban.sdd-phase-columns
version: 0.1.0
title: Kanban Columns Associated with SDD Phases
owner: octopus-board-platform
default_load: operational
status: draft
tags: [kanban, sdd, sqlite, tauri, svelte, implementation-plan]
target_runtime: octopus
load_profiles:
  minimal:
    filter: "priority in [critical] AND type in [decision]"
    estimated_tokens: 800
  operational:
    filter: "priority in [critical, high] AND type in [workflow, decision, facts]"
    estimated_tokens: 3200
  executable:
    filter: "type in [workflow, orchestration] AND code is present"
    estimated_tokens: 4500
  full:
    filter: "*"
    estimated_tokens: 6800

node arch.schema.sdd-phase-field
type: decision
stability: high
priority: critical
summary: add nullable sdd_phase TEXT field directly to kanban_columns with CHECK constraint and partial unique index per project
rationale:
  - simplest_representation_for_optional_one_to_one_association
  - avoids_extra_join_table_and_extra_repository_complexity
tradeoffs:
  - adds_domain_specific_field_to_general_column_table
  - uniqueness_requires_partial_index_support

node migration.025.schema
type: workflow
stability: high
priority: critical
depends: [arch.schema.sdd-phase-field]
input: [src-tauri/src/storage/sqlite/migrations/025_column_sdd_phase.sql]
output: [kanban_columns_has_sdd_phase, unique_partial_index_exists]
summary: add sdd_phase column -> add partial unique index -> seed default mappings
steps:
  - alter kanban_columns add nullable sdd_phase with CHECK constraint
  - create unique index on project_id and sdd_phase where sdd_phase is not null
  - update ready rows to analyze
  - update in_progress rows to implement
  - update review rows to verify
code:
  lang: sql
  target: src-tauri/src/storage/sqlite/migrations/025_column_sdd_phase.sql
  action: create
  body: |
    ALTER TABLE kanban_columns
        ADD COLUMN sdd_phase TEXT
        CHECK (sdd_phase IS NULL OR sdd_phase IN ('analyze', 'plan', 'implement', 'verify'));

    CREATE UNIQUE INDEX idx_kanban_columns_sdd_phase
        ON kanban_columns(project_id, sdd_phase)
        WHERE sdd_phase IS NOT NULL;

    UPDATE kanban_columns SET sdd_phase = 'analyze'
    WHERE slug = 'ready' AND sdd_phase IS NULL;

    UPDATE kanban_columns SET sdd_phase = 'implement'
    WHERE slug = 'in_progress' AND sdd_phase IS NULL;

    UPDATE kanban_columns SET sdd_phase = 'verify'
    WHERE slug = 'review' AND sdd_phase IS NULL;
verify:
  - type: file_exists
    file: src-tauri/src/storage/sqlite/migrations/025_column_sdd_phase.sql
  - type: file_contains
    file: src-tauri/src/storage/sqlite/migrations/025_column_sdd_phase.sql
    pattern: "ADD COLUMN sdd_phase"
agent_context:
  load_nodes: [arch.schema.sdd-phase-field]
  load_files:
    - path: src-tauri/src/storage/sqlite/migrations.rs
      range: full
  system_hint: "SQLite migration file. Follow existing naming convention 0XX_description.sql."
execution_status: pending

node backend.model.kanban-column
type: workflow
stability: medium
priority: critical
depends: [arch.schema.sdd-phase-field]
input: [src-tauri/src/models/kanban_column.rs]
output: [kanban_column_structs_support_sdd_phase]
summary: add sdd_phase to KanbanColumn, CreateKanbanColumn, and UpdateKanbanColumn structs
steps:
  - add Option<String> sdd_phase to KanbanColumn
  - add Option<String> sdd_phase to CreateKanbanColumn
  - add Option<Option<String>> sdd_phase to UpdateKanbanColumn
  - extend default_column_seeds with sdd_phase mapping
code_blocks:
  - lang: rust
    target: src-tauri/src/models/kanban_column.rs
    action: insert_after
    anchor: "pub requires_approval: bool,"
    body: |
      /// Optional SDD phase associated with this column.
      pub sdd_phase: Option<String>,
verify:
  - type: command
    run: cargo check --manifest-path src-tauri/Cargo.toml
    expect: exit_code_0
  - type: file_contains
    file: src-tauri/src/models/kanban_column.rs
    pattern: "pub sdd_phase: Option<String>"
agent_context:
  load_nodes: [arch.schema.sdd-phase-field]
  load_files:
    - path: src-tauri/src/models/kanban_column.rs
      range: full
  system_hint: "Rust project using rusqlite. The UpdateKanbanColumn uses Option<Option<T>> for partial updates."
execution_status: pending

node frontend.new-task.phase-selector
type: workflow
stability: medium
priority: critical
depends: [backend.model.kanban-column]
input: [src/lib/components/board/NewTaskDialog.svelte]
output: [new_task_dialog_can_choose_initial_sdd_phase_and_resolve_column]
summary: extend NewTaskDialog so SDD-enabled tasks can choose an initial phase and auto-resolve target column
steps:
  - import SDD_PHASES and SDD_PHASE_LABELS
  - add selectedSddPhase state
  - show selector only when sddEnabled is true
  - on submit resolve column via getColumnBySddPhase
  - pass resolved column_id or null to createTask
  - reset selectedSddPhase when sddEnabled becomes false
code:
  lang: svelte
  target: src/lib/components/board/NewTaskDialog.svelte
  action: insert_after
  anchor: "let sddEnabled = $state(false);"
  body: |
    let selectedSddPhase = $state<string>('');

    $effect(() => {
        if (!sddEnabled) selectedSddPhase = '';
    });
verify:
  - type: file_contains
    file: src/lib/components/board/NewTaskDialog.svelte
    pattern: "selectedSddPhase"
agent_context:
  load_nodes: [arch.schema.sdd-phase-field]
  load_files:
    - path: src/lib/components/board/NewTaskDialog.svelte
      range: full
    - path: src/lib/types/sdd.ts
      range: full
    - path: src/lib/stores/columns.svelte.ts
      range: [1, 50]
  system_hint: "Svelte 5 using runes ($state, $derived, $effect). Tailwind CSS v4. Import from $lib/."
execution_status: pending

node rollout.orchestration
type: orchestration
stability: medium
priority: critical
summary: coordinate backend-first rollout with parallel frontend after backend stabilizes
parallel_groups:
  - group: 1-schema
    nodes: [migration.025.schema]
    strategy: sequential
  - group: 2-backend-models
    nodes: [backend.model.kanban-column]
    strategy: sequential
    requires: [1-schema]
  - group: 3-frontend-ui
    nodes: [frontend.new-task.phase-selector]
    strategy: sequential
    requires: [2-backend-models]
detail:
  Simplified orchestration example. A full plan would include repository, commands,
  types, store, column editor, badge, and test nodes across additional parallel groups.
```

---

## 39. Edge Cases and Failure Modes

### 39.1 Contradictory active nodes
Two active nodes may assert conflicting policies.

Mitigation:
- use `conflicts`
- use `status`
- use `scope`
- use `valid_from` / `valid_until`

### 39.2 Node too large
Symptoms:
- summary cannot discriminate clearly
- operational load is huge
- retrieval frequently pulls irrelevant detail

Mitigation:
- split by workflow phase
- split examples out
- split stable rule set from volatile exceptions

### 39.3 Node too small
Symptoms:
- excessive graph fragmentation
- too many dependencies for simple questions
- cognitive overhead for maintainers

Mitigation:
- merge near-duplicate nodes
- keep one node per primary idea, not per sentence

### 39.4 Cycles in `depends`
Hard cycles make expansion ambiguous and can cause loaders to recurse indefinitely.

Mitigation:
- reject cycles
- convert some links to `related_to`
- redesign node boundaries

### 39.5 Stale replacements
A node may say it replaces another, while the old node remains active and highly ranked.

Mitigation:
- validator should warn
- loader should down-rank deprecated and superseded nodes

### 39.6 Excessive examples
Examples often dominate token cost.

Mitigation:
- separate as `example` nodes
- load only on demand

### 39.7 Ambiguous applicability
A rule may be valid only in prod or only during migration.

Mitigation:
- use `scope`
- use `applies_when`
- use temporal bounds

### 39.8 Uncertain information
A team may want to store hypotheses or reverse-engineered behavior.

Mitigation:
- use `confidence`
- avoid encoding uncertain findings as hard facts

### 39.9 Code block target drift
The `target` file in a code block may not exist, may have changed structure, or the `anchor`/`old` pattern may no longer match.

Mitigation:
- verify checks should detect missing targets before execution
- agents should be instructed to read the current file state before applying code
- `agent_context.load_files` should include the target file to give the agent current state

### 39.10 Verification false positives
A verify check may pass even though the implementation is incorrect (e.g., `file_contains` matches a comment rather than actual code).

Mitigation:
- use specific patterns that match implementation, not comments
- combine multiple verify types (e.g., `file_contains` + `command` with tests)
- treat verification as a confidence signal, not absolute proof

### 39.11 Orchestration deadlock
A circular `requires` between parallel groups would create a deadlock where no group can start.

Mitigation:
- validators MUST reject cycles in `requires` relationships between groups
- this is analogous to cycle detection in `depends` between nodes

### 39.12 Execution state corruption
If a runtime crashes mid-execution, node states may be inconsistent (e.g., `in_progress` with no agent running).

Mitigation:
- use sidecar state files that can be reset independently
- provide an `agm reset` command to clear execution state
- implement heartbeat or timeout mechanisms for `in_progress` nodes

### 39.13 Agent context overload
An `agent_context` that loads too many nodes or files may exceed the agent's context window.

Mitigation:
- use `max_tokens` to set a budget
- prefer `range` over `full` for large files
- load `depends` at summary level by default, operational only when listed in `load_nodes`

---

## 40. Security Considerations

AGM itself is a content format, but its usage can create security issues.

### 40.1 Secret leakage
AGM files MUST NOT be treated as a safe place to store secrets. Compilers and validators SHOULD detect likely secret patterns.

### 40.2 Over-trusting examples
Examples MAY contain sensitive sample payloads. Pipelines SHOULD scrub secrets, internal URLs, and personal data when appropriate.

### 40.3 Runtime authority confusion
A decision stored in AGM is not a security boundary by itself. AGM informs agents and tools; it does not enforce runtime policy unless integrated with enforcement mechanisms.

### 40.4 Stale security knowledge
Security-critical nodes SHOULD have clear ownership, validation, and freshness management.

### 40.5 Code block execution safety
Code blocks in AGM are intended for execution by agents. Runtimes MUST:
- sandbox code execution to prevent unintended side effects
- validate `target` paths to prevent directory traversal attacks
- reject code blocks that reference paths outside the project root
- log all file modifications for auditability
- never execute code blocks in production environments without explicit human approval

### 40.6 Agent context data exposure
`agent_context.load_files` may reference files containing sensitive data. Runtimes MUST:
- apply the same access controls to context loading as to direct file access
- not send file contents to external agents without user consent
- respect `.gitignore` and `.agmignore` patterns when loading files

---

## 41. Performance Considerations

### 41.1 Loader performance
Summary-only loading SHOULD be extremely cheap.

### 41.2 Index design
A practical index often includes:
- exact lookup by node ID
- lexical index over summaries and tags
- embedding index over summary + operational fields

### 41.3 Caching
Stable nodes with `stability: high` are good candidates for aggressive caching.

### 41.4 Chunk generation
If embeddings are used, chunking SHOULD align with node boundaries as much as possible.

---

## 42. Implementation Roadmap

### 42.1 Minimum viable implementation
- parser (including code blocks and verify fields)
- validator (including cycle detection, reference resolution, code block field validation)
- summary/operational/executable/full loader
- basic renderer
- basic indexer

### 42.2 Recommended next layer
- Markdown-to-AGM assisted compiler (with code block extraction)
- graph visualization
- semantic diffing
- linter for summary quality and node granularity
- execution state tracker (in-file or sidecar)
- basic orchestration scheduler (sequential, dependency-order)

### 42.3 Orchestration layer
- parallel group scheduler with concurrency control
- agent context builder (prompt construction from `agent_context`)
- verification runner (execute `verify` checks, update `execution_status`)
- progress reporting and failure handling
- retry logic with `retry_count` tracking
- integration with agent runtimes (e.g., Claude Code, custom agents)

### 42.4 Mature ecosystem
- IDE support (syntax highlighting, node navigation, graph preview)
- package imports and cross-package references
- schema profiles per node type
- query helpers for graph traversal
- policy-based validation rules
- visual orchestration dashboard (e.g., Kanban view of execution state)
- AGM-native diffing for plan evolution tracking

---

## 43. Reference Templates

### 43.1 `rules` template

```agm
node domain.rule_set
type: rules
stability: high
priority: critical
summary: concise rule summary
items:
  - rule_a
  - rule_b
detail:
  explanatory details
```

### 43.2 `workflow` template

```agm
node domain.flow
type: workflow
stability: medium
priority: high
depends: [domain.rule_set]
input: [arg1, arg2]
output: [result1]
summary: step1 -> step2 -> step3
steps:
  - step1
  - step2
  - step3
detail:
  explanation and caveats
```

### 43.3 `entity` template

```agm
node domain.entity
type: entity
stability: medium
priority: high
summary: concise entity meaning
fields:
  - field_a: type
  - field_b: type
detail:
  additional field semantics
```

### 43.4 `decision` template

```agm
node domain.decision
type: decision
stability: high
priority: critical
depends: [domain.rule_set]
conflicts: [domain.bad_approach]
summary: chosen architectural decision
rationale:
  - reason one
  - reason two
tradeoffs:
  - downside one
  - downside two
```

### 43.5 `exception` template

```agm
node domain.edge.case
type: exception
stability: medium
priority: high
depends: [domain.flow]
summary: concise statement of the edge case
detail:
  description of the failure or special condition
resolution:
  - mitigation one
  - mitigation two
```

### 43.6 `workflow` with code blocks template

```agm
node domain.impl.feature
type: workflow
stability: medium
priority: critical
depends: [domain.schema, domain.model]
input: [src/models/entity.rs]
output: [entity_model_supports_new_field]
summary: add new_field to Entity struct and update repository queries
steps:
  - add field to struct
  - update row_to_entity mapping
  - update create and update queries
code:
  lang: rust
  target: src/models/entity.rs
  action: insert_after
  anchor: "pub struct Entity {"
  body: |
    pub new_field: Option<String>,
verify:
  - type: command
    run: cargo check
    expect: exit_code_0
  - type: file_contains
    file: src/models/entity.rs
    pattern: "new_field: Option<String>"
agent_context:
  load_nodes: [domain.schema, domain.model]
  load_files:
    - path: src/models/entity.rs
      range: full
  system_hint: "Rust project using rusqlite. Follow existing patterns in the file."
```

### 43.7 `orchestration` template

```agm
node domain.rollout
type: orchestration
stability: medium
priority: critical
summary: coordinate implementation in backend-first phases with parallel frontend work
parallel_groups:
  - group: 1-foundation
    nodes: [domain.migration, domain.registration]
    strategy: sequential
  - group: 2-models
    nodes: [domain.model.a, domain.model.b]
    strategy: parallel
  - group: 3-implementation
    nodes: [domain.repo, domain.commands]
    strategy: parallel
    requires: [2-models]
  - group: 4-frontend
    nodes: [domain.types, domain.store, domain.ui]
    strategy: sequential
    requires: [3-implementation]
  - group: 5-testing
    nodes: [domain.test.unit, domain.test.integration]
    strategy: parallel
    requires: [4-frontend]
detail:
  Backend-first rollout preferred because UI depends on stable schema and command contracts.
```

---

## 44. Appendix A: Informal Grammar

This grammar is informal and intentionally simplified.

```text
file            := header node+
header          := header_field+
header_field    := scalar_field | list_field | block_field | structured_field

node            := node_decl node_field+
node_decl       := "node" SP IDENT NL

node_field      := scalar_field
                 | inline_list_field
                 | indented_list_field
                 | block_field
                 | code_field
                 | verify_field
                 | agent_context_field
                 | parallel_groups_field

scalar_field    := KEY ":" SP? VALUE NL

inline_list_field := KEY ":" SP? "[" list_items "]" NL
list_items      := VALUE ("," SP? VALUE)*

indented_list_field := KEY ":" NL INDENT list_item+
list_item       := "-" SP VALUE NL

block_field     := KEY ":" NL INDENT block_line+
block_line      := TEXT NL

code_field      := "code:" NL INDENT code_subfield+
code_subfield   := scalar_field | body_field
body_field      := "body:" SP "|" NL INDENT code_line+
code_line       := TEXT NL

code_blocks_field := "code_blocks:" NL (INDENT "-" SP code_subfield+)+

verify_field    := "verify:" NL (INDENT "-" SP verify_item)+
verify_item     := verify_subfield+
verify_subfield := scalar_field

agent_context_field := "agent_context:" NL INDENT context_subfield+
context_subfield := scalar_field | inline_list_field | load_files_field
load_files_field := "load_files:" NL (INDENT "-" SP file_ref)+
file_ref        := file_ref_subfield+
file_ref_subfield := scalar_field

parallel_groups_field := "parallel_groups:" NL (INDENT "-" SP group_subfield+)+
group_subfield  := scalar_field | inline_list_field

load_profiles_field := "load_profiles:" NL (INDENT profile_entry)+
profile_entry   := IDENT ":" NL INDENT profile_subfield+
profile_subfield := scalar_field
```

This is sufficient for a parser implementation. More formal grammar can be added in later versions.

---

## 45. Appendix B: Example AST Shape

A normalized AST for a node might look like this:

```json
{
  "node": "auth.login",
  "type": "workflow",
  "stability": "medium",
  "priority": "critical",
  "depends": ["auth.constraints", "auth.session", "auth.tenant.resolve"],
  "input": ["host", "return_url"],
  "output": ["redirect_url", "sid_cookie"],
  "summary": "resolve tenant -> select oidc config -> redirect -> callback -> create sid session",
  "steps": [
    "resolve tenant from request host",
    "select oidc config for that tenant",
    "redirect to identity provider",
    "validate callback, correlation, and state",
    "persist access and refresh token server-side",
    "create sid session",
    "emit httpOnly sid cookie"
  ],
  "detail": [
    "Login MUST NOT leak access tokens to the browser.",
    "Correlation and state validation are mandatory.",
    "Return URL validation SHOULD prevent open redirect issues."
  ],
  "code": {
    "lang": "rust",
    "target": "src/handlers/auth.rs",
    "action": "replace",
    "old": "fn login_handler() { todo!() }",
    "body": "fn login_handler(req: HttpRequest) -> Result<Redirect> { ... }"
  },
  "verify": [
    { "type": "command", "run": "cargo check", "expect": "exit_code_0" },
    { "type": "file_contains", "file": "src/handlers/auth.rs", "pattern": "fn login_handler" }
  ],
  "agent_context": {
    "load_nodes": ["auth.constraints", "auth.session"],
    "load_files": [
      { "path": "src/handlers/auth.rs", "range": "full" }
    ],
    "system_hint": "Rust project using actix-web."
  },
  "execution_status": "pending",
  "executed_by": null,
  "executed_at": null,
  "retry_count": 0
}
```

---

## 46. Appendix C: Suggested CLI Commands

These are illustrative only.

```bash
# Validation and linting
agm validate auth-platform.agm
agm lint auth-platform.agm

# Rendering
agm render auth-platform.agm --format markdown
agm render auth-platform.agm --format json

# Loading
agm load auth-platform.agm --mode summary --nodes auth.login,auth.constraints
agm load auth-platform.agm --profile operational
agm load auth-platform.agm --profile debug

# Graph and compilation
agm graph auth-platform.agm
agm compile docs/auth.md --out build/auth-platform.agm
agm diff old.agm new.agm

# Orchestration
agm status impl-plan.agm                              # show execution status of all nodes
agm run impl-plan.agm                                 # execute all pending nodes
agm run impl-plan.agm --node backend.repo              # execute a specific node
agm run impl-plan.agm --group 2-backend-models          # execute a specific parallel group
agm run impl-plan.agm --dry-run                        # show what would execute without running
agm retry impl-plan.agm --node migration.025.schema     # retry a failed node
agm reset impl-plan.agm                                # reset all execution state to pending
agm context impl-plan.agm --node frontend.new-task      # show the agent context that would be built
agm verify impl-plan.agm --node backend.repo            # run verification checks for a node
```

---

## 47. Appendix D: Error Code Registry

### Parse errors (AGM-P)

| Code | Severity | Message |
|---|---|---|
| `AGM-P001` | error | Missing required header field: `{field}` |
| `AGM-P002` | error | Invalid node declaration syntax |
| `AGM-P003` | error | Unexpected indentation |
| `AGM-P004` | error | Tab character in indentation (spaces required) |
| `AGM-P005` | error | Unterminated block field |
| `AGM-P006` | error | Duplicate field `{field}` in node `{node}` |
| `AGM-P007` | error | Invalid inline list syntax |
| `AGM-P008` | error | Empty file (no nodes) |
| `AGM-P009` | warning | Unknown field `{field}` in node `{node}` |
| `AGM-P010` | info | File spec version `{file_version}` newer than parser version `{parser_version}` |

### Validation errors (AGM-V)

| Code | Severity | Message |
|---|---|---|
| `AGM-V001` | error | Node `{node}` missing required field: `type` |
| `AGM-V002` | error | Node `{node}` missing required field: `summary` |
| `AGM-V003` | error | Duplicate node ID: `{node}` |
| `AGM-V004` | error | Unresolved reference `{ref}` in `{field}` of node `{node}` |
| `AGM-V005` | error | Cycle detected in `depends`: `{cycle_path}` |
| `AGM-V006` | error | Invalid `execution_status` value: `{value}` |
| `AGM-V007` | error | `valid_from` is after `valid_until` in node `{node}` |
| `AGM-V008` | error | Code block missing required field: `{field}` |
| `AGM-V009` | error | Verify entry missing required field: `{field}` |
| `AGM-V010` | warning | Node type `{type}` typically includes field `{field}` (missing) |
| `AGM-V011` | warning | Summary is empty in node `{node}` |
| `AGM-V012` | warning | Summary exceeds 200 characters in node `{node}` |
| `AGM-V013` | warning | Conflicting active nodes co-loaded: `{node_a}` and `{node_b}` |
| `AGM-V014` | warning | Deprecated node `{node}` missing `replaces` or `superseded_by` |
| `AGM-V015` | error | `target` path is absolute or contains traversal: `{path}` |
| `AGM-V016` | error | Disallowed field `{field}` on node type `{type}` (strict mode) |
| `AGM-V017` | warning | Disallowed field `{field}` on node type `{type}` (standard mode) |
| `AGM-V018` | error | Orchestration group `{group}` references non-existent node `{node}` |
| `AGM-V019` | error | Cycle in orchestration `requires`: `{cycle_path}` |
| `AGM-V020` | error | Invalid `execution_status` transition: `{from}` → `{to}` |
| `AGM-V021` | error | Node ID does not match required pattern: `{node}` |
| `AGM-V022` | error | Memory key does not match required pattern: `{key}` |
| `AGM-V023` | error | Invalid memory action: `{action}` |

### Import errors (AGM-I)

| Code | Severity | Message |
|---|---|---|
| `AGM-I001` | error | Unresolved import: `{package}` |
| `AGM-I002` | error | Import version constraint not satisfied: `{package}@{constraint}` (found `{actual}`) |
| `AGM-I003` | error | Circular import detected: `{cycle_path}` |
| `AGM-I004` | error | Cross-package reference to non-existent node: `{ref}` |
| `AGM-I005` | warning | Import `{package}` is deprecated |

### Runtime errors (AGM-R)

| Code | Severity | Message |
|---|---|---|
| `AGM-R001` | error | Code block target file not found: `{path}` |
| `AGM-R002` | error | Code block anchor/old pattern not found in target: `{pattern}` |
| `AGM-R003` | error | Code block anchor/old matches multiple locations in target |
| `AGM-R004` | error | Verification check failed: `{check_description}` |
| `AGM-R005` | error | Agent context file not found: `{path}` |
| `AGM-R006` | error | Memory operation failed: `{action}` on key `{key}` |
| `AGM-R007` | warning | Node `{node}` retry count exceeded threshold: `{count}` |
| `AGM-R008` | error | Execution timeout for node `{node}` |

---

---

## 48. Appendix E: Conformance Test Requirements

A conformant AGM v1.0 implementation MUST pass the following categories of tests.

### 48.1 Parser conformance

| Category | Test count (minimum) | Description |
|---|---|---|
| Valid files | 20 | Files that MUST parse without errors |
| Invalid syntax | 15 | Files that MUST produce parse errors with correct codes |
| Edge cases | 10 | Unicode, empty fields, maximum nesting, very long lines |
| Round-trip | 5 | Parse → serialize → parse produces identical AST |

### 48.2 Validator conformance

| Category | Test count (minimum) | Description |
|---|---|---|
| Required fields | 10 | Missing type, summary, etc. |
| Reference resolution | 10 | Valid refs, broken refs, cross-package refs |
| Cycle detection | 5 | depends cycles, orchestration requires cycles, import cycles |
| Type schemas | 10 | Required/disallowed field combinations per type |
| Security | 5 | Absolute paths, traversal in target, secrets detection |

### 48.3 JSON canonical form conformance

| Category | Test count (minimum) | Description |
|---|---|---|
| Forward conversion | 10 | AGM text → JSON matches expected output |
| Backward conversion | 10 | JSON → AGM text → JSON round-trip |

### 48.4 Test file distribution

Conformance test files MUST be distributed as part of the spec repository under a `tests/` directory with the following structure:

```
tests/
  parse/
    valid/          # files that MUST parse
    invalid/        # files that MUST fail with specific error codes
    edge/           # edge case files
  validate/
    valid/          # files that MUST validate
    invalid/        # files that MUST produce specific validation errors
  json/
    forward/        # AGM text + expected JSON pairs
    roundtrip/      # JSON → AGM → JSON pairs
  memory/
    operations/     # memory action test scenarios
  orchestration/
    scheduling/     # parallel group and dependency resolution tests
```

Each test file MUST include a header comment specifying the expected outcome:

```agm
# expect: error AGM-V003
# description: duplicate node ID should be rejected
agm: 1
package: test.duplicate
version: 0.1.0

node test.node
type: facts
summary: first

node test.node
type: facts
summary: duplicate
```

---

# Final Notes

AGM v1.0 is the first stable release of the Agent Graph Memory specification. It represents a format that is:

- **Parseable**: deterministic syntax rules with MUST-level enforcement, conformance tests, and a formal error model
- **Verifiable**: type schemas, code blocks with verification contracts, and structured validation
- **Executable**: orchestration primitives, agent context, execution state, and memory persistence
- **Interoperable**: JSON canonical form, cross-package imports, and standardized error codes

The format serves three roles simultaneously:

1. **Knowledge representation** — summary-first nodes with progressive expansion for efficient agent consumption
2. **Implementation protocol** — code blocks, verification contracts, and agent context that transform plans into executable instructions
3. **Orchestration standard** — dependency graphs, parallel groups, execution state, and memory that coordinate multi-agent work

The practical path for adoption:

1. Build a CLI parser that passes the conformance test suite (start with `agm validate` and `agm render`)
2. Implement the runtime loop: parse → schedule → build context → execute → verify → update state
3. Connect memory persistence for cross-execution learning
4. Measure token savings, execution accuracy, and agent coordination efficiency against equivalent Markdown workflows

AGM is most valuable when there is a runtime that brings it to life. The spec defines the contract; the runtime delivers the value.
