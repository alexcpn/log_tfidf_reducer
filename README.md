# logreduce

Reduce noisy log files to an LLM-ready summary — up to **99.9% token reduction** while keeping every error and unique event.

```
Before:  1,000,000 lines  →  12,803,209 tokens  →  ~$38/question
After:         374 lines  →       7,504 tokens  →  ~$0.02/question
```

Works standalone as a CLI, or transparently inside **Claude Code**, **Codex**, **Cursor**, and **GitHub Copilot** — one static binary, no Node/npm required anywhere.

This project grew out of [aiops_logreduction](https://github.com/alexcpn/aiops_logreduction), an earlier experiment in suppressing periodically repeating logs and surfacing the rare ones with TF-IDF ranking and time-series fitting.

---

## Quickstart

### 1. Install

`logreduce` is a single static binary —  Rust based code for performance.

**Quick install (any platform)** — download the binary for your OS from
[Releases](https://github.com/alexcpn/log_tfidf_reducer/releases) and put it on PATH:

```bash
# Linux / macOS — adjust filename for your platform
curl -L https://github.com/alexcpn/log_tfidf_reducer/releases/latest/download/logreduce-linux-x64 \
  -o /usr/local/bin/logreduce
chmod +x /usr/local/bin/logreduce
```

```powershell
# Windows (PowerShell) — works even on locked-down corporate machines with no npm
# Uses your user profile's bin folder — no admin rights needed
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\bin" | Out-Null
Invoke-WebRequest -Uri "https://github.com/alexcpn/log_tfidf_reducer/releases/latest/download/logreduce-win32-x64.exe" `
  -OutFile "$env:USERPROFILE\bin\logreduce.exe"
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
[Environment]::SetEnvironmentVariable("Path", "$userPath;$env:USERPROFILE\bin", "User")
# Open a new terminal, then:
logreduce --version
```

**Or build from source** (requires Rust): `cargo install logreduce`

### 2. Reduce a log

```bash
logreduce app.log                                      # default 8k-token budget
logreduce app.log --budget 32000 --context 3 --stats  # larger budget + stats
kubectl logs my-pod | logreduce > incident.log         # pipe from kubectl
```

### 3. Editor integration (once per machine + once per repo)

The binary installs its own integration files — no MCP server, no Node, no npm.
Run from your project root:

```bash
cd your-project/

logreduce install --editor=all           # every editor below
# or one at a time:
logreduce install --editor=claude-code   # .claude/settings.json (hook) + .claude/skills/logreduce/
logreduce install --editor=codex         # .codex/hooks.json (hook) + .agents/skills/logreduce/ + AGENTS.md
logreduce install --editor=copilot       # .github/hooks/logreduce.json (hook) + .agents/skills/logreduce/ + AGENTS.md
logreduce install --editor=cursor        # .agents/skills/logreduce/ + AGENTS.md

git add .claude/ .codex/ .agents/ .github/ AGENTS.md && git commit -m "chore: add logreduce editor integration"
```

Every editor gets the same portable [`SKILL.md`](templates/SKILL.md)
([Agent Skills](https://agentskills.io) format): it tells the agent to run
`logreduce <path>` instead of reading a large log directly, covering logs the
agent *discovers itself* mid-session. Claude Code reads it from
`.claude/skills/`; Codex, Cursor and VS Code Copilot read `.agents/skills/`.
`AGENTS.md` gets a one-line pointer to the skill.

Where the editor supports it, a **hook** (`logreduce hook`, the same binary in
hook mode) runs on every prompt: if the prompt names a log file of 500+ lines,
the reduced log is injected as context before the model starts — deterministic,
not dependent on the model remembering the skill. Hooks can add context but not
rewrite the prompt, so logs *pasted inline* are left as-is — save them to a file
and pass the path instead.

| Editor | Skill | Prompt hook | Notes |
|---|---|---|---|
| Claude Code | ✓ | ✓ | Start a new session after installing |
| Codex | ✓ | ✓ | Run `/hooks` once to trust the hook |
| GitHub Copilot (VS Code) | ✓ | ✓ | Agent mode only |
| Cursor | ✓ | — | Cursor's `beforeSubmitPrompt` can only allow/block, not add context |

### 4. Spec Kit bug workflow (optional)

[`speckit/`](speckit/) is a [Spec Kit](https://github.com/github/spec-kit)
extension that adds a log-intake step to the bundled `bug` triage workflow:
`/speckit.logreduce.intake` reduces the logs attached to a bug report into
`.specify/bugs/<slug>/evidence.md` for `/speckit.bug.assess` to work from.

---

## How it works

`logreduce` ranks log lines by TF-IDF over masked templates, blended with severity weighting:

1. **Parse** — detects format (JSON, logfmt, syslog, klog, plain); joins stack trace continuations
2. **Redact** — removes secrets and PII before scoring (JWTs, API keys, emails, high-entropy tokens)
3. **Mask** — replaces timestamps/UUIDs/IPs/numbers with placeholders so identical events share one template
4. **Score** — TF-IDF rarity × severity weight × burst bonus; common ERRORs survive via severity weighting
5. **Select** — every distinct template gets ≥1 representative; then fills remaining budget by score
6. **Render** — chronological output; collapsed repeats annotated `(×500)`; gap markers for omitted spans

**The crash-loop trap**: naive TF-IDF drops repeated ERRORs (they're common → low score). `logreduce` rescues them via severity weighting and the per-template guarantee.

**The masking requirement**: without masking, every UUID-bearing line looks unique and ranking collapses to noise. Masking is load-bearing, not an optimisation.

---

## Benchmark results

Tested against [LogDx](https://github.com/eyuansu62/LogDx) — 35 real CI incident cases with human-labelled ground truth:

| Budget | Critical signal recall | Cases |
|---|---|---|
| 8,000 tokens | **99% (144/146)** | 35 |
| 2,000 tokens | **99% (144/146)** | 35 |

On a 1M-line synthetic log (16-core machine):

| Budget | Lines kept | Token reduction | Wall time |
|---|---|---|---|
| 8,000 | 374 | **99.9%** | 2.3 s |
| 32,000 | 1,506 | **99.8%** | 2.3 s |

---

## Build & develop

```bash
cargo build --release    # Rust binary → target/release/logreduce
cargo fmt --check
cargo clippy -- -D warnings
cargo test               # pipeline + hook + install tests
cargo bench              # throughput: MB/s & lines/s
```

---

## Known limitations

- Token reduction numbers are from synthetic logs with known noise ratios; real production logs vary
- Rarity ≠ importance: a one-off DEBUG line with a random token scores high; severity weighting partially compensates
- Binary download requires the GitHub release to exist; fallback: `cargo install logreduce`

---

*Single Rust binary: `tiktoken-rs`, `regex`, `once_cell`, `rayon`, `clap`, `serde`. Editor integration (`logreduce hook` / `logreduce install`) ships in the same binary — no separate runtime. Blended score weights (rarity 0.4, severity 0.5, burst 0.1) configurable via `--weights`.*
