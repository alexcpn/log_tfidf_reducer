# logreduce-mcp

Stop paying $38 to ask Claude about a log file.

`logreduce-mcp` is an MCP server that automatically compresses large log files before they reach any AI model — reducing token usage by up to 99.9% while keeping every error and unique event.

## Prerequisites

**Step 1 — install the `logreduce` binary:**

```bash
cargo install logreduce
```

> Requires Rust. Install Rust at https://rustup.rs  
> After installing, make sure `~/.cargo/bin` is on your PATH.

Verify: `logreduce --version`

**Step 2 — install the MCP server:**

```bash
npm install -g logreduce-mcp
```

Verify: `logreduce-mcp --version`

---

## Claude Code (automatic — zero extra steps after setup)

The Claude Code integration intercepts large logs *before the model sees them*. You just paste a log path or content and ask your question normally.

**Run the installer:**

```bash
# In your project root
npx logreduce-mcp --install
```

This writes two things:
- `.claude/hooks/UserPromptSubmit.js` — the interception hook
- `.claude/settings.json` — registers the hook and MCP server

**That's it.** Start a new Claude Code session. Paste a log file path in any prompt:

```
What caused the errors in /var/log/app.log?
```

The log is silently reduced (e.g. 50,000 lines → 300 lines) before Claude reads it.

**What gets reduced automatically:**
- Any file path in the prompt pointing to a file with ≥ 500 lines
- Any inline log block of ≥ 500 lines pasted directly in the prompt

**Verify it's working:**

```bash
echo '{"session_id":"test","prompt":"check /var/log/syslog"}' \
  | node .claude/hooks/UserPromptSubmit.js
# → {"prompt":"check \n--- LOG (reduced ...) ---\n# LOG SUMMARY..."} 
```

---

## Cursor

**Step 1 — add to `.cursor/mcp.json`** in your project root (create if it doesn't exist):

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

Or run the installer:

```bash
npx logreduce-mcp --install --editor=cursor
```

**Step 2 — reload Cursor.** The `reduce_log` tool appears in the MCP tool list.

**Step 3 — add a workspace rule** so Cursor calls it automatically.  
Create or edit `.cursorrules` (or your system prompt):

```
When asked to analyse a log file or log content, first call the reduce_log 
tool to compress it to an 8000-token budget before proceeding.
```

**Usage:** In a Cursor agent chat, either ask about a log directly and let Cursor call `reduce_log` automatically, or invoke it explicitly:

```
Use reduce_log on /var/log/nginx/error.log with budget 16000
```

---

## GitHub Copilot (VS Code)

**Step 1 — open the MCP config:**  
Command Palette → `MCP: Open User Configuration`

**Step 2 — add the server:**

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

**Step 3 — add a workspace instruction.**  
Create `.github/copilot-instructions.md` in your repo:

```markdown
When a user asks about a log file or shares log content, 
call the reduce_log MCP tool first to compress the log 
before analysing it.
```

**Step 4 — switch to Agent mode** in Copilot Chat (MCP tools only work in Agent mode).

**Usage:** Ask Copilot about a log in Agent mode. It will call `reduce_log` before responding.

---

## The `reduce_log` tool (manual use)

In any MCP-enabled agent session you can also call the tool directly:

```
Use reduce_log with path=/var/log/app.log
Use reduce_log with path=/var/log/app.log budget=32000
Use reduce_log with text="<paste log here>"
```

**Parameters:**

| Parameter | Default | Description |
|---|---|---|
| `path` | — | Absolute path to a local log file |
| `text` | — | Raw log content (alternative to path) |
| `budget` | `8000` | Token budget (100–200,000) |
| `context` | `2` | Context lines around each kept line |
| `no_redact` | `false` | Disable secret/PII redaction |
| `format` | `auto` | Force format: `json \| logfmt \| syslog \| klog \| plain` |

**Response:** `{ "text": "<reduced log>", "stats": { "input_lines": N, "kept_lines": M, "input_tokens": N, "output_tokens": M } }`

---

## How it works

`logreduce` ranks log lines using TF-IDF over masked templates, blended with severity weighting. It guarantees:

- Every distinct message type gets at least one representative
- All ERROR/FATAL lines are surfaced (crash-loop errors are never silently dropped)
- Repeated lines are collapsed with a count annotation: `(×500)`
- Secrets and PII are redacted before the log reaches the model

See the [main repo README](../README.md) for the full technical write-up.

---

## Troubleshooting

**`logreduce: command not found`**  
Add `~/.cargo/bin` to your PATH: `export PATH="$HOME/.cargo/bin:$PATH"`  
Add this line to your `~/.bashrc` or `~/.zshrc` to make it permanent.

**Hook not triggering in Claude Code**  
Ensure `.claude/settings.json` exists with a `hooks.UserPromptSubmit` entry.  
Re-run `npx logreduce-mcp --install` to fix it.

**Cursor doesn't show the reduce_log tool**  
Check `.cursor/mcp.json` exists and reload Cursor. Cursor requires the MCP server to start successfully — run `npx logreduce-mcp` manually to check for errors.

**Log not being detected by the hook**  
The hook triggers for files ≥ 500 lines or inline blocks ≥ 500 lines. For smaller logs, call `reduce_log` manually.
