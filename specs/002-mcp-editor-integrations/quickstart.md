# Quickstart: MCP Editor Integrations

## Prerequisites

- `logreduce` on PATH: `cargo install logreduce` (or download a release binary and add to PATH)
- Node.js 18+
- One of: Claude Code ≥ 2.0, Cursor, or VS Code with GitHub Copilot (Agent mode)

Verify: `logreduce --version` and `node --version`

## Install the MCP server

```bash
npm install -g logreduce-mcp
# or use npx for one-off use: npx logreduce-mcp
```

---

## Claude Code (automatic log interception)

Run the installer in your project root:

```bash
npx logreduce-mcp --install
```

This writes:
- `.claude/hooks/UserPromptSubmit.js` — the intercept hook
- Merges the MCP server entry + hook registration into `.claude/settings.json`

**Verify**:

```bash
# Check settings were written
cat .claude/settings.json | grep logreduce

# Test the hook manually
echo '{"session_id":"test","prompt":"Analyse /var/log/syslog"}' | \
  node .claude/hooks/UserPromptSubmit.js
# → { "prompt": "..." } if the file has ≥500 lines
# → {}                  if the file is small or doesn't exist
```

**Usage**: paste a log file path or raw log content into any Claude Code prompt. The hook runs automatically — no extra steps.

---

## Cursor

Add to `.cursor/mcp.json` in your project root (create if missing):

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

Reload Cursor. The `reduce_log` tool appears in the MCP tool list.

**Usage**: in a Cursor agent chat, either invoke `reduce_log` explicitly or let Cursor call it automatically if you've added this workspace instruction (`.cursorrules` or system prompt):

```
When analysing a log file or log content, first call reduce_log to compress it to an 8000-token budget.
```

---

## GitHub Copilot (VS Code, Agent mode)

Open the VS Code MCP user config via Command Palette → **"MCP: Open User Configuration"** and add:

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

Add a workspace instruction in `.github/copilot-instructions.md`:

```markdown
When a user asks about a log file or shares log content, call the reduce_log MCP tool first to compress the log before analysing it.
```

**Usage**: switch to **Agent mode** in Copilot Chat, then ask about a log file. Copilot will call `reduce_log` automatically before responding.

---

## Manual tool invocation (any editor)

Once the MCP server is configured, you can call the tool manually in any agent session:

```
Use reduce_log with path=/var/log/app.log and budget=16000
```

Or with inline text:

```
Use reduce_log with text="<paste log here>" and budget=8000
```

---

## Verify end-to-end (SC-001 through SC-007)

```bash
# SC-001/004: time the hook on a 10k-line log
time echo '{"session_id":"test","prompt":"what happened in /tmp/app.log"}' | \
  node .claude/hooks/UserPromptSubmit.js

# SC-002: confirm MCP tool output matches binary directly
logreduce sample_logs/plain.log --budget 4000 > direct.txt
# (call reduce_log via MCP with same args and compare text field to direct.txt)

# SC-003: cold-start time
time npx logreduce-mcp --version

# SC-005: setup timer — from "cargo install logreduce" to working Claude Code hook
# Target: < 5 minutes
```
