# Implementation Plan: MCP Editor Integrations

**Branch**: `002-mcp-editor-integrations` | **Date**: 2026-06-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-mcp-editor-integrations/spec.md`

## Summary

Expose the `logreduce` Rust binary to Claude Code, Cursor, and GitHub Copilot via a Node.js/TypeScript MCP stdio server (`reduce_log` tool) plus a Claude Code `UserPromptSubmit` hook that automatically intercepts and reduces large log content before the model sees it. No changes to the Rust binary. Delivered as an npm package (`logreduce-mcp`) installable with `npx`.

## Technical Context

**Language/Version**: Node.js 18+ / TypeScript 5.4

**Primary Dependencies**: `@modelcontextprotocol/sdk` (MCP stdio server), `zod` (input validation), `vitest` (tests)

**Storage**: None — stateless; the MCP server spawns the `logreduce` binary as a child process per call

**Testing**: Vitest (faster cold start than Jest; native ESM/TypeScript support)

**Target Platform**: Cross-platform (linux, macOS, Windows); binary invoked via PATH lookup

**Project Type**: npm package (MCP stdio server + CLI installer)

**Performance Goals**: SC-003 (1s cold start), SC-004 (< 3s for 10k lines in hook)

**Constraints**: No network calls; no API keys; `logreduce` binary on PATH (no bundling); hook is fail-open within 5s

**Scale/Scope**: Single-user developer tool; one MCP server process per editor session

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Applies to 002? | Status |
|---|---|---|
| I. Performance Is a Contract | Indirectly — wrapper must not add significant latency | ✅ SC-003/SC-004 targets set; no heavy work in JS layer |
| II. Fidelity Over Compression | Fully delegated to the Rust binary | ✅ Wrapper adds no reduction logic |
| III. Mask Before You Score | Fully delegated to the Rust binary | ✅ Wrapper passes flags through unchanged |
| IV. Test-First with Golden Outputs | Yes — vitest tests required | ✅ Tests precede implementation in task plan |
| V. CLI Contract & Decoupling | Yes — MCP server makes zero network calls | ✅ Local subprocess wrapper only |

**Constitution gate: PASSED** — no violations. This feature wraps, not replaces, the binary.

## Project Structure

### Documentation (this feature)

```text
specs/002-mcp-editor-integrations/
├── plan.md           ← this file
├── research.md       ← R1–R8 decisions
├── data-model.md     ← entities: ReduceLogInput/Output, HookInput/Output, etc.
├── contracts/
│   ├── mcp-tool.md   ← reduce_log tool schema + error contract
│   └── hook.md       ← UserPromptSubmit hook I/O contract
├── quickstart.md     ← per-editor setup guide
└── tasks.md          ← Phase 2 output (not yet generated)
```

### Source Code (repository root)

```text
mcp/                              # npm package root
├── package.json                  # name: logreduce-mcp, bin: { logreduce-mcp: dist/cli.js }
├── tsconfig.json
├── src/
│   ├── server.ts                 # MCP stdio server entry, reduce_log tool registration
│   ├── reducer.ts                # child_process wrapper: invoke logreduce binary
│   ├── detector.ts               # log detection heuristics for the hook
│   ├── parser.ts                 # parse logreduce stdout/stderr → ReduceLogOutput
│   ├── install.ts                # --install CLI: writes hook script + settings.json
│   └── hook.js                   # plain JS hook (no TS transpile at runtime)
├── tests/
│   ├── server.test.ts
│   ├── reducer.test.ts
│   ├── detector.test.ts
│   ├── parser.test.ts
│   └── install.test.ts
└── dist/                         # compiled JS (gitignored)

.claude/                          # written by --install; committed by developer
├── settings.json                 # MCP server + hook registration
└── hooks/
    └── UserPromptSubmit.js       # copy of mcp/src/hook.js
```

## Key Design Decisions

### D1 — Plain JS hook, TypeScript server

The hook (`hook.js`) runs from `.claude/hooks/` with zero build step — no transpilation at runtime. TypeScript is used for the MCP server (build once via `tsc`; runs from `dist/`).

### D2 — Fail-open everywhere

Both the MCP server error handlers and the hook's timeout/error path pass through the original input unchanged. A broken reducer must never prevent a developer from asking their question.

### D3 — `--stats` always passed

The MCP server always passes `--stats` to the binary so token counts appear in stderr. The parser reads both stdout (reduced log) and stderr (stats lines) to populate `stats.*`. Stats are not surfaced in `text` — only `text` and the `stats` object are returned.

### D4 — Inline text via stdin

When `text` is provided, the server writes it to the binary's stdin — no temp files needed. The binary already reads stdin when no file argument is given.

### D5 — Installer merges, not overwrites

`--install` deep-merges new entries into the existing `.claude/settings.json`, never overwrites the whole file. Hook script is always overwritten (ensures installed version is current).

## Complexity Tracking

| Area | Complexity | Justification |
|---|---|---|
| MCP server + tool | Low | SDK handles protocol; one tool, simple subprocess |
| Log detection heuristics | Medium | File-path detection easy; inline block detection (70% log-line pattern) needs care |
| Hook prompt rewriting | Medium | Locating the log fragment in the prompt and splicing the replacement is non-trivial |
| Installer (settings merge) | Low-Medium | JSON merge is simple; edge cases (corrupt JSON, missing dirs) need handling |
| Cross-platform PATH resolution | Low | `which`/`where` via execSync; fallback to known cargo paths |
| Stats parsing | Low | Regex on known header format |

## Phase 0: Research ✅

See `research.md` for all decisions. Summary:
- MCP SDK: `@modelcontextprotocol/sdk` (official)
- Hook I/O: JSON on stdin/stdout; prompt rewritten by returning `{ "prompt": "..." }`
- Cursor: `.cursor/mcp.json` with `mcpServers` key
- Copilot: VS Code MCP user config with `servers` key
- Binary invocation: `execFileSync` in hook (5s timeout), `execFileAsync` in MCP server
- Test framework: Vitest

## Phase 1: Design ✅

- `data-model.md`: ReduceLogInput, ReduceLogOutput, HookInput, HookOutput, LogDetectionResult, InstallConfig
- `contracts/mcp-tool.md`: reduce_log tool schema, error codes, invocation semantics
- `contracts/hook.md`: hook I/O contract, detection heuristics, timing constraints, prompt rewrite format
- `quickstart.md`: per-editor install + verification steps

## Verification Plan

1. `cd mcp && npm test` — vitest unit tests for all modules
2. End-to-end: `npx logreduce-mcp --install` → check `.claude/settings.json` + hook script written
3. MCP client test: call `reduce_log(path=sample_logs/plain.log)`, compare `text` to binary stdout
4. Hook test: feed prompt with 600-line inline log → rewritten prompt contains `# LOG SUMMARY`
5. Timing: `time echo '...' | node .claude/hooks/UserPromptSubmit.js` on 10k-line log → < 3s (SC-004)
6. Fail-open: point PATH to a broken binary → hook must return `{}` within 5s, not crash
