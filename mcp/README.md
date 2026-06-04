# logreduce-mcp

MCP server that wraps the [`logreduce`](https://github.com/alexcpn/log_tfidf_reducer) Rust binary,
exposing a `reduce_log` tool to Claude Code, Cursor, and GitHub Copilot.

## Prerequisites

- `logreduce` on PATH: `cargo install logreduce` (then ensure `~/.cargo/bin` is on your PATH)
- Node.js ≥ 18

## Install & setup

```bash
# Claude Code (automatic hook + MCP server)
npx logreduce-mcp --install

# Cursor — add to .cursor/mcp.json:
# { "mcpServers": { "logreduce": { "command": "npx", "args": ["logreduce-mcp"] } } }

# GitHub Copilot — add to VS Code MCP user config:
# { "servers": { "logreduce": { "command": "npx", "args": ["logreduce-mcp"] } } }
```

## Quality gates

```bash
cd mcp
npm install
npm run typecheck   # tsc --noEmit
npm run build       # tsc
npm test            # vitest run
```

## Tool: reduce_log

Accepts `path` (file path) or `text` (inline log) plus optional `budget`, `context`,
`no_redact`, `format`. Returns `{ text: "<reduced log>", stats: { input_lines, kept_lines, input_tokens, output_tokens } }`.
