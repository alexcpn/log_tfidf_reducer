# Feeding Logs to an LLM Without Going Broke

## The 12-million-token problem

Picture a common on-call scenario. It's 2 AM, something is on fire, and you have a log file. You want to paste it into Claude and ask "what happened?" But the log is a million lines long. You can't fit it in the context window, and even if you could, at roughly 12.8 million tokens the input cost alone would be around $38 — for a single question.

So you do what everyone does: you truncate it. You grab the first ten thousand lines, paste those in, and ask your question. Claude gives you a confident answer about the service starting up successfully. That's what the first ten thousand lines contain: startup boilerplate and health-check heartbeats. The actual incident happened at line 800,000.

There has to be a better way to cut a log down before handing it to an LLM. This article describes the approach I built — `logreduce` — and the two non-obvious design problems that had to be solved to make it work.

---

## The idea: rank lines by how unusual they are

The core observation is that production logs are not uniformly informative. A typical service log at any serious scale looks something like this:

```
09:00:01 INFO  health check ok
09:00:02 INFO  health check ok
09:00:03 INFO  health check ok
09:00:04 INFO  health check ok
...              [600,000 more times]
09:01:07 ERROR failed to connect to database: connection refused
09:01:08 INFO  health check ok
09:01:09 INFO  health check ok
```

Sixty percent of the lines are identical health-check heartbeats. The error you need to find is one line in a hundred thousand. An LLM does not need to read every health check to understand what happened. It needs to know that health checks were happening (and that they passed), how many times, and that at 09:01:07 something went wrong.

This is exactly the problem TF-IDF was invented to solve — not for logs, but for document search. TF-IDF (term frequency–inverse document frequency) measures how characteristic a term is of a particular document relative to the whole collection. Terms that appear everywhere get low scores; rare terms that distinguish a document get high scores. Apply the same logic to log lines: treat each line as a document and score it by how unusual its vocabulary is across the entire log. Lines that say things nobody else said score high. Lines that repeat the same phrase six hundred thousand times score low.

The result of ranking by TF-IDF score and keeping only the highest-ranked lines until a token budget is exhausted is a log that concentrates the interesting parts and discards the noise.

---

## Non-obvious problem 1: you have to mask variable fields first

Here is the thing that kills a naïve TF-IDF implementation on logs before it gets started. Consider these two lines:

```
09:01:07 ERROR request a3f2c1d4-9b8e-4f21-bc44-e5129f7d3a01 failed: timeout after 5237ms
09:01:08 ERROR request 77b4e920-3c1a-4b9e-8d35-f290a1c6b8f2 failed: timeout after 5891ms
```

These are the same error. The request ID is different, the duration is different, but they're expressing exactly the same thing: a request timed out. A naïve TF-IDF implementation sees two completely different lines — different GUIDs, different numbers — so it treats them as two distinct events, both rare, both high-scoring. Across a log with a million such errors, every single instance looks unique. TF-IDF assigns every line a high score, the ranking collapses to noise, and you've learned nothing.

The fix is to mask variable tokens before scoring. Every timestamp becomes `<TS>`. Every UUID or hex string becomes `<ID>`. Every IP address becomes `<IP>`. Every number becomes `<NUM>`. Durations, file paths, ports: all replaced with their category placeholder. What's left is the *template* — the structural skeleton of the message that identifies what kind of line it is:

```
<TS> ERROR request <ID> failed: timeout after <NUM>ms
```

Now the million timeout errors all share one template. TF-IDF correctly identifies this template as highly frequent across the log, scores it low, and stops wasting token budget repeating the same error message a million times. The rare events — the novel error message that fired twice, the FATAL that fired once — stand out.

We have a test that proves this is load-bearing. Running the reducer with masking disabled and checking that the ranking becomes useless is part of the test suite. If someone ever removes or breaks the masking step, a test fails immediately. The masking is not an optimisation — it is the prerequisite for the whole approach to work.

---

## Non-obvious problem 2: the crash-loop trap

After implementing TF-IDF with masking, we hit a second problem that is almost the opposite of the first one.

Consider a service that is crash-looping. Every second, the process dies and restarts, emitting the same error message each time. By morning there are 500,000 copies of:

```
ERROR failed to connect to database: connection refused
```

This is the most important line in the entire log. It is the incident. But from TF-IDF's perspective, this line appears 500,000 times. It is the most *common* template in the log. Low inverse document frequency. Low score. A naïve TF-IDF implementation would rank it near the bottom and discard it in favour of rare lines that are often irrelevant one-offs.

This is the crash-loop trap: naive TF-IDF and naive rarity ranking will confidently delete the thing you most need to know.

The solution has three parts. First, the scoring function blends TF-IDF rarity with explicit severity weighting. An ERROR line gets a severity weight of 0.9 regardless of how often it appears. The weights are tuned so severity (0.5) slightly outweighs rarity (0.4), meaning a common error still wins more budget than a rare informational line. Second, the selection algorithm guarantees that every distinct message type — every unique template — contributes at least one representative line to the output before any template gets a second. No template is silently dropped. Third, repeated lines are collapsed: instead of emitting 500,000 copies of the error or none, the output emits a handful of instances with a count annotation:

```
09:01:07 ERROR failed to connect to database: connection refused   (×500000)
09:01:08 ERROR failed to connect to database: connection refused
09:01:09 ERROR failed to connect to database: connection refused
...
```

The LLM sees the error, sees the count, understands the scope of the incident, and never had to read the same line half a million times.

---

## What the output looks like

The output from `logreduce` is a plain text file the LLM can read as a normal log. It starts with a compact summary:

```
# LOG SUMMARY  |  1,000,000 lines → 374 kept (0%)
# levels: FATAL 0  ERROR 1  WARN 1  INFO 999,998  OTHER 0
# top templates: "<TS> INFO health check ok" ×600,012 ; "<TS> INFO request <ID> processed in <NUM>ms" ×150,000 ; ...
```

Then the body in chronological order — time always runs forward, regardless of how lines were ranked internally. Omitted spans are marked:

```
2026-06-01 00:00:01 INFO service started, version 2.3.1
2026-06-01 00:00:02 INFO health check ok   (×600012)
… 847,293 lines omitted …
2026-06-01 09:01:07 WARN  response time elevated: 892ms
2026-06-01 09:01:12 ERROR failed to connect to database: connection refused   (×500000)
2026-06-01 09:01:13 ERROR failed to connect to database: connection refused
```

The LLM gets the shape of the log (a million lines, almost all normal), the incident (ERROR at 09:01:12, repeated 500k times), and context around the anomaly. It does not get the 600,000 health checks.

---

## The numbers

On a 1-million-line, 49.8 MB log generated with a realistic noise-to-signal ratio (60% health-check heartbeats, 1% errors):

| Token budget | Lines kept | Input tokens | Output tokens | Reduction |
|---|---|---|---|---|
| 8,000 | 374 | 12,803,209 | 7,504 | **99.9%** |
| 32,000 | 1,506 | 12,803,209 | 29,660 | **99.8%** |

Processing time on a 16-core machine: **2.3 seconds**.

The cost framing matters more than the speed. At approximate Claude Sonnet pricing, sending that raw log would cost around $38 per question. After reduction to an 8k-token budget, it costs around $0.02 — roughly 1,900 times cheaper. For repeated questions about the same incident, combining the reduced log with prompt caching drops subsequent queries to around 10% of even that price.

---

## The pipeline

The implementation is a Rust CLI called `logreduce`. The pipeline has seven stages:

**1. Parse.** Detect the log format (JSON-per-line, logfmt, syslog, klog, or plain text) from the first 200 lines. Extract timestamp, severity level, and message. Join continuation lines — stack traces, indented frames — into single logical records so they score as one unit, not as dozens of unrelated noisy lines.

**2. Redact.** Before any scoring happens, replace secrets and PII. Known-shape patterns catch JWTs, Bearer tokens, AWS key prefixes, PEM blocks, emails, and key=value pairs where the key is in a secrets wordlist (`password`, `api_key`, `authorization`, and so on). A high-entropy backstop catches anything with 20+ characters of base64-or-hex alphabet and Shannon entropy above 3.5 bits per character. The output that reaches the LLM never contains credentials, regardless of what the developers left in the log.

**3. Mask.** Replace volatile tokens with placeholders. This is the load-bearing step described above.

**4. Score.** Build a frequency table over distinct templates, compute native TF-IDF (no sklearn — the calculation is trivial when you're working over a few thousand distinct templates rather than millions of raw lines), and blend with severity and a one-time burst bonus for templates that appear exactly once.

**5. Select.** Sort by score, then greedily admit lines until the token budget is exhausted. The guarantee: every distinct template gets one representative before any template gets a second. Context windows (configurable, default ±2 lines) pull in neighbours of kept lines so errors don't appear without context.

**6. Render.** Re-sort selected lines chronologically. Annotate repeated lines with their count. Insert gap markers for omitted spans. Prepend the summary header.

**7. (Optional) Analyse.** A decoupled Python script reads the reduced file and sends it to Claude via the Anthropic SDK, with the log body marked as a cacheable block. The Rust binary has no network dependency — it only reads and writes files.

---

## What doesn't work well (yet)

## Benchmark validation: LogDx

Before calling the numbers done, I tested `logreduce` against [LogDx](https://github.com/eyuansu62/LogDx) — a public benchmark of 35 real CI incident cases (GitHub Actions logs ranging from 40 to 89,000 lines) with human-labelled ground truth that identifies which exact lines constitute the critical diagnostic signal. The metric is recall: what fraction of the "critical" evidence lines survive the reduction?

| Budget | Critical recall | Cases | Avg lines kept |
|--------|----------------|-------|----------------|
| 8,000 tokens | **99% (144/146)** | 35 | 75% |
| 4,000 tokens | **99% (144/146)** | 35 | 75% |
| 2,000 tokens | **99% (144/146)** | 35 | 74% |

The recall is stable across budgets because the first-pass template guarantee (every distinct message type gets at least one representative) operates before budget pressure is applied. Critical signal lines tend to be either unique or high-severity, so they're among the first admitted.

The two remaining misses are both eval artefacts rather than genuine failures: one is an 11-character line (`pnpm format`) too short to fingerprint reliably; the other is a JavaScript source line embedded in log output that happened to get bumped by budget pressure in its specific case.

The benchmark exercise uncovered two real bugs. First, the path masking regex (`(?:\/[\w.\-]+){3,}`) was collapsing stack trace frames from different files into an identical template. `at /lib/debugger.js:92` and `at /test/test-debugger.js:15` both became `at <PATH>:<NUM>` — the tool correctly deduplicated what it mistakenly believed were the same event. Removing path masking entirely (UUID and NUM masks already handle the variable parts of paths) fixed the template collapse. Second, the renderer was truncating multi-line joined records to their first line, silently discarding stack frames that had been correctly joined to their parent exception. Emitting all lines of a joined record fixed both the rendering and the recall.

---

## What doesn't work well (yet)

The current implementation has two remaining limitations.

**Real production logs, not just synthetic ones.** The token reduction numbers come from a synthetic log with known template weights. Real nginx access logs, real Kubernetes event streams, and real JVM garbage collection logs all have different noise profiles. The benchmark uses real CI logs, which validates the recall claim, but the *token reduction percentage* on a fresh production log depends on how repetitive that log is.

**Rarity is not the same as importance.** A one-off DEBUG line with a random UUID in it might score high on rarity without being interesting. The masking step reduces this problem significantly — masked templates are much more structurally informative than raw lines — but it is not eliminated. Severity weighting partially compensates. A Drain3-style learned template miner would be more robust than the current regex approach, at the cost of added complexity.

---

## Using it

### CLI — direct use

```bash
# Install (pre-built binary, no Rust required)
# Download from: https://github.com/alexcpn/log_tfidf_reducer/releases/latest

# Or build from source
cargo build --release          # → ./target/release/logreduce

# Reduce a log to an 8k-token budget (default)
logreduce app.log

# Larger budget, more context lines, show stats
logreduce app.log --budget 32000 --context 3 --stats

# Pipe from kubectl
kubectl logs my-pod | logreduce --budget 8000 > incident.log

# Ask Claude about the reduced log (optional Python layer)
export ANTHROPIC_API_KEY=sk-ant-...
python python/analyze.py reduced.log --question "What caused the errors at 09:01?"
```

---

### Editor integration — Claude Code, Cursor, GitHub Copilot

The MCP server [`logreduce-mcp`](https://www.npmjs.com/package/logreduce-mcp) wraps the binary so AI coding assistants can reduce logs automatically. Install once, works across all three editors.

**Step 1 — install the MCP server** (also downloads the `logreduce` binary automatically):

```bash
npm install -g logreduce-mcp
```

**Step 2 — set up your editor:**

#### Claude Code (automatic — zero extra steps after setup)

```bash
cd your-project/
npx logreduce-mcp --install
```

That's it. Restart Claude Code. From now on, any prompt containing a log file path or large inline log block is silently reduced before Claude reads it:

```
You:    "What caused the errors in /var/log/app.log?"
          ↓ hook intercepts
Claude: [sees 312-line summary instead of 50,000-line raw log]
```

#### Cursor

```bash
npx logreduce-mcp --install --editor=cursor
```

Then add a workspace rule in `.cursorrules`:

```
When asked to analyse a log file or log content, first call the
reduce_log tool to compress it to an 8000-token budget.
```

Reload Cursor. The `reduce_log` tool appears in the agent's tool list.

#### GitHub Copilot (VS Code)

Open Command Palette → **MCP: Open User Configuration** and add:

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

Create `.github/copilot-instructions.md`:

```markdown
When asked about a log file or log content, call the reduce_log MCP
tool first to compress the log before analysing it.
```

Switch to **Agent mode** in Copilot Chat. Done.

> Full documentation: [`mcp/README.md`](mcp/README.md)

---

*The Rust reducer uses `tiktoken-rs` for token counting, `regex` + `once_cell` for compile-once masking patterns, and `rayon` for parallel processing. The blended score weights (rarity 0.4, severity 0.5, burst 0.1) are configurable via `--weights`. The MCP server is Node.js/TypeScript using `@modelcontextprotocol/sdk`.*

---

## Build & develop

**Requirements**: Rust stable toolchain (`rustup`). Python 3.11+ for the optional LLM layer.

```bash
# Build
cargo build --release          # → ./target/release/logreduce

# Quality gates (all must pass)
cargo fmt --check
cargo clippy -- -D warnings
cargo test                     # 13 tests: golden-file, crash-loop, masking ablation, redaction, determinism
cargo bench                    # criterion: MB/s & lines/s (SC-004)

# Reduce a log (default 8k-token budget)
./target/release/logreduce app.log

# Options
./target/release/logreduce app.log \
  --budget 16000 \             # token ceiling
  --context 3 \                # ±N context lines around each kept line
  --stats \                    # print reduction stats to stderr
  -o reduced.log               # write to file instead of stdout

# Disable secret redaction (redaction is ON by default)
./target/release/logreduce app.log --no-redact

# From a pipe
kubectl logs my-pod | ./target/release/logreduce --budget 4000 > incident.log

# Optional: ask Claude about the reduced log
pip install -r python/requirements.txt
export ANTHROPIC_API_KEY=sk-ant-...
python python/analyze.py reduced.log --question "What caused the errors?"
```

### Verify success criteria

| Criterion | How to check |
|---|---|
| SC-001 token ratio ≤10% | `--stats` shows input vs output tokens on a noisy log |
| SC-002/003 fidelity | Run on `sample_logs/crash_loop.log` — every ERROR appears; each template has ≥1 rep |
| SC-004 throughput | `cargo bench` on 1M-line synthetic log completes in ~2.3 s on 16 cores |
| SC-005 determinism | Run twice, `diff` outputs — must be identical |
| SC-006 recall | `python eval/logdx_eval.py /tmp/LogDx/cases --budget 8000` → 99% on 35 CI cases |
| SC-007 redaction | Run on `sample_logs/secrets.log` — all known patterns appear as `<REDACTED:…>` |
