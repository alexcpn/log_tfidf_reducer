# Research: MCP Editor Integrations

## R1 — MCP SDK for Node.js

**Decision**: Use `@modelcontextprotocol/sdk` (official npm package).

**Pattern**:
```typescript
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

const server = new McpServer({ name: "logreduce-mcp", version: "1.0.0" });
server.tool("reduce_log", { /* zod schema */ }, async (args) => ({ ... }));
await server.connect(new StdioServerTransport());
```

**Critical**: never use `console.log()` in an MCP stdio server — it corrupts the JSON-RPC stream. Use `console.error()` for all diagnostics.

**Alternatives considered**: Python MCP SDK (also official, but Node.js is more standard for editor tooling); raw JSON-RPC without SDK (too much boilerplate).

## R2 — Claude Code UserPromptSubmit Hook

**Hook input (stdin)**: Claude Code sends a JSON object containing, at minimum:
```json
{ "session_id": "...", "prompt": "<user prompt text>", ... }
```

**Hook output (stdout)**: The hook returns a JSON object. To modify the prompt before the model sees it:
```json
{ "prompt": "<rewritten prompt>" }
```
To pass through unchanged, return `{}` or `{ "continue": true }`. To block, return exit code 2 with a message on stderr.

**Hook script location**: `.claude/hooks/UserPromptSubmit.js` (project-level) or `~/.claude/hooks/UserPromptSubmit.js` (user-level).

**Settings registration** (`.claude/settings.json`):
```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "type": "command",
      "command": "node .claude/hooks/UserPromptSubmit.js"
    }]
  }
}
```

**Alternatives considered**: Hooking at a different lifecycle event — `PreToolUse` fires after the model has already seen the prompt, so it cannot reduce before the model reads it. `UserPromptSubmit` is the only correct hook for pre-model interception.

## R3 — Cursor MCP Configuration

**File**: `.cursor/mcp.json` (project-level) or `~/.cursor/mcp.json` (user-level).

**Format**:
```json
{
  "mcpServers": {
    "logreduce": {
      "command": "npx",
      "args": ["logreduce-mcp"]
    }
  }
}
```

Cursor has a ceiling of ~40 active MCP tools across all servers. The `reduce_log` tool is the only one from this server, well within limits.

## R4 — GitHub Copilot (VS Code) MCP Configuration

**File**: VS Code user MCP config, accessed via Command Palette → "MCP: Open User Configuration". Root key is `"servers"` (different from Cursor's `"mcpServers"`).

**Format**:
```json
{
  "servers": {
    "logreduce": {
      "command": "npx",
      "args": ["logreduce-mcp"]
    }
  }
}
```

MCP tools are only available in Copilot **Agent mode** (not chat mode).

**Workspace rule** (`.github/copilot-instructions.md` or `.vscode/settings.json`):
```
When a user asks you to analyse a log file or log content, first call the reduce_log tool to compress the log to a token budget before proceeding.
```

## R5 — Child Process Invocation from Hook (synchronous)

**Decision**: Use `execSync` for the hook (must return synchronously); use `spawn` with promise wrapping for the MCP server handler (async is fine there).

**Hook pattern**:
```javascript
const { execFileSync } = require("child_process");

function reduceLogs(logText, budget = 8000) {
  return execFileSync("logreduce", ["--budget", String(budget)], {
    input: logText,
    encoding: "utf8",
    timeout: 5000,   // FR-006: 5-second fail-open timeout
  });
}
```

**MCP server pattern**:
```typescript
import { execFile } from "child_process";
import { promisify } from "util";
const execFileAsync = promisify(execFile);

const { stdout } = await execFileAsync("logreduce", args, { input: text, timeout: 30_000 });
```

## R6 — Binary PATH Resolution

At startup, the MCP server checks for `logreduce` on PATH using `which`/`where` (or Node's `fs.accessSync` on known paths). If not found, it exits with a clear error message printed to stderr before the stdio server starts.

```typescript
import { execSync } from "child_process";

function findBinary(): string {
  try {
    return execSync("which logreduce", { encoding: "utf8" }).trim();
  } catch {
    throw new Error(
      "logreduce not found on PATH. Install it with `cargo install logreduce` and ensure ~/.cargo/bin is on your PATH."
    );
  }
}
```

## R7 — Output Parsing (stats extraction)

The `logreduce` binary writes a `# LOG SUMMARY | N lines → M kept (P%)` header to stdout. The MCP server parses this to populate the `stats` object:

```typescript
function parseStats(output: string): Stats {
  const match = output.match(/(\d[\d,]*) lines → (\d[\d,]*) kept/);
  const tokenMatch = output.match(/(\d[\d,]*)\s*→\s*(\d[\d,]*)/g);
  return {
    input_lines: parseInt((match?.[1] ?? "0").replace(/,/g, "")),
    kept_lines:  parseInt((match?.[2] ?? "0").replace(/,/g, "")),
    input_tokens: 0,  // extracted from header if --stats flag added
    output_tokens: 0,
  };
}
```

**Decision**: Add `--stats` flag to the binary invocation; the binary already prints token counts when `--stats` is passed. Parse `input_tokens` and `output_tokens` from the stderr stats output.

## R8 — Test Framework

**Decision**: Vitest (faster than Jest, first-class TypeScript + ESM support, compatible with Node 18+).

**Alternatives considered**: Jest (more mature ecosystem but slower cold start; heavier config for ESM); Mocha (no built-in assertions; more boilerplate).
