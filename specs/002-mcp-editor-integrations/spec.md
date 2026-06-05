# Feature Specification: MCP Editor Integrations for Automatic Log Reduction

**Feature Branch**: `002-mcp-editor-integrations`

**Created**: 2026-06-03

**Status**: Draft

**Input**: MCP server wrapping logreduce for Claude Code, Cursor, and GitHub Copilot, with a Claude Code UserPromptSubmit hook for automatic log interception before the model sees the prompt.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Claude Code Auto-Intercept (Priority: P1)

A developer pastes a large log file into Claude Code and asks "what went wrong?" Without any extra steps, the log is silently reduced to a token budget before Claude sees it. The developer gets a useful diagnosis without hitting token limits or paying $38 per query — and without having to know the reducer exists.

**Why this priority**: This is the highest-value scenario. Automatic, zero-friction interception makes the tool invisible infrastructure. It is also the only editor where true prompt rewriting is possible (via a `UserPromptSubmit` hook).

**Independent Test**: Paste a log file path or raw log content into a Claude Code session. Confirm the model's context contains the reduced form (header summary + compressed body), not the raw log. The original prompt question is preserved unchanged. The developer sees no extra steps.

**Acceptance Scenarios**:

1. **Given** a developer types a prompt containing a local log file path, **When** Claude Code submits the prompt, **Then** the hook intercepts, reduces the file to an 8k-token budget, and the model receives the reduced content inline.
2. **Given** a developer pastes raw log text (≥ 1000 lines) directly in the prompt, **When** Claude Code submits, **Then** the hook detects the log block, reduces it, and the model receives the compressed version.
3. **Given** the prompt contains no log content (normal conversation), **When** Claude Code submits, **Then** the hook is a no-op and the original prompt passes through unchanged.
4. **Given** the log reduction fails or times out, **When** Claude Code submits, **Then** the hook passes the original prompt through unchanged (fail-open), and a warning is appended to the prompt.

---

### User Story 2 — MCP Tool for Any Supporting Editor (Priority: P2)

A developer working in Cursor, VS Code with Copilot, or any MCP-compatible agent manually invokes a `reduce_log` tool when they want to analyse a large log. The model calls the tool, gets back the reduced log, and uses it for diagnosis. No clipboard gymnastics, no external scripts.

**Why this priority**: This works for all MCP-compatible editors with one shared server. It is model-invoked (not automatic), but it is the universal integration path.

**Independent Test**: Connect an MCP client to the server and call `reduce_log` with a log path or raw text. Confirm the tool returns a reduced log respecting the requested token budget and the returned content matches what the `logreduce` binary produces standalone.

**Acceptance Scenarios**:

1. **Given** an MCP client calls `reduce_log(path="app.log", budget=8000)`, **When** the server handles the call, **Then** it returns the reduced log body and summary header within the budget.
2. **Given** an MCP client calls `reduce_log(text="<raw log content>", budget=16000)`, **When** the server handles the call, **Then** it reduces the inline text and returns the result.
3. **Given** `reduce_log` is called with an unreadable path, **When** the server handles the call, **Then** it returns a structured error with a human-readable message (no crash).
4. **Given** `reduce_log` is called with an empty log, **When** the server handles the call, **Then** it returns a valid but empty summary with zero lines kept.

---

### User Story 3 — Cursor / Copilot Guided Workflow (Priority: P3)

A developer in Cursor or GitHub Copilot is nudged by a workspace rule to call the `reduce_log` tool before asking about a large log. A one-time config snippet adds the MCP server and a persistent instruction. After setup, the developer's workflow is: paste log path → the model automatically calls `reduce_log` → asks the developer's question on the result.

**Why this priority**: These editors have no prompt-rewrite hook; "automatic" here is a best-effort instruction rather than a guarantee. The MCP tool (P2) is already the mechanism; this story adds the editor-specific configuration and rule.

**Independent Test**: In a fresh Cursor workspace with the config snippet applied, ask Claude "what caused the errors in app.log?" without explicitly mentioning the reduce tool. Confirm the model calls `reduce_log` before responding to the original question.

**Acceptance Scenarios**:

1. **Given** the MCP server config is applied to a Cursor workspace, **When** a developer asks about a log file, **Then** Cursor's agent model calls `reduce_log` and uses the result in its response.
2. **Given** the config snippet for GitHub Copilot is applied, **When** a developer asks about a log, **Then** Copilot invokes `reduce_log` via MCP and uses the reduced output.
3. **Given** no log is mentioned in the prompt, **When** the developer asks a general question, **Then** `reduce_log` is not invoked.

---

## Functional Requirements *(mandatory)*

### Core Behaviour

- **FR-001** The MCP server exposes a `reduce_log` tool that accepts either a file path or raw log text plus an optional token budget (default 8000).
- **FR-002** `reduce_log` returns a structured JSON object: `{ "text": "<summary header + compressed body>", "stats": { "input_lines": N, "kept_lines": N, "input_tokens": N, "output_tokens": N } }`. The `text` field contains the same content the `logreduce` binary writes to stdout; `stats` surfaces the reduction metrics without requiring the caller to parse the `# LOG SUMMARY` header lines.
- **FR-003** The MCP server is a standalone process communicated with via stdio (the standard MCP transport), requiring no daemon, no port, and no persistent service.
- **FR-004** The Claude Code integration provides a `UserPromptSubmit` hook that detects log content in prompts and calls the reducer before submission — transparently to the developer.
- **FR-005** Log detection in the hook uses heuristics: a local file path resolving to a file ≥ 500 lines, or an inline block of ≥ 500 lines that matches log-line patterns (timestamps + severity keywords).
- **FR-006** The hook is fail-open: if the reduction takes longer than 5 seconds or fails for any reason, the original prompt is passed through unmodified.
- **FR-007** Cursor and Copilot integrations consist of a config snippet (MCP server entry) and a workspace instruction file that instructs the model to call `reduce_log` before analysing logs.
- **FR-008** All parameters accepted by `logreduce` (budget, context window, no-redact flag, format hint) are exposed as optional arguments on `reduce_log`.
- **FR-009** The MCP server must not require an `ANTHROPIC_API_KEY` or any network access — it wraps the local binary only.
- **FR-010** The package ships as a single installable unit: an `npm` package that bundles the MCP server and the Claude Code hook/settings fragment. The `logreduce` binary must already be installed and on PATH — the server locates it via a PATH lookup at startup and exits with a clear error if it is not found.
- **FR-013** Running `npx logreduce-mcp --install` in a project root automatically writes the hook script to `.claude/hooks/UserPromptSubmit.js` and merges the required MCP server entry and hook registration into `.claude/settings.json`. The installer is idempotent (re-running it is safe).

### Security & Privacy

- **FR-011** The same secret/PII redaction that `logreduce` applies by default is applied before any log content is returned from the MCP tool. The raw log never leaves the local machine.
- **FR-012** The Claude Code hook must not log, store, or transmit prompt content; it only pipes text through the local reducer binary.

---

## Success Criteria *(mandatory)*

- **SC-001** A developer pastes a 10,000-line log into Claude Code and receives a Claude response that references lines from across the log (not just the head), without manually invoking the reducer — within 5 seconds of submission.
- **SC-002** Calling `reduce_log` via MCP on any log that the `logreduce` binary handles produces byte-identical output to running the binary directly with equivalent arguments.
- **SC-003** The MCP server starts in under 1 second on a typical developer machine (cold start from `npx` or equivalent).
- **SC-004** The Claude Code `UserPromptSubmit` hook adds less than 3 seconds of latency for a 10,000-line log on a modern laptop.
- **SC-005** Configuration for each editor (Claude Code, Cursor, Copilot) can be applied by a developer in under 5 minutes by following the quickstart.
- **SC-006** The `reduce_log` tool is discoverable: when a developer asks Claude Code "how do I reduce a log?", the model can find and describe the tool from its tool list.
- **SC-007** The hook is a no-op for prompts without log content — zero latency overhead on ordinary conversation turns.

---

## Key Entities *(optional)*

| Entity | Description |
|---|---|
| MCP Server | A stdio-based process exposing the `reduce_log` tool; wraps the `logreduce` binary |
| `reduce_log` Tool | The single MCP tool: accepts `path` or `text`, `budget`, `context`, `no_redact`, `format`; returns reduced log + stats |
| Claude Code Hook | A `UserPromptSubmit` hook script that detects log content, calls the MCP server (or binary directly), and rewrites the prompt |
| Workspace Config | Editor-specific JSON/YAML snippets that register the MCP server and add a persistent reduction instruction |
| Log Detector | Heuristic logic (inside the hook) that determines whether a prompt fragment is a log file path or inline log block |

---

## Scope & Boundaries *(mandatory)*

**In scope**:
- MCP server wrapping `logreduce` (stdio transport)
- `reduce_log` tool with all `logreduce` parameters exposed
- Claude Code `UserPromptSubmit` hook with log-detection heuristics
- Config snippets for Cursor and GitHub Copilot workspaces
- Quickstart documentation for each editor

**Out of scope**:
- Modifying the `logreduce` binary (it ships as-is from feature 001)
- A GUI or visual interface
- Remote/cloud log ingestion (local files only)
- Streaming reduction (batch only)
- Support for editors beyond the three listed
- Authentication between the MCP server and editors

---

## Dependencies & Assumptions *(mandatory)*

**Dependencies**:
- Feature 001 (`logreduce` binary) must be installed and on PATH by the developer before using the MCP server
- Node.js runtime for the MCP server (standard MCP SDK is available as an npm package)
- Claude Code ≥ 2.0 for `UserPromptSubmit` hook support
- Cursor and GitHub Copilot must support MCP (both do as of 2026)

**Assumptions**:
- The `logreduce` binary is installed by the developer (e.g., `cargo install logreduce` or from a release binary); bundling across platforms is deferred to a future version
- Developers accept a one-time config step per editor; ongoing usage is automatic
- Log detection heuristics (500+ lines, timestamp patterns) cover the common case; edge cases (short logs, unusual formats) fall through to normal prompt flow without error
- The MCP stdio server is invoked by the editor's MCP runner — no manual process management required

---

## Clarifications

### Session 2026-06-03

- Q: Should the MCP server npm package bundle the pre-built `logreduce` binary or PATH-lookup an existing install? → A: PATH lookup only — `logreduce` installed separately; MCP server exits with a clear error if not found. Bundling deferred (cross-compilation complexity).
- Q: Should `reduce_log` return plain text or a structured JSON object? → A: Structured JSON `{ text, stats }` — `text` is the full binary stdout, `stats` surfaces metrics without parsing the header.
- Q: How does a developer install the Claude Code hook — manual JSON edit or CLI command? → A: CLI installer — `npx logreduce-mcp --install` writes hook script + settings entry automatically; idempotent.
