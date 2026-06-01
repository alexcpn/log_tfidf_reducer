# Phase 1 Data Model: TF-IDF Log Reduction

Internal domain types for the Rust reducer. These are conceptual; field names
map closely to the eventual structs in `src/`.

## Entity: ReduceConfig

The resolved configuration for one reduction run (from CLI args + defaults).

| Field | Type | Notes |
|-------|------|-------|
| `input` | path / stdin | source log (FR-001) |
| `output` | path / stdout | reduced output (FR-001) |
| `budget_tokens` | u32 | default 8000 (R1) |
| `max_lines` | Option<u32> | optional alternative cap (R1) |
| `context_window` | u8 | ±lines around kept lines; 0 disables (FR-015) |
| `weights` | (w_r, w_s, w_f) | blended-score weights, default (0.4,0.5,0.1) (R7) |
| `redact` | bool | default true (FR-014) |
| `format` | Option<Format> | override; else auto-detect (R5) |

**Validation**: `budget_tokens ≥ 1`; weights ≥ 0; `context_window ≤` a sane cap.

## Entity: LogRecord

One logical log entry (may span multiple physical lines after join).

| Field | Type | Notes |
|-------|------|-------|
| `index` | usize | original order (stable tie-break, FR-010) |
| `timestamp` | Option<i64> | epoch; None when absent (R5) |
| `level` | Severity | explicit or inferred (FR-005a / R6) |
| `raw` | str slice | original text (for output) |
| `template_id` | TemplateId | link to its Template (post-mask) |

**State**: raw → parsed → masked+redacted (gets `template_id`) → scored →
selected/omitted. No persistence; lives for the run only.

## Entity: Severity (enum)

`Fatal | Error | Warn | Info | Other` with weights 1.0 / 0.9 / 0.6 / 0.2 / 0.1
(R6). Used by the blended score and the header summary.

## Entity: Template (message type)

A group of records identical after redaction + masking (FR-003, FR-004).

| Field | Type | Notes |
|-------|------|-------|
| `id` | TemplateId | stable id |
| `masked` | String | the template string (e.g. `<TS> INFO req <ID> <NUM>`) |
| `count` | u64 | total occurrences (drives collapse counts, FR-007) |
| `df_terms` | term set | contributes to IDF (document frequency) |
| `representative` | usize | record index kept when collapsed |
| `rarity` | f32 | Σ tf·idf, un-normalized (R7) |

**Identity/uniqueness**: `masked` string is the natural key. **Invariant
(Principle II)**: every Template contributes ≥ 1 representative to the output
before any contributes a second line (FR-004).

## Entity: ScoredLine

| Field | Type | Notes |
|-------|------|-------|
| `record_index` | usize | → LogRecord |
| `score` | f32 | `w_r·norm(rarity)+w_s·severity+w_f·burst` (R7) |
| `kept` | bool | selection result |
| `as_context` | bool | kept only because adjacent to a kept line (FR-015) |

## Entity: ReducedOutput

The rendered artifact (FR-008/009/011).

| Part | Content |
|------|---------|
| `header` | total lines, time span, counts by severity, top-N template freq table, token ratio (FR-009/011) |
| `body` | selected records in chronological order, with collapse counts (`×N`) and gap markers for omitted spans (FR-008) |
| `stats` | input vs output token/line counts (FR-011) |

## Relationships

```
ReduceConfig ──drives──▶ pipeline
LogRecord N───1 Template          (many records share one template)
LogRecord 1───1 ScoredLine
Template / ScoredLine ──feed──▶ ReducedOutput (header + body + stats)
```

## Determinism rules (FR-010)

- Stable sort keys everywhere: score desc, then `timestamp`, then original
  `index`. Template iteration ordered by `id` (insertion order).
- No hashing-order-dependent output (FxHashMap used only for counting, never for
  output ordering).
