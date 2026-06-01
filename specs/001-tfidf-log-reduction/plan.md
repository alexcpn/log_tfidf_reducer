# Implementation Plan: TF-IDF Log Reduction for LLM Token Savings

**Branch**: `001-tfidf-log-reduction` | **Date**: 2026-06-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-tfidf-log-reduction/spec.md`

## Summary

Build a high-throughput log-reduction tool: a Rust CLI (`logreduce`) that parses
a log file, masks volatile data and redacts secrets/PII into per-line
*templates*, ranks lines by a blend of TF-IDF rarity + severity + frequency,
selects lines under a token budget (with context windows and a guaranteed
representative per template), and renders a chronological, summarized,
LLM-ready file. A thin, decoupled Python script (`python/analyze.py`) optionally
sends that file to Claude via the official `anthropic` SDK with prompt caching.
The reducer performs no network I/O and is deterministic.

## Technical Context

**Language/Version**: Rust (stable, edition 2021) for the reducer; Python 3.11+
for the optional LLM layer.

**Primary Dependencies**: Rust — `clap` (CLI), `rayon` (data parallelism),
`regex` + `std::sync::LazyLock` (compile-once mask/redact patterns), `memmap2`
(fast large-file reads), `rustc-hash` (`FxHashMap`), `tiktoken-rs` (token
budgeting). Python — `anthropic` (LLM call only).

**Storage**: Files only — input log (read), reduced output (written / stdout).
No database.

**Testing**: `cargo test` (golden-file + unit), `criterion` (`benches/`) for
throughput; optional `pytest` for the Python wrapper.

**Target Platform**: Linux/macOS command line (single static-ish binary).

**Project Type**: CLI tool (single Rust crate) + a thin Python helper script.

**Performance Goals**: Reduce a 1 GB / multi-million-line log in < 60 s on a
typical developer laptop (SC-004); throughput reported in MB/s and lines/s;
scales with available cores.

**Constraints**: Reduction is fully offline (no network — FR-012); output is
deterministic for identical input+config (FR-010); memory bounded relative to
*distinct-template* count, not total line count; secrets/PII redacted by default
(FR-014).

**Scale/Scope**: 10k–10M+ lines, multi-GB files; hundreds–thousands of distinct
templates typical.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Performance Is a Contract | `benches/` with criterion reporting MB/s & lines/s; >10% regression blocks merge | PASS — bench is a Phase-1 artifact + verification step |
| II. Fidelity Over Compression (NON-NEGOTIABLE) | ≥1 representative per distinct template; all ERROR/FATAL preserved; repeats summarized with counts | PASS — guaranteed in `select` (FR-004/005/007) |
| III. Mask Before You Score (NON-NEGOTIABLE) | TF-IDF runs only on masked templates; ablation test proves masking is load-bearing | PASS — `mask` precedes `score`; ablation test planned |
| IV. Test-First with Golden Outputs | golden-file test per format; deterministic output | PASS — `tests/` + fixtures planned (FR-010) |
| V. CLI Contract & Decoupling | offline single-purpose reducer; LLM only in Python layer | PASS — no network in crate; `python/analyze.py` separate |

**Initial gate: PASS.** No violations → Complexity Tracking is empty.

**Post-Design re-check (after Phase 1): PASS.** The data model, CLI contract, and
module boundaries preserve all five gates; redaction (FR-014) folds into the
`mask` stage without adding network dependencies.

## Project Structure

### Documentation (this feature)

```text
specs/001-tfidf-log-reduction/
├── plan.md              # This file
├── spec.md              # Feature spec (+ Clarifications)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── cli.md           # CLI command + output-format contract
└── checklists/
    └── requirements.md  # spec quality checklist
```

### Source Code (repository root)

```text
Cargo.toml               # crate manifest
src/
├── main.rs              # clap CLI entry → lib::reduce
├── lib.rs               # pipeline wiring + ReduceConfig
├── parse.rs             # [1] format detect + (ts, level, msg); multi-line join
├── mask.rs              # [2] volatile-token masking + secret/PII redaction
├── score.rs             # [3][4][5] freq table + native TF-IDF + blended score
├── select.rs            # [6] token-budget selection + context windows
└── render.rs            # [7] chronological assembly + header + gap markers
benches/
└── reduce_bench.rs      # criterion: MB/s & lines/s on synthetic log
tests/
└── pipeline.rs          # golden-file tests + masking ablation test
sample_logs/             # fixtures: klog, json-lines, syslog, plain
python/
├── analyze.py           # optional: send reduced file to Claude (anthropic SDK)
└── requirements.txt     # anthropic
```

**Structure Decision**: Single Rust crate (CLI tool) at repo root — the simplest
layout that satisfies Principle V (one offline binary). The Python LLM helper is
an isolated `python/` directory that only consumes the reducer's output file;
it shares no code with the crate, keeping the deterministic core fully decoupled
from the network-bound LLM call.

## Complexity Tracking

> No Constitution Check violations — section intentionally empty.
