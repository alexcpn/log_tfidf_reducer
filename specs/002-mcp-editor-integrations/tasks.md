---
description: "Task list for MCP Editor Integrations"
---

# Tasks: MCP Editor Integrations

**Input**: Design documents from `/specs/002-mcp-editor-integrations/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/mcp-tool.md, contracts/hook.md

**Tests**: INCLUDED — constitution Principle IV (Test-First) applies; vitest tests are first-class tasks.

**Organization**: Tasks grouped by user story (US1 P1 → US2 P2 → US3 P3) so each story is an independently testable increment.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 / US2 / US3

## Path Conventions

All source under `mcp/` at repo root:
- `mcp/src/` — TypeScript source
- `mcp/tests/` — vitest tests
- `.claude/` — hook and settings written by installer (committed by developer)

---

## Phase 1: Setup

**Purpose**: Initialize the `mcp/` npm package and project infrastructure.

- [x] T001 Create `mcp/` directory with `package.json` (`name: logreduce-mcp`, `version: 0.1.0`, `bin: { logreduce-mcp: dist/cli.js }`, `type: module`, `engines: { node: ">=18" }`); add `@modelcontextprotocol/sdk`, `zod` as dependencies and `vitest`, `typescript`, `@types/node` as devDependencies
- [x] T002 [P] Add `mcp/tsconfig.json` targeting ES2022, `module: NodeNext`, `outDir: dist`, `strict: true`; add `mcp/.gitignore` excluding `dist/`, `node_modules/`
- [x] T003 [P] Add `mcp/vitest.config.ts` with Node environment; add `"test": "vitest run"` script in `package.json`; document quality gates (`npm test`, `npm run build`, `tsc --noEmit`) in `mcp/README.md`
- [x] T004 [P] Create empty module stubs: `mcp/src/server.ts`, `mcp/src/reducer.ts`, `mcp/src/detector.ts`, `mcp/src/parser.ts`, `mcp/src/install.ts`, `mcp/src/cli.ts`, `mcp/src/hook.js`; create `mcp/tests/` directory

---

## Phase 2: Foundational

**Purpose**: Core binary-invocation wrapper and output parser that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T005 Implement `mcp/src/reducer.ts`: export `findBinary(): string` (PATH lookup via `execSync("which logreduce")`, throws with install instructions if not found) and `invokeReducer(args: string[], input?: string, timeoutMs?: number): Promise<{ stdout: string; stderr: string }>` using `child_process.execFile` promisified; enforce configurable timeout (default 30s); surface binary exit-code errors
- [x] T006 [P] Implement `mcp/src/parser.ts`: export `parseOutput(stdout: string, stderr: string): ReduceLogOutput` that extracts `input_lines`, `kept_lines` from the `# LOG SUMMARY | N lines → M kept` header, and `input_tokens`, `output_tokens` from the stats stderr lines; returns `{ text: stdout, stats: { ... } }`
- [x] T007 [P] Write `mcp/tests/reducer.test.ts`: test `findBinary` throws when binary absent (mock PATH); test `invokeReducer` on `sample_logs/plain.log` returns non-empty stdout; test timeout behavior (mock slow binary)
- [x] T008 [P] Write `mcp/tests/parser.test.ts`: golden-output test — feed known logreduce stdout + stderr, assert correct `stats.*` values; test malformed header returns zero stats without throwing

**Checkpoint**: Binary wrapper and parser verified ✅

---

## Phase 3: User Story 1 — MCP Tool for Any Editor (P2 — foundational for all editors)

**Goal**: A working `reduce_log` MCP tool that any MCP-compatible editor can call.

**Note**: P2 is implemented before P1 here because the Claude Code hook (P1) builds on the same MCP server. Delivering the MCP server first gives a testable increment for all editors before adding the hook layer.

**Independent Test**: Connect an MCP test client, call `reduce_log(path=sample_logs/plain.log, budget=4000)`, confirm the response is a JSON object with `text` (containing `# LOG SUMMARY`) and `stats.kept_lines > 0`. Confirm `reduce_log(text="")` returns stats with 0 lines.

### Tests for User Story 1

- [x] T009 [P] [US1] Write `mcp/tests/server.test.ts`: test `reduce_log` with `path=sample_logs/plain.log` returns `{ text, stats }` matching binary direct output (byte-identical `text`); test with inline `text`; test error on both path+text provided; test error on neither provided; test graceful error when `logreduce` not on PATH
- [x] T010 [P] [US1] Write `mcp/tests/parser.test.ts` additional cases: `reduce_log` on empty log returns `{ text: "<header>", stats: { input_lines: 0, kept_lines: 0, ... } }`; stat fields are all non-negative integers

### Implementation for User Story 1

- [x] T011 [US1] Implement `mcp/src/server.ts`: create `McpServer` (`@modelcontextprotocol/sdk`), register `reduce_log` tool with zod input schema matching `contracts/mcp-tool.md` (path xor text, budget, context, no_redact, format), call `invokeReducer` with appropriate flags, call `parseOutput`, return `{ content: [{ type: "text", text: JSON.stringify(result) }] }`; handle all error cases from `contracts/mcp-tool.md`; never use `console.log` (use `console.error`)
- [x] T012 [US1] Implement `mcp/src/cli.ts`: parse `--install` flag (delegate to `install.ts`); default behaviour starts the MCP stdio server via `StdioServerTransport`; call `findBinary()` at startup and exit with clear message if not found; add `"start": "node dist/cli.js"` script
- [x] T013 [US1] Run `npm run build` (`tsc`), run `npm test`, fix any failures; verify `node dist/cli.js --version` exits cleanly

**Checkpoint**: `reduce_log` MCP tool working ✅ — any MCP-compatible editor can call it now

---

## Phase 4: User Story 2 — Claude Code Auto-Intercept (P1)

**Goal**: A `UserPromptSubmit` hook that automatically detects and reduces log content before Claude sees it.

**Independent Test**: Feed a JSON prompt containing a path to a 600-line log (`sample_logs/plain.log` repeated) via stdin to `node .claude/hooks/UserPromptSubmit.js`; confirm stdout contains `{ "prompt": "...<LOG SUMMARY>..." }`. Feed a normal prompt; confirm stdout is `{}` within 50ms.

### Tests for User Story 2

- [x] T014 [P] [US2] Write `mcp/tests/detector.test.ts`: test file-path detection (prompt contains path to a ≥500-line file → `detected: true, type: "path"`); test inline block detection (500+ lines matching log pattern → `detected: true, type: "inline"`); test short prompt → `detected: false`; test path to non-existent file → `detected: false`; test path to <500 line file → `detected: false`
- [x] T015 [P] [US2] Write `mcp/tests/install.test.ts`: test `--install` in a temp dir creates `.claude/hooks/UserPromptSubmit.js`; creates `.claude/settings.json` with correct hook registration; running `--install` twice leaves settings unchanged (idempotent); test that existing settings keys are preserved

### Implementation for User Story 2

- [x] T016 [US2] Implement `mcp/src/detector.ts`: export `detectLog(prompt: string): LogDetectionResult`; file-path check: find path-like substrings, check file exists, count lines (≥500 → detected); inline block check: split prompt into lines, find run of ≥500 lines where ≥70% match `/\d{2}[:\/]\d{2}|^\s*(ERROR|WARN|INFO|DEBUG|FATAL)/im`; return `{ detected, type, content, startIndex, endIndex }`
- [x] T017 [US2] Implement `mcp/src/hook.js` (plain CommonJS, no TypeScript): read stdin synchronously (`fs.readFileSync(0, "utf8")`), parse JSON `{ prompt }`; call `detectLog`; if detected, call `logreduce` via `execFileSync` with 5s timeout (fail-open on error/timeout); splice reduced content into prompt using `startIndex`/`endIndex`; write `JSON.stringify({ prompt: rewritten })` or `{}` to stdout; all errors → `{}` (never exit code 2)
- [x] T018 [US2] Implement `mcp/src/install.ts`: locate the installed `hook.js` (resolve relative to the install script); create `.claude/hooks/` dir; copy `hook.js` to `.claude/hooks/UserPromptSubmit.js`; read/create `.claude/settings.json`; deep-merge `{ hooks: { UserPromptSubmit: [{ type: "command", command: "node .claude/hooks/UserPromptSubmit.js" }] } }`; add MCP server entry; write back; print success summary
- [x] T019 [US2] Manual end-to-end test: run `node mcp/dist/cli.js --install`; confirm `.claude/` files created; run `echo '{"session_id":"t","prompt":"check sample_logs/plain.log"}' | node .claude/hooks/UserPromptSubmit.js`; confirm rewritten prompt contains `# LOG SUMMARY` (SC-001/SC-004 verification)

**Checkpoint**: Claude Code auto-intercept working ✅

---

## Phase 5: User Story 3 — Cursor / Copilot Guided Workflow (P3)

**Goal**: Config snippets and workspace instructions for Cursor and GitHub Copilot.

**Independent Test**: Apply the Cursor config snippet to a fresh `.cursor/mcp.json`; verify the MCP server appears in Cursor's tool list. Apply the Copilot config; verify `reduce_log` is callable in Agent mode.

### Implementation for User Story 3

- [x] T020 [P] [US3] Create `mcp/config/cursor-mcp.json`: `{ "mcpServers": { "logreduce": { "command": "npx", "args": ["logreduce-mcp"] } } }`; create `mcp/config/copilot-mcp.json`: `{ "servers": { "logreduce": { "command": "npx", "args": ["logreduce-mcp"] } } }`; create `mcp/config/copilot-instructions.md` with the workspace rule (call reduce_log before analysing any log)
- [x] T021 [P] [US3] Update `mcp/src/install.ts` to accept `--editor cursor|copilot` flag: copy the relevant config snippet to `.cursor/mcp.json` or prompt the user to add the copilot config manually (with instructions printed to stdout); update installer tests in `mcp/tests/install.test.ts`
- [x] T022 [P] [US3] Update `specs/002-mcp-editor-integrations/quickstart.md` with accurate copy-paste snippets now that the config files are generated; verify all three editor sections are complete and tested

**Checkpoint**: All three editors configured ✅

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T023 [P] Publish dry-run: `npm pack` in `mcp/`; inspect tarball contents (`tar -tf logreduce-mcp-*.tgz`) to confirm `dist/`, `src/hook.js`, `config/` are included and `node_modules/`, `tests/` are excluded; adjust `.npmignore` as needed
- [x] T024 [P] Timing benchmarks (SC-003/SC-004): `time npx logreduce-mcp --version` (cold start, must be < 1s); `time echo '{"session_id":"t","prompt":"..."} ' | node .claude/hooks/UserPromptSubmit.js` with a 10k-line prompt (must be < 3s)
- [x] T025 [P] Fail-open validation (FR-006): point `PATH` to a directory with a broken `logreduce` stub; confirm hook returns `{}` within 5s; confirm MCP server returns an `InternalError` (not a crash)
- [x] T026 [P] SC-002 byte-identity check: run `logreduce sample_logs/plain.log --budget 4000` → save to `direct.txt`; call `reduce_log` via MCP with same args; compare `result.text` to `direct.txt` — must be identical
- [x] T027 Update `mcp/README.md` with build/test/install steps matching the shipped CLI; update `specs/002-mcp-editor-integrations/quickstart.md` if any steps changed during implementation

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies — start immediately
- **Foundational (Phase 2)**: depends on Setup (binary wrapper + parser used by all stories)
- **US1 — MCP Tool (Phase 3)**: depends on Foundational; delivers the MCP server for all editors
- **US2 — Claude Code Hook (Phase 4)**: depends on Foundational; independent of US1 (hook calls binary directly, not via MCP server)
- **US3 — Cursor/Copilot (Phase 5)**: depends on US1 (references the running MCP server); independent of US2
- **Polish (Phase 6)**: depends on all user story phases

### Parallel Opportunities

```bash
# Setup: T002, T003, T004 in parallel
# Foundational: T006, T007, T008 in parallel (T005 must complete first)
# US1 tests: T009, T010 in parallel
# US2 tests: T014, T015 in parallel; T016 then T017 then T018 (sequential, same detector→hook→install chain)
# US3: T020, T021, T022 in parallel
# Polish: T023–T027 all in parallel
```

---

## Implementation Strategy

### MVP First (US1 only — MCP Tool)

1. Phase 1 Setup → 2. Phase 2 Foundational → 3. Phase 3 US1 → **STOP & validate**

This alone delivers `reduce_log` for Cursor and Copilot Agent mode. The hook (US2) and config snippets (US3) are additive.

### Incremental Delivery

1. Setup + Foundational → binary wrapper ready
2. US1 → MCP server working for any MCP editor
3. US2 → Claude Code automatic interception
4. US3 → Cursor/Copilot config snippets
5. Polish → timing benchmarks, fail-open, byte-identity, packaging

---

## Notes

- [P] = different files / independent; safe to parallelize
- Constitution Principle IV: tests before implementation (T009/T010 before T011/T012)
- Hook (`hook.js`) is plain CommonJS — no build step required at runtime
- Server (`server.ts`) must never use `console.log` (corrupts MCP JSON-RPC stream)
- `execFileSync` in hook enforces 5s fail-open; `execFileAsync` in MCP server allows 30s for large logs
