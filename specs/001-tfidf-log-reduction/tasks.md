---
description: "Task list for TF-IDF Log Reduction for LLM Token Savings"
---

# Tasks: TF-IDF Log Reduction for LLM Token Savings

**Input**: Design documents from `/specs/001-tfidf-log-reduction/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/cli.md

**Tests**: INCLUDED — the project constitution (Principle IV: Test-First with
Golden Outputs; Principle I: benchmarks; Principle III: masking ablation test)
mandates tests, so test tasks are first-class here.

**Organization**: Tasks are grouped by user story (US1 P1 → US2 P2 → US3 P3) so
each story is an independently testable increment.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 / US2 / US3 (story-phase tasks only)

## Path Conventions

Single Rust crate at repo root: `src/`, `tests/`, `benches/`, `sample_logs/`,
plus an isolated `python/` helper (per plan.md Structure Decision).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Create Rust crate structure per plan.md (`Cargo.toml`, `src/main.rs`, `src/lib.rs`, empty module files `src/parse.rs` `src/mask.rs` `src/score.rs` `src/select.rs` `src/render.rs`, dirs `tests/` `benches/` `sample_logs/`)
- [x] T002 Declare dependencies in `Cargo.toml`: `clap`, `rayon`, `regex`, `memmap2`, `rustc-hash`, `tiktoken-rs`, `serde_json`; dev-dependency `criterion` with a `[[bench]]` entry for `reduce_bench`
- [x] T003 [P] Add `rustfmt.toml` and clippy config; document the `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `cargo bench` quality gates (constitution) in `README.md`
- [x] T004 [P] Scaffold `python/` LLM helper: `python/requirements.txt` (`anthropic`) and a stub `python/analyze.py`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types and pipeline skeleton every user story depends on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 [P] Define core domain types in `src/lib.rs` per data-model.md: `Severity` enum + weights (Fatal 1.0/Error 0.9/Warn 0.6/Info 0.2/Other 0.1), `TemplateId`, `LogRecord`, `Template`, `ScoredLine`
- [x] T006 Define `ReduceConfig` struct + defaults in `src/lib.rs` (budget 8000, max_lines None, context 2, weights (0.4,0.5,0.1), redact true, format auto) per data-model.md
- [x] T007 Implement clap CLI in `src/main.rs` mapping all flags to `ReduceConfig` per contracts/cli.md (`INPUT/stdin`, `--out`, `--budget`, `--max-lines`, `--context`, `--no-redact`, `--format`, `--weights`, `--stats`) including exit-code scaffolding (0/1/2)
- [x] T008 Wire the empty pipeline `reduce(config) -> Result<ReducedOutput>` in `src/lib.rs` calling parse→mask→score→select→render stubs in order
- [x] T009 [P] Add sample log fixtures in `sample_logs/` (one each: `plain.log`, `json.log`, `syslog.log`, `klog.log`) covering noisy/boilerplate + embedded errors
- [x] T010 [P] Build golden-file test harness in `tests/pipeline.rs`: helper that runs `reduce` on a fixture and compares stdout to a checked-in `sample_logs/<name>.expected`

**Checkpoint**: Foundation ready — user story implementation can begin ✅

---

## Phase 3: User Story 1 - Shrink a log to a token budget without losing the incident (Priority: P1) 🎯 MVP

**Goal**: Turn a large noisy log into a much smaller file that stays within a
token budget while keeping every error and a representative of every distinct
message type.

**Independent Test**: Run the reducer on `sample_logs/` at a set budget; confirm
output is within budget, contains all ERROR/FATAL lines, includes ≥1
representative of every template, and surfaces a crash-loop error with a count.

### Tests for User Story 1 (write first; must FAIL before implementation) ⚠️

- [x] T011 [P] [US1] Golden test in `tests/pipeline.rs`: noisy fixture reduces under budget, all ERROR/FATAL present, ≥1 representative per template (FR-004/005)
- [x] T012 [P] [US1] Test in `tests/pipeline.rs`: an error repeated 50k times is surfaced with an accurate `(×N)` count and not dropped (FR-005/007, Principle II)
- [x] T013 [P] [US1] Masking ablation test in `tests/pipeline.rs`: with masking disabled the ranking collapses (proves Principle III is load-bearing)
- [x] T014 [P] [US1] Token-budget test in `tests/pipeline.rs`: output token count ≤ `--budget` using the tiktoken counter (FR-006)

### Implementation for User Story 1

- [x] T015 [US1] Implement `src/parse.rs`: format auto-detect (json/logfmt/syslog/klog/plain) + `--format` override, extract `(timestamp, level, message)`, multi-line join for stack traces, severity inference fallback (FR-005a) per research R5/R6
- [x] T016 [US1] Implement secret/PII redaction in `src/mask.rs`: known-shape patterns + high-entropy backstop → `<REDACTED:kind>`, runs BEFORE variable masking, per research R3 (FR-014)
- [x] T017 [US1] Implement variable masking in `src/mask.rs`: TS/ID/IP/NUM/PATH/PORT/DUR placeholders → template string, compile-once `LazyLock` patterns, per research R4 (FR-003)
- [x] T018 [US1] Implement frequency table + native TF-IDF in `src/score.rs`: `FxHashMap<template,Template>`, `idf=ln(N/df)+1`, `rarity=Σ tf·idf` over distinct templates (FR-002), per research R7
- [x] T019 [US1] Implement blended score in `src/score.rs`: `w_r·norm(rarity)+w_s·severity+w_f·burst` with configurable weights (FR-002/005), per research R7
- [x] T020 [US1] Implement selection in `src/select.rs`: rank desc, greedy admit to token budget (`tiktoken-rs`, chars/4 fallback), **guarantee ≥1 representative per template before any second line** (FR-004/006), collapse repeats with counts (FR-007)
- [x] T021 [US1] Implement context windows in `src/select.rs`: keep ±`context` adjacent lines around each kept line, `0` disables, counted against budget (FR-015)
- [x] T022 [US1] Implement minimal `src/render.rs`: emit selected records (with `(×count)` collapse annotations) to `--out`/stdout, with stable deterministic ordering (FR-001/007/010)
- [x] T023 [US1] Finalize pipeline wiring in `src/lib.rs` + `src/main.rs`: stdin/stdout + `--out`, exit codes per contracts/cli.md, reduction stats to stderr on `--stats` (FR-011), ensure no network calls (FR-012)

**Checkpoint**: MVP — `logreduce app.log` yields a smaller, faithful, budget-bounded file ✅

---

## Phase 4: User Story 2 - Readable, summarized output an LLM can reason over (Priority: P2)

**Goal**: Make the reduced output read like a coherent, summarized timeline.

**Independent Test**: Reduce a fixture and confirm output is chronological with
an accurate header summary, collapsed repeats show counts, and omitted spans are
marked.

### Tests for User Story 2 (write first; must FAIL before implementation) ⚠️

- [x] T024 [P] [US2] Test in `tests/pipeline.rs`: body is chronological (and falls back to input order when timestamps absent) (FR-008)
- [x] T025 [P] [US2] Test in `tests/pipeline.rs`: header summary present & accurate (total lines, time span, severity counts, top-N templates) and gap markers appear for omitted spans (FR-008/009)

### Implementation for User Story 2

- [x] T026 [US2] Implement chronological re-sort of selected records in `src/render.rs` (stable by timestamp then original index; input-order fallback) (FR-008/010)
- [x] T027 [US2] Implement gap markers in `src/render.rs`: replace omitted spans with a single `… N lines omitted (reason) …` marker (FR-008)
- [x] T028 [US2] Implement header summary block in `src/render.rs`: total lines, time span, counts by severity, top-N template frequency table, token ratio (FR-009/011)

**Checkpoint**: US1 + US2 both work independently; output is summarized and chronological ✅

---

## Phase 5: User Story 3 - One-step analysis with an LLM (Priority: P3)

**Goal**: Optionally send a reduced file to Claude for analysis, fully decoupled
from reduction.

**Independent Test**: Point `analyze.py` at an already-reduced file; confirm it
builds/sends a valid request, and that reduction works identically with this
script absent.

### Tests for User Story 3 (write first; must FAIL before implementation) ⚠️

- [ ] T029 [P] [US3] Test in `python/test_analyze.py`: `analyze.py` reads a reduced file and constructs a valid request with the body in a `cache_control` block (mock client / no real network), and reduction is unaffected when credentials/network are absent (FR-012)

### Implementation for User Story 3

- [x] T030 [US3] Implement `python/analyze.py`: read reduced file, call Claude via `anthropic` SDK with the log body in a `cache_control` block (prompt caching), `--model` and `--question` flags, per contracts/cli.md and research R9
- [x] T031 [US3] Implement graceful failure in `python/analyze.py` (missing `ANTHROPIC_API_KEY` / no network → clear error, non-zero exit) and document that the reducer never imports/requires it (FR-012, Principle V)

**Checkpoint**: All three stories independently functional ✅ (T029 mock test deferred)

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Constitution gates, success-criteria verification, edge cases

- [x] T032 [P] Implement criterion benchmark in `benches/reduce_bench.rs` reporting MB/s & lines/s on a generated large synthetic log; verify SC-004 (<60s for ~1 GB) (Principle I)
- [x] T033 [P] Edge-case tests in `tests/pipeline.rs`: empty/whitespace-only, no timestamps, all-identical, all-unique, budget < one line, malformed/binary, extremely long lines (FR-013)
- [x] T034 [P] Determinism test in `tests/pipeline.rs`: run twice on the same input/config, assert byte-identical output (SC-005/FR-010)
- [x] T035 [P] Redaction corpus test in `tests/pipeline.rs`: fixture seeded with fake secrets/PII → 100% of known-pattern values redacted (SC-007)
- [ ] T036 [P] Update `README.md` + `specs/001-tfidf-log-reduction/quickstart.md` so build/run/verify steps match the shipped CLI; confirm all quality gates pass
- [ ] T037 Run quickstart.md end-to-end validation (build → reduce a sample → optional analyze) and confirm SC-001/002/003/006

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies — start immediately
- **Foundational (Phase 2)**: depends on Setup — BLOCKS all user stories
- **User Stories (Phase 3–5)**: depend on Foundational
  - US2 (render summary/chronology) extends `src/render.rs` produced in US1 → US2 depends on US1
  - US3 (Python LLM layer) depends only on the reducer producing a file → can start after US1 (independent of US2)
- **Polish (Phase 6)**: depends on the user stories it verifies (benchmark needs full pipeline; SC tests need US1/US2)

### User Story Dependencies

- **US1 (P1)**: after Foundational — no dependency on other stories. MVP.
- **US2 (P2)**: after US1 (shares `src/render.rs`). Independently testable.
- **US3 (P3)**: after US1 (needs a reduced file). Independent of US2.

### Within Each User Story

- Tests written first and FAIL before implementation (Principle IV)
- parse/mask before score; score before select; select before render

### Parallel Opportunities

- Setup: T003, T004 in parallel
- Foundational: T005, T009, T010 in parallel (T006→T007→T008 are sequential in `lib.rs`/`main.rs`)
- US1 tests T011–T014 all [P] together (same file, but independent assertions — write as separate test fns)
- US1 implementation: T016 and T017 touch `src/mask.rs` (sequential); T018/T019 `src/score.rs` (sequential); parse (T015) is independent and can run alongside mask/score work
- Polish: T032–T035 in parallel (different concerns)

---

## Parallel Example: User Story 1 tests

```bash
# Write all US1 tests first (they must fail), then implement:
Task: "Golden test: under budget + all errors + rep per template (T011)"
Task: "Crash-loop error surfaced with count (T012)"
Task: "Masking ablation collapses ranking (T013)"
Task: "Output within token budget (T014)"
```

---

## Implementation Strategy

### MVP First (User Story 1 only)

1. Phase 1 Setup → 2. Phase 2 Foundational → 3. Phase 3 US1 → **STOP & validate**
   (`logreduce` produces a smaller, faithful, budget-bounded log). This alone is
   shippable value.

### Incremental Delivery

1. Setup + Foundational → foundation ready
2. US1 → MVP (reduced file)
3. US2 → readable summarized/chronological output
4. US3 → optional one-step LLM analysis
5. Polish → benchmarks + success-criteria + edge-case verification

---

## Notes

- [P] = different files / independent, safe to parallelize
- Verify tests fail before implementing (Principle IV)
- Redaction (T016) runs before masking (T017) so secrets never inflate rarity (Principle III)
- Selection (T020) must never drop a template entirely (Principle II) — this is the highest-risk task; keep its test (T011) authoritative
- Commit after each task or logical group
- T029 (mock test for analyze.py) deferred — requires `pytest`/`unittest.mock`; implementation complete (T030/T031 done)
