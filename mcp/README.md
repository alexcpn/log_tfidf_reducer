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

## Installation

```bash
npm install -g logreduce-mcp
```

That's all. The postinstall script automatically downloads the `logreduce` binary for your platform (Linux x64, macOS ARM/Intel, Windows x64) — **no Rust required**.

You will see output like:
```
[logreduce-mcp] Downloading logreduce for linux-x64...
[logreduce-mcp] Saved to: ~/.logreduce/bin/logreduce
[logreduce-mcp] Add to PATH: export PATH="~/.logreduce/bin:$PATH"
```

Add the shown line to your `~/.bashrc` / `~/.zshrc` (Linux/macOS) or System Environment Variables (Windows), then open a new terminal.

Verify: `logreduce --version`

### Manual binary install (alternative)

If the auto-download fails, download from the [releases page](https://github.com/alexcpn/log_tfidf_reducer/releases/latest):

| Platform | File |
|---|---|
| Linux x64 | `logreduce-linux-x64` |
| macOS Apple Silicon | `logreduce-darwin-arm64` |
| macOS Intel | `logreduce-darwin-x64` |
| Windows x64 | `logreduce-win32-x64.exe` |

Or build from source: `cargo install logreduce` (requires [Rust](https://rustup.rs))

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

> **No Node/npm available?** Skip MCP entirely — see [Cursor without Node/npm (rules-based)](#cursor-without-nodenpm-rules-based) below. It works identically on Linux and Windows and needs only the standalone `logreduce` binary.

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

### Cursor without Node/npm (rules-based)

If `npx` is unavailable or broken (common on locked-down corporate Windows —
see [Troubleshooting](#troubleshooting)), don't use MCP at all. Cursor's agent
can run shell commands directly, so a project rule that tells it to shell out
to the standalone `logreduce` binary works just as well — and needs no Node,
npm, or MCP server. This works identically on Linux and Windows.

1. Get the `logreduce` binary onto PATH — no Rust/npm required (see the
   [main README](../README.md#1-install) for the direct-download links for
   Linux/macOS/Windows).
2. Add the rule file:
   ```bash
   mkdir -p .cursor/rules
   curl -L https://raw.githubusercontent.com/alexcpn/log_tfidf_reducer/main/mcp/config/cursor-rules.mdc \
     -o .cursor/rules/logreduce.mdc
   ```
   (Windows PowerShell: `Invoke-WebRequest -Uri "https://raw.githubusercontent.com/alexcpn/log_tfidf_reducer/main/mcp/config/cursor-rules.mdc" -OutFile ".cursor\rules\logreduce.mdc"`)
3. Reload Cursor. The agent now runs `logreduce <file>` itself in the
   terminal before reading large logs — see [`config/cursor-rules.mdc`](config/cursor-rules.mdc)
   for the exact instructions it follows.

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
The postinstall script downloads the binary to `~/.logreduce/bin/` but it may not be on your PATH yet. Add it:
```bash
echo 'export PATH="$HOME/.logreduce/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc
```
Or re-run the install to trigger the download again: `npm install -g logreduce-mcp`

**Claude Code: settings error on startup**  
Re-run the installer — it uses the correct hook format for your Claude Code version:
```bash
npx logreduce-mcp --install
```

**Claude Code: hook not triggering**  
The hook only fires for logs ≥ 500 lines. For smaller logs, call `reduce_log` manually.  
Check `~/.claude/settings.json` (user-level) or `.claude/settings.json` (project-level) for a `hooks.UserPromptSubmit` entry.

**Windows: `npx logreduce-mcp ...` just opens the `.js` file in a text editor**  
Some corporate Windows machines reassign the `.js` file association away from
Node/Windows Script Host to a text editor (a common security hardening step),
so `npx` ends up "opening" the resolved script instead of running it with
`node`. Workarounds:
- Install globally and run the generated shim directly (it explicitly wraps
  the call with `node.exe`, bypassing the file association):
  ```powershell
  npm install -g logreduce-mcp
  logreduce-mcp --install --editor=cursor
  ```
- If the same problem prevents the editor from *launching* the MCP server at
  runtime (config uses `"command": "npx"`), skip MCP altogether and use the
  [rules-based Cursor integration](#cursor-without-nodenpm-rules-based) instead
  — it only needs the standalone `logreduce` binary, no Node/npm at all.

**Cursor: `reduce_log` not in tool list**  
Check `.cursor/mcp.json` exists and is valid JSON. Run `npx logreduce-mcp` in a terminal to see if the server starts without errors, then reload Cursor.

**Copilot: tool not available**  
Make sure you are in **Agent mode** (not standard chat). MCP tools are not available in standard Copilot Chat.
