# logreduce

Reduce noisy log files to an LLM-ready summary — up to **99.9% token reduction** while keeping every error and unique event.

```
Before:  1,000,000 lines  →  12,803,209 tokens  →  ~$38/question
After:         374 lines  →       7,504 tokens  →  ~$0.02/question
```

Works standalone as a CLI, or transparently inside **Claude Code**, **Cursor**, and **GitHub Copilot** — one static binary, no Node/npm required anywhere.

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
Invoke-WebRequest -Uri "https://github.com/alexcpn/log_tfidf_reducer/releases/latest/download/logreduce-win32-x64.exe" `
  -OutFile "C:\tools\logreduce.exe"
C:\tools\logreduce.exe --version   # add C:\tools to PATH
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
Run one command per editor from your project root:

```bash
cd your-project/

logreduce install --editor=claude-code   # writes .claude/settings.json (hook entry)
logreduce install --editor=cursor        # writes .cursor/rules/logreduce.mdc
logreduce install --editor=copilot       # writes .github/copilot-instructions.md

git add .claude/ .cursor/ .github/ && git commit -m "chore: add logreduce editor integration"
```

**Claude Code** — automatic interception, zero extra steps after setup: the
installed hook (`logreduce hook`, the same binary in hook mode) intercepts
every prompt and silently reduces any log path or large inline log block
before Claude reads it — deterministic, not dependent on the model
remembering to do it.

**Cursor** — installs a [project rule](.cursor/rules) that tells the agent to
run `logreduce <path>` itself in the terminal before reading large logs. Reload
Cursor after installing.

**GitHub Copilot (VS Code)** — installs `.github/copilot-instructions.md` with
the same instruction. Requires **Agent mode** in Copilot Chat (custom
instructions and terminal commands aren't available in standard chat).

All three approaches rely on the agent shelling out to `logreduce` directly —
see [`templates/`](templates/) for the exact rule/instruction text each
installer writes.

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
