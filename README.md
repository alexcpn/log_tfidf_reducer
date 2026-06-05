# logreduce

Reduce noisy log files to an LLM-ready summary — up to **99.9% token reduction** while keeping every error and unique event.

```
Before:  1,000,000 lines  →  12,803,209 tokens  →  ~$38/question
After:         374 lines  →       7,504 tokens  →  ~$0.02/question
```

Works standalone as a CLI, or transparently inside **Claude Code**, **Cursor**, and **GitHub Copilot** via MCP.

---

## Quickstart

### 1. Install

```bash
# MCP server + binary (no Rust required — binary auto-downloaded)
npm install -g logreduce-mcp
echo 'export PATH="$HOME/.logreduce/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc

# Or build from source
cargo install logreduce
```

### 2. Reduce a log

```bash
logreduce app.log                                      # default 8k-token budget
logreduce app.log --budget 32000 --context 3 --stats  # larger budget + stats
kubectl logs my-pod | logreduce > incident.log         # pipe from kubectl
```

### 3. Editor integration (once per machine + once per repo)

**Claude Code** — automatic interception, zero extra steps after setup:
```bash
cd your-project/
npx logreduce-mcp --install
git add .claude/ && git commit -m "chore: add logreduce-mcp"
```
Any prompt with a log path or large inline log is silently reduced before Claude reads it.

**Cursor:**
```bash
npx logreduce-mcp --install --editor=cursor
git add .cursor/ && git commit -m "chore: add logreduce-mcp"
```
Add to `.cursorrules`: *"When asked to analyse log content, first call the reduce_log tool."*

**GitHub Copilot (VS Code)** — Command Palette → **MCP: Open User Configuration**:
```json
{ "servers": { "logreduce": { "command": "npx", "args": ["logreduce-mcp"] } } }
```
Then switch to **Agent mode** in Copilot Chat.

> Full setup guide, troubleshooting, and all parameters: [`mcp/README.md`](mcp/README.md)

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
cargo test               # 13 tests
cargo bench              # throughput: MB/s & lines/s

cd mcp && npm install && npm test   # MCP server: 35 vitest tests
```

---

## Known limitations

- Token reduction numbers are from synthetic logs with known noise ratios; real production logs vary
- Rarity ≠ importance: a one-off DEBUG line with a random token scores high; severity weighting partially compensates
- Binary download requires the GitHub release to exist; fallback: `cargo install logreduce`

---

*Rust reducer: `tiktoken-rs`, `regex`, `once_cell`, `rayon`. MCP server: Node.js/TypeScript, `@modelcontextprotocol/sdk`. Blended score weights (rarity 0.4, severity 0.5, burst 0.1) configurable via `--weights`.*
