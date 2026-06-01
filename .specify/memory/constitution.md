<!--
SYNC IMPACT REPORT
==================
Version change: (template, unversioned) → 1.0.0
Rationale: Initial ratification — first concrete constitution derived from the
placeholder template. MINOR/PATCH not applicable; this is the 1.0.0 adoption.

Modified principles: none (initial definition). Placeholder principles replaced:
  [PRINCIPLE_1_NAME] → I. Performance Is a Contract
  [PRINCIPLE_2_NAME] → II. Fidelity Over Compression (NON-NEGOTIABLE)
  [PRINCIPLE_3_NAME] → III. Mask Before You Score (NON-NEGOTIABLE)
  [PRINCIPLE_4_NAME] → IV. Test-First with Golden Outputs
  [PRINCIPLE_5_NAME] → V. CLI Contract & Decoupling

Added sections:
  - Performance & Resource Standards (Section 2)
  - Development Workflow & Quality Gates (Section 3)

Removed sections: none.

Templates requiring updates:
  ✅ .specify/templates/plan-template.md — Constitution Check gate is generic
     ("[Gates determined based on constitution file]"); no contradiction. The
     Performance Goals / Constraints fields already accommodate Principle I.
  ✅ .specify/templates/spec-template.md — no mandatory section added/removed;
     no change required.
  ✅ .specify/templates/tasks-template.md — already carries test/perf task
     categories compatible with Principles I & IV; no change required.
  ✅ .specify/templates/checklist-template.md — generic; no change required.

Follow-up TODOs: none. RATIFICATION_DATE set to initial adoption date.
-->

# Log TF-IDF Reducer Constitution

This project builds a high-throughput log-reduction tool: a Rust CLI that ranks
log lines by TF-IDF over masked templates, blends in severity and frequency, and
emits a compact, LLM-ready file. A thin, decoupled Python layer performs the
optional LLM call. These principles are non-negotiable engineering rules; they
exist to protect the two things that make the tool worth using — **it is fast**
and **it never silently throws away the line you needed.**

## Core Principles

### I. Performance Is a Contract

The hot path (read → parse → mask → count) MUST be implemented in Rust and MUST
remain measurably fast. Every release MUST be benchmarked with `criterion`
(`benches/`) reporting throughput in MB/s and lines/s on a representative
synthetic log. Throughput MUST scale with available cores (parallelism via
`rayon`). A change that regresses benchmarked throughput by more than 10% MUST
be justified in the PR or rejected. "Fast enough" is not a feeling — it is a
number tracked over time.

**Rationale**: The entire premise is reducing 10k–10M+ line logs cheaply; if the
reducer is slow it is cheaper to send raw logs, and the tool has no reason to
exist.

### II. Fidelity Over Compression (NON-NEGOTIABLE)

Reduction MUST NOT silently discard signal. The reducer MUST guarantee at least
one representative line for every distinct template before any template
contributes a second line, and MUST preserve all `ERROR`/`FATAL` records subject
to the blended score. Suppressed repetition MUST be summarized with a count
(e.g. `(×412)`), never dropped without trace. Omitted spans MUST be marked.

**Rationale**: TF-IDF rewards rarity, but a crash-looping error is *common* and
would be ranked low — naive TF-IDF deletes the incident. Severity weighting plus
frequency-collapse exist specifically to prevent this; weakening them defeats
the tool.

### III. Mask Before You Score (NON-NEGOTIABLE)

TF-IDF and all ranking MUST operate on **masked templates**, never on raw lines.
Volatile tokens (timestamps, UUIDs/hex, IPs, numbers, paths, ports, durations)
MUST be normalized to placeholders before scoring. Masking rules MUST be
deterministic and covered by tests, including an ablation test proving that
disabling masking collapses the ranking.

**Rationale**: Unmasked, every line carries a unique GUID/timestamp and looks
"rare," so the ranking turns to noise. Masking is load-bearing, not an
optimization.

### IV. Test-First with Golden Outputs

Behavior MUST be specified by tests written alongside or before implementation.
Each supported log format MUST have a golden-file test under `tests/` with
fixtures in `sample_logs/`. Output MUST be deterministic for a given input and
configuration (stable ordering, stable tie-breaks). Bug fixes MUST add a
regression test reproducing the bug first.

**Rationale**: A reducer that changes its output unpredictably cannot be trusted
in a pipeline, and silent fidelity regressions (Principle II) are only caught by
golden tests.

### V. CLI Contract & Decoupling

The reducer MUST be a single-purpose CLI: input via args/stdin, reduced log to
stdout (or `--out`), diagnostics to stderr, and meaningful exit codes. The
reducer MUST NOT make network calls or depend on any LLM. The LLM step lives
only in the Python layer, which consumes the reducer's output file and is
independently replaceable.

**Rationale**: Composability and testability require the deterministic, offline
core to be cleanly separable from the non-deterministic, network-bound LLM call.

## Performance & Resource Standards

- **Language/stack**: Rust for the reducer (`clap`, `rayon`, `regex` compiled
  once, `memmap2`, fast hashing); Python only for the optional LLM call via the
  official `anthropic` SDK with prompt caching.
- **Memory**: memory use MUST stay bounded relative to distinct-template count,
  not total line count; large files are streamed/`mmap`-ed, never fully
  materialized as owned `String`s where avoidable.
- **Token budgeting**: selection MUST respect a configurable token budget using
  `tiktoken-rs`. The OpenAI/Claude tokenizer mismatch is accepted as a
  conservative approximation and MUST be documented, not hidden.
- **Benchmarks gate**: `cargo bench` results MUST be reviewable; unexplained
  >10% regressions block merge (Principle I).

## Development Workflow & Quality Gates

- **Mandatory gates before merge**: `cargo fmt --check`, `cargo clippy -D
  warnings`, `cargo test`, and `cargo bench` (for hot-path changes) MUST pass.
- **Spec-driven flow**: features follow the spec-kit workflow
  (`/speckit-specify` → `/speckit-plan` → `/speckit-tasks` →
  `/speckit-implement`); the plan's Constitution Check gate MUST be satisfied.
- **Review compliance**: reviewers MUST verify Principles II and III explicitly
  for any change touching scoring, masking, or selection. Added complexity MUST
  be justified against a simpler rejected alternative.

## Governance

This constitution supersedes ad-hoc practice for this project. Amendments MUST be
proposed via PR that states the change, its rationale, and a migration note if
behavior changes; they take effect on merge. Versioning of this document follows
semantic versioning: **MAJOR** for removing/redefining a principle or governance
rule, **MINOR** for adding a principle or materially expanding guidance,
**PATCH** for clarifications. All PRs and reviews MUST verify compliance with the
principles above; deviations MUST be recorded in the plan's Complexity Tracking
section with justification or the change MUST be revised.

**Version**: 1.0.0 | **Ratified**: 2026-06-01 | **Last Amended**: 2026-06-01
