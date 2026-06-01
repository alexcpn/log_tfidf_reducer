# Phase 0 Research: TF-IDF Log Reduction

Resolves the open/deferred decisions from the spec clarifications and plan
Technical Context. No outstanding NEEDS CLARIFICATION remain after this phase.

## R1. Default token budget & units

- **Decision**: Token budget is the primary, user-supplied unit (`--budget N`),
  defaulting to **8000 tokens** when unspecified. Also accept `--max-lines N` as
  an alternative cap; whichever is more restrictive wins.
- **Rationale**: 8k comfortably fits a reduced incident log into any modern model
  context while leaving room for the question/answer; line cap is a convenience
  for users who think in lines.
- **Alternatives considered**: target reduction *ratio* (rejected as primary —
  unpredictable absolute size); fixed model context (rejected — couples the
  offline reducer to a specific model, violating Principle V).

## R2. Token counting vs. the Claude tokenizer mismatch

- **Decision**: Count tokens with `tiktoken-rs` (`cl100k_base`) as a fast,
  deterministic approximation; treat the budget as a *conservative* ceiling.
  Print a one-line disclosure that counts are approximate for Claude.
- **Rationale**: No public exact Claude tokenizer in Rust; `cl100k_base`
  correlates closely enough for budgeting and keeps the reducer offline and
  deterministic (Principle V). Over-counting slightly is safe (stays under
  real budget).
- **Alternatives considered**: chars/4 heuristic (kept as a no-dependency
  fallback if `tiktoken-rs` is unavailable); calling an API token-count endpoint
  (rejected — would put network in the reducer, violating FR-012/Principle V).

## R3. Secret/PII redaction (FR-014, SC-007)

- **Decision**: Redact in the `mask` pass in two layers:
  1. **Known-shape patterns** → replace with `<REDACTED:kind>`: emails, JWTs
     (`eyJ[\w-]+\.[\w-]+\.[\w-]+`), AWS-style keys (`AKIA[0-9A-Z]{16}`),
     `Bearer <token>`, PEM blocks (`-----BEGIN ... PRIVATE KEY-----`), and
     key/value pairs whose key ∈ {password, passwd, secret, token, api_key,
     apikey, authorization, access_key}.
  2. **High-entropy backstop**: any remaining token of length ≥ 20 over a
     base64/hex alphabet with Shannon entropy ≥ 3.5 bits/char → `<REDACTED:hi>`.
- **Rationale**: Layer 1 catches the common, recognizable cases precisely; layer
  2 catches unknown secret formats. Both are deterministic and disclosed as
  best-effort (residual risk per FR-014).
- **Alternatives considered**: ML/NER-based PII detection (rejected — heavy,
  non-deterministic, network/model dependency); no redaction (rejected by Q1).
- **Ordering note**: redaction happens *before* variable-masking so a redacted
  secret never inflates template rarity (ties to Principle III).

## R4. Variable masking → templates (FR-003)

- **Decision**: Regex masking with a compile-once pattern set (`LazyLock`):
  timestamps→`<TS>`, UUID/hex/hash→`<ID>`, IPv4/IPv6→`<IP>`, numbers→`<NUM>`,
  paths→`<PATH>`, ports→`<PORT>`, durations→`<DUR>`. The masked string is the
  line's *template*.
- **Rationale**: Fast, deterministic, dependency-light, and sufficient to make
  TF-IDF meaningful (Principle III). 
- **Alternatives considered / later upgrade**: Drain3-style online template
  mining — more robust to unseen formats but adds complexity and state; deferred
  to a follow-up (recorded in spec/plan open items).

## R5. Format detection & multi-line join (FR parse)

- **Decision**: Sniff the first ~200 non-empty lines to pick a format among
  {JSON-per-line, logfmt/key-value, syslog, klog/glog, plain}. Extract
  `(timestamp, level, message)`; unparseable lines fall back to
  `message = whole line`. Join continuation lines (indented, or lacking a new
  timestamp, e.g. stack frames) into one logical record.
- **Rationale**: Robustness over precision — never hard-fail (FR-013); multi-line
  join prevents stack traces from fragmenting into noise (spec edge case).
- **Alternatives considered**: user-declared format flag only (kept as an
  override `--format`, but auto-detect is the default for zero-config use).

## R6. Severity determination (FR-005a)

- **Decision**: Use an explicit level field when the parser found one; otherwise
  infer from case-insensitive keyword/pattern match in the message
  (`fatal|panic`, `error|exception|traceback|stacktrace`, `warn`). Map to a
  severity weight (FATAL 1.0, ERROR 0.9, WARN 0.6, INFO 0.2, other 0.1).
- **Rationale**: Keeps the FR-005 fidelity guarantee meaningful on logs without a
  structured level (the common case). Per Q3 clarification.

## R7. Native TF-IDF & blended scoring (FR-002)

- **Decision**: Build over **distinct templates** (not raw lines). For term `t`:
  `idf_t = ln(N_templates / df_t) + 1`; line rarity (un-normalized so rare-term
  lines score high) `rarity = Σ_t tf(t)·idf_t`. Blended score:
  `score = w_r·norm(rarity) + w_s·severity + w_f·burst`, defaults
  `w_r=0.4, w_s=0.5, w_f=0.1` (so severity can rescue recurring errors,
  Principle II). Weights overridable via flags.
- **Rationale**: Cheap (few thousand templates), deterministic, and directly
  implements the constitution's anti-crash-loop blend. No sklearn needed.
- **Alternatives considered**: sklearn `TfidfVectorizer` in Python (rejected —
  would force the hot path through Python/FFI; native is faster and keeps the
  binary self-contained).

## R8. Parallelism & memory (Performance Goals, Principle I)

- **Decision**: `memmap2` the input; split into line ranges; process ranges in
  parallel with `rayon`, each worker masking+redacting and counting into a
  thread-local `FxHashMap<template, TemplateStat>`, then merge. Selection/render
  are single-pass over template stats + a bounded set of kept records.
- **Rationale**: The hot path (read→mask→count) is embarrassingly parallel and
  is the bottleneck; memory stays bounded by distinct-template count, not total
  lines (plan Constraints).
- **Alternatives considered**: line-buffered single-threaded read (simpler but
  fails SC-004 throughput on multi-GB inputs).

## R9. LLM layer (User Story 3, decoupled)

- **Decision**: `python/analyze.py` reads the reduced file and calls Claude via
  the official `anthropic` SDK, placing the log body in a `cache_control` block
  for prompt caching; default model a cost-effective Sonnet/Haiku, overridable.
  Entirely separate process; reduction never imports or requires it.
- **Rationale**: Satisfies Principle V (decoupling) and FR-012 (offline core).
  Prompt caching cuts repeat-analysis cost.
- **Alternatives considered**: in-Rust LLM call via `reqwest` (viable, discussed,
  but user chose Rust-reduce + Python-LLM split).
