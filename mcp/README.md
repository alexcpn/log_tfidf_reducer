# logreduce-mcp

[![npm version](https://img.shields.io/npm/v/logreduce-mcp.svg)](https://www.npmjs.com/package/logreduce-mcp)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../LICENSE)

**Stop paying $38 to ask an AI about a log file.**

`logreduce-mcp` is an MCP server that compresses large log files before they reach any AI model — reducing token usage by up to 99.9% while keeping every error, warning, and unique event.

```
Before:  1,000,000 lines  →  12,803,209 tokens  →  ~$38/question
After:         374 lines  →       7,504 tokens  →  ~$0.02/question
```

Works with **Claude Code**, **Cursor**, and **GitHub Copilot**.

---

## Prerequisites

### 1. Install `logreduce` (the Rust binary)

**Option A — Download a pre-built binary (no Rust required):**

Go to the [latest release](https://github.com/alexcpn/log_tfidf_reducer/releases/latest) and download the binary for your platform:

| Platform | File |
|---|---|
| Linux x64 | `logreduce-linux-x64` |
| macOS Apple Silicon | `logreduce-darwin-arm64` |
| macOS Intel | `logreduce-darwin-x64` |
| Windows x64 | `logreduce-win32-x64.exe` |

**Linux / macOS** — save to somewhere on your PATH and make it executable:

```bash
# Example for Linux x64
curl -L https://github.com/alexcpn/log_tfidf_reducer/releases/latest/download/logreduce-linux-x64 \
  -o /usr/local/bin/logreduce
chmod +x /usr/local/bin/logreduce
```

**Windows** — download `logreduce-win32-x64.exe`, rename it to `logreduce.exe`, and place it in a folder on your `PATH` (e.g. `C:\tools\`):

```powershell
Invoke-WebRequest -Uri "https://github.com/alexcpn/log_tfidf_reducer/releases/latest/download/logreduce-win32-x64.exe" `
  -OutFile "C:\tools\logreduce.exe"
```

**Option B — Build from source (requires Rust):**

```bash
cargo install logreduce
# Make sure ~/.cargo/bin is on your PATH
```

Verify: `logreduce --version`

### 2. Install this MCP server

```bash
npm install -g logreduce-mcp
```

Verify: `logreduce-mcp --version`

---

## Integration: Claude Code

Claude Code supports a `UserPromptSubmit` hook that rewrites your prompt *before the model sees it*. This means you just ask your question normally — the log is compressed automatically, invisibly.

### Setup (one time, per project)

```bash
cd your-project/
npx logreduce-mcp --install
```

This creates two files:

```
your-project/
└── .claude/
    ├── settings.json          ← registers the MCP server and hook
    └── hooks/
        └── UserPromptSubmit.js  ← intercepts prompts containing logs
```

**Start a new Claude Code session** in that folder to activate.

### How it works

```
You type:    "What caused the errors in /var/log/app.log?"
                              ↓  hook fires
Hook reads:  /var/log/app.log  (50,000 lines)
Hook runs:   logreduce → 312 lines, 7,800 tokens
Claude sees: "What caused the errors in [# LOG SUMMARY | 50,000 → 312 kept ...]"
```

The hook triggers automatically when your prompt contains:
- A path to a file with **≥ 500 lines**
- An inline log block of **≥ 500 lines** pasted directly

For smaller logs it passes through unchanged (zero overhead).

### Verify it's working

```bash
cd your-project/

# Simulate a prompt — should return a rewritten prompt with # LOG SUMMARY
echo '{"session_id":"test","prompt":"check /var/log/syslog"}' \
  | node .claude/hooks/UserPromptSubmit.js

# Expected output:
# {"prompt":"check \n--- LOG (reduced from ... lines) ---\n# LOG SUMMARY..."}

# If the file has < 500 lines or logreduce is not on PATH, returns:
# {}   ← pass-through, no change
```

### Usage

Just ask questions normally. No special syntax needed:

```
What caused the spike in errors at 09:00?
Summarise /var/log/nginx/error.log
Why is my service crashing? Here is the log: [paste 2000 lines]
```

---

## Integration: Cursor

Cursor supports MCP servers natively. The `reduce_log` tool appears in the agent's tool list and can be called automatically via a workspace rule.

### Setup

**Option A — run the installer:**

```bash
cd your-project/
npx logreduce-mcp --install --editor=cursor
```

This creates `.cursor/mcp.json` in your project.

**Option B — add manually.** Create or edit `.cursor/mcp.json`:

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

**Reload Cursor** after saving. The `reduce_log` tool should appear in the MCP panel.

### Make it automatic

Create a `.cursorrules` file (or add to your existing system prompt):

```
When asked to analyse a log file or log content, always call the reduce_log
tool first to compress the log to an 8000-token budget. Use the result for
your analysis. Report the reduction ratio if it is significant.
```

### Verify it's working

In a Cursor agent chat, type:
```
List your available MCP tools
```
You should see `reduce_log` in the list.

### Usage

With the workspace rule in place, just ask normally:

```
What's wrong in /var/log/app.log?
```

Cursor will call `reduce_log` first, then answer based on the compressed output.

Or call it explicitly:

```
Use reduce_log on /var/log/nginx/error.log with budget=16000
```

---

## Integration: GitHub Copilot (VS Code)

GitHub Copilot supports MCP servers in **Agent mode** (not standard chat). Once configured, Copilot calls `reduce_log` automatically before analysing log content.

### Setup

**Step 1 — open the MCP configuration file.**

Open the Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`) and run:
```
MCP: Open User Configuration
```

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

Save the file. VS Code will start the MCP server in the background.

**Step 3 — add a workspace instruction.**

Create `.github/copilot-instructions.md` in your repository:

```markdown
## Log analysis

When a user asks about a log file or shares log content (more than a
few lines), always call the `reduce_log` MCP tool first to compress
the log to an 8000-token budget. Use `result.text` for your analysis
and mention the reduction ratio from `result.stats`.
```

### Verify it's working

Switch to **Agent mode** in the Copilot Chat panel (the dropdown next to the send button). Type:

```
@workspace what tools do you have available?
```

You should see `reduce_log` listed.

### Usage

In Agent mode, ask about a log file:

```
What is causing the failures in /var/log/app.log?
```

Copilot will call `reduce_log`, then use the compressed output for its response.

> **Note:** MCP tools only work in **Agent mode**. They are not available in standard Copilot Chat.

---

## The `reduce_log` tool — manual use

You can also call the tool directly in any agent session:

```
Use reduce_log with path=/var/log/app.log
Use reduce_log with path=/var/log/app.log and budget=32000
Use reduce_log with text="[paste log content here]"
```

**All parameters:**

| Parameter  | Default | Description |
|------------|---------|-------------|
| `path`     | —       | Absolute path to a local log file |
| `text`     | —       | Raw log content as a string (alternative to `path`) |
| `budget`   | `8000`  | Maximum output tokens (100 – 200,000) |
| `context`  | `2`     | Lines of context to keep around each kept line |
| `no_redact`| `false` | Disable automatic secret/PII redaction |
| `format`   | `auto`  | Force format detection: `json` \| `logfmt` \| `syslog` \| `klog` \| `plain` |

**Response format:**

```json
{
  "text": "# LOG SUMMARY  |  50000 lines → 312 kept (0%)\n...",
  "stats": {
    "input_lines": 50000,
    "kept_lines": 312,
    "input_tokens": 640000,
    "output_tokens": 7800
  }
}
```

---

## How it works

`logreduce` uses TF-IDF over masked log templates, blended with severity weighting:

1. **Parse** — detects format (JSON, logfmt, syslog, klog, plain text)
2. **Redact** — removes secrets and PII before any scoring
3. **Mask** — replaces timestamps/UUIDs/IPs/numbers with placeholders so identical events share one template
4. **Score** — ranks templates by rarity × severity; crash-loop errors (common but high-severity) are never dropped
5. **Select** — admits lines greedily until the token budget is used; every distinct template gets at least one representative
6. **Render** — re-sorts chronologically, annotates repeats (`×500`), marks gaps (`… 48,000 lines omitted …`)

For the full technical write-up and benchmark results, see the [main repo README](../README.md).

---

## Troubleshooting

**`logreduce: command not found`**  
Run `which logreduce`. If nothing, add `~/.cargo/bin` to your PATH permanently:
```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc
```

**Claude Code: settings error on startup**  
Re-run the installer — it uses the correct hook format for your Claude Code version:
```bash
npx logreduce-mcp --install
```

**Claude Code: hook not triggering**  
The hook only fires for logs ≥ 500 lines. For smaller logs, call `reduce_log` manually.  
Check `~/.claude/settings.json` (user-level) or `.claude/settings.json` (project-level) for a `hooks.UserPromptSubmit` entry.

**Cursor: `reduce_log` not in tool list**  
Check `.cursor/mcp.json` exists and is valid JSON. Run `npx logreduce-mcp` in a terminal to see if the server starts without errors, then reload Cursor.

**Copilot: tool not available**  
Make sure you are in **Agent mode** (not standard chat). MCP tools are not available in standard Copilot Chat.
