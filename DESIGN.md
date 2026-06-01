# Log Reduction via TF-IDF for LLM Token Savings — Design

## 1. Goal

Take a raw log file (often 10k–10M+ lines) and produce a **much smaller,
LLM-ready** version that preserves the *informative* lines (errors, rare
events, state transitions) while suppressing high-frequency boilerplate
(heartbeats, repeated INFO, health checks).

Target: feed the **reduced** log to an LLM instead of the raw log, cutting
input tokens by an order of magnitude with minimal loss of the lines a human
debugger would actually care about.

**This design covers the reduction pipeline only.** The LLM call itself is out
of scope — the output is a file/string ready to drop into any prompt.

### Success metrics
- **Token reduction ratio**: `tokens(reduced) / tokens(raw)`. Goal ≤ 0.1 on
  noisy production logs.
- **Signal recall**: of the lines a human marks as "relevant to the incident,"
  what fraction survive reduction. Goal ≥ 0.95 for ERROR/WARN/FATAL.
- **Chronology preserved**: output is human-readable in time order, not score
  order.

---

## 2. Why TF-IDF (and where it bites)

**The core insight is correct:** in real logs most lines are near-duplicate
boilerplate. TF-IDF down-weights terms that appear in many "documents" and
up-weights rare terms, so ranking lines by their TF-IDF mass naturally floats
the unusual lines to the top.

**But rarity ≠ importance — this is the central risk.** Two failure modes:

1. **The crash-loop problem.** A service that crash-loops emits the *same*
   ERROR 50,000 times. That line is now extremely *common* → low TF-IDF → it
   gets suppressed. Yet it's the whole story of the incident.
2. **The rare-noise problem.** A one-off DEBUG line with a random GUID is rare
   → high TF-IDF → it gets kept even though it's irrelevant.

The design must therefore **not use TF-IDF alone.** It blends three signals
(§5) and uses frequency-collapse (§4) so that even a high-frequency error keeps
at least one representative line plus a count.

---

## 3. Pipeline overview

```
raw log
  │
  ▼
[1] Parse & normalize        → timestamp, level, message extracted
  │
  ▼
[2] Mask variable fields     → "template" per line (the make-or-break step)
  │
  ▼
[3] Template grouping        → frequency table; collapse near-duplicates
  │
  ▼
[4] Score each line          → blend(TF-IDF rarity, severity, frequency)
  │
  ▼
[5] Select to token budget   → top lines + context windows, never drop a
  │                            unique template entirely
  ▼
[6] Re-order chronologically → emit reduced log + header summary
  │
  ▼
LLM-ready output
```

---

## 4. Stage details

### [1] Parse & normalize
Extract `(timestamp, level, message)` from each line. Support a few common
formats (syslog, JSON lines, logfmt, k8s/klog) via pluggable regex; fall back
to "whole line = message, no timestamp" so it never hard-fails.

### [2] Mask variable fields — **the make-or-break step**
Raw lines contain per-line-unique tokens: timestamps, UUIDs, IPs, hex
addresses, request IDs, numbers. If we run TF-IDF on raw text, *every* line
looks "rare" because of its unique GUID, and the ranking collapses to noise.

So before scoring, replace volatile tokens with placeholders:
- timestamps → `<TS>`
- UUIDs / hex / hashes → `<ID>`
- IPv4/IPv6 → `<IP>`
- numbers → `<NUM>`
- file paths, ports, durations → `<PATH>`, `<PORT>`, `<DUR>`

Two implementation options:
- **Regex masking** (simple, fast, good enough to start).
- **Drain3 log templating** (online template miner — more robust, learns
  templates automatically). Recommended if regex masking proves brittle.

The masked string is the line's **template**. TF-IDF runs on templates, not raw
text.

### [3] Template grouping & frequency-collapse
Group identical templates. Build a frequency table `template → count`. This
gives us:
- The denominator for IDF.
- **Frequency-collapse**: instead of emitting 50,000 identical lines, emit one
  representative (the first, or the most recent) annotated `(×50000)`. This
  alone is often the bulk of the token savings and it *cannot* drop a recurring
  error — it always keeps one copy.

### [4] Scoring
Run sklearn `TfidfVectorizer(norm=None)` over the **distinct templates**
(`norm=None` so rare-term lines accumulate higher raw scores rather than being
L2-normalized to ~equal). A line's rarity score:

```
rarity(line) = Σ_t  tf(t, template) · idf(t)      # idf = log(N/df_t)+1
```

Optionally divide by template token length to avoid biasing toward verbose
lines (configurable — verbose lines are often stack traces we *want*).

### [5] Final blended score
```
score(line) =  w_r · norm(rarity)          # TF-IDF: unusual phrasing
             + w_s · severity(level)        # FATAL=1.0 ERROR=0.9 WARN=0.6 …
             + w_f · burst(template)        # one-time bump for rare templates,
                                            #   but recurring ERRORs kept via §3
```
Default weights `w_s ≥ w_r` so severity can rescue the crash-loop case.
Weights are tunable per deployment.

### [6] Selection under a token budget
1. Sort lines by `score` descending.
2. Greedily admit lines until the **token budget** is hit (count tokens with
   `tiktoken`/Anthropic counter; fall back to `chars/4`).
3. For each admitted high-score line, also pull a **±k line context window**
   (errors are rarely interpretable alone — the lines around them matter).
4. Guarantee: **every distinct template contributes ≥ 1 representative line**
   before any template contributes a second, so nothing unique is ever fully
   dropped.

### [7] Output assembly
- Re-sort selected lines **chronologically** (LLM reads a timeline, not a
  ranked list).
- Insert gap markers: `… 412 lines omitted (mostly <TS> INFO health-check) …`.
- Prepend a compact **header summary**: total lines, time span, top-N template
  frequency table, counts by level. This gives the LLM the "shape" of the log
  cheaply.

Example output skeleton:
```
# LOG SUMMARY  |  1,204,331 lines → 1,840 kept  |  09:00–17:30
# levels: INFO 1.19M  WARN 8.2k  ERROR 412  FATAL 3
# top templates: "<TS> INFO health ok" ×980k ; "<TS> INFO req <ID> 200" ×190k …

09:02:11 INFO  service started, version <NUM>
… 980,000 lines omitted (health-check heartbeats) …
14:31:07 WARN  connection pool at <NUM>% capacity
14:31:09 ERROR failed to acquire connection: timeout after <DUR>   (×412)
14:31:09 ERROR   at db.pool.acquire(<PATH>)
…
```

---

## 5. Failure modes & mitigations (summary)

| Risk | Mitigation |
|------|-----------|
| Crash-loop error suppressed (common = low TF-IDF) | severity weight + frequency-collapse keeps a representative |
| Rare GUID noise kept | masking removes the GUID before scoring |
| Every line looks rare | masking (§2) — without it the whole approach fails |
| Error line kept but context lost | ±k context windows (§6.3) |
| Output unreadable (score order) | chronological re-sort (§6) |
| Token estimate wrong | real tokenizer, not chars/4, when available |

---

## 6. Proposed module layout (for when we build)

```
log_tfidf/
  parse.py       # [1] format detection + (ts, level, msg) extraction
  mask.py        # [2] regex / Drain3 templating
  score.py       # [3][4][5] frequency table + TF-IDF + blended score
  select.py      # [6] token-budget selection + context windows
  render.py      # [7] chronological assembly + header summary
  reduce.py      # CLI: reduce.py app.log --budget 8000 --out reduced.log
  eval/          # §7 evaluation harness + fixtures
```
No LLM dependency in the core path; `reduce.py` emits a file/string.

---

## 7. Evaluation plan

1. **Token ratio** on a handful of real logs (k8s, app server, syslog).
2. **Recall harness**: take a log with a known incident, hand-label the
   "relevant" lines, measure how many survive at budgets 2k/8k/32k.
3. **A/B usefulness** (optional, later): ask the same LLM question against full
   vs reduced log; compare answers. This is the real test of "did we keep what
   matters."
4. **Ablations**: masking on/off, TF-IDF-only vs blended, with/without
   frequency-collapse — to show each piece earns its place.

---

## 8. Open questions

- **Streaming vs batch?** Drain3 supports online templating, so a streaming
  reducer is feasible later. Start with batch (whole file).
- **TF-IDF granularity**: line-as-document (proposed) vs sliding-window-as-
  document. Line-level is simpler and matches the "keep/drop a line" decision.
- **Multi-line entries** (stack traces, JSON pretty-print): need a join step in
  [1] so a stack trace is one logical record, not N noisy lines.
- **Default weights** `w_r/w_s/w_f`: pick by tuning on the eval set.

---

## 9. Relationship to the existing `aiops_logreduction` repo

The original repo does TF-IDF + **Prophet** time-series thresholding for
*anomaly detection* (flagging outliers over time). This design reuses the
TF-IDF idea but reframes it for **token reduction**: the deliverable is a
smaller log, not an anomaly chart. The Prophet temporal step is complementary
and could later feed an extra signal into the blended score (§5) — "this
template fired at an unexpected time" → boost.
```
```
