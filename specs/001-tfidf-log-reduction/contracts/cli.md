# Contract: `logreduce` CLI + Output Format

The reducer's external interface (Principle V: text in/out, offline). This is the
stable contract downstream tasks and tests target.

## Command

```
logreduce [OPTIONS] [INPUT]
```

| Arg / Option | Default | Meaning |
|--------------|---------|---------|
| `INPUT` | stdin | path to the log file; `-` or omitted = stdin (FR-001) |
| `-o, --out <PATH>` | stdout | write reduced output here (FR-001) |
| `-b, --budget <N>` | `8000` | token budget ceiling (R1, FR-006) |
| `--max-lines <N>` | none | alternative cap; tighter of the two wins (R1) |
| `-c, --context <N>` | `2` | ± context lines around each kept line; `0` disables (FR-015) |
| `--no-redact` | off | disable secret/PII redaction (default ON) (FR-014) |
| `--format <FMT>` | auto | force `json\|logfmt\|syslog\|klog\|plain` (R5) |
| `--weights <r,s,f>` | `0.4,0.5,0.1` | blended-score weights (R7) |
| `--stats` | off | print reduction stats to stderr (FR-011) |

### I/O & exit codes (Principle V)

- Reduced log → **stdout** (or `--out`). Diagnostics/stats → **stderr**.
- Exit `0` success (including empty-input → header-only/empty output, edge case).
- Exit `1` usage error (bad args). Exit `2` I/O error (unreadable input/output).
- **No network access** under any flag (FR-012).

### Determinism (FR-010)

Same input + same options ⇒ byte-identical stdout.

## Output format contract

```
# LOG SUMMARY  | <total> lines → <kept> kept (<ratio>)  | <start>–<end>
# levels: INFO <n>  WARN <n>  ERROR <n>  FATAL <n>
# top templates: "<masked>" ×<n> ; "<masked>" ×<n> ; …
#
<ts> <LEVEL> <message>                         # chronological body (FR-008)
… <n> lines omitted (<short reason>) …          # gap marker (FR-008)
<ts> <LEVEL> <message>   (×<count>)             # collapsed repeat (FR-007)
```

Rules:
- Body is **chronological**; falls back to input order when timestamps absent.
- Collapsed templates show one representative with `(×count)` (FR-007).
- Omitted spans replaced by a single gap marker stating count + nature (FR-008).
- Secrets/PII appear only as `<REDACTED:kind>` (FR-014).
- Header always present (even for empty input → counts of 0).

## Optional LLM layer (decoupled — User Story 3)

```
python python/analyze.py <reduced-file> [--model <id>] [--question "<q>"]
```

- Reads an already-produced reduced file; sends it to Claude via the `anthropic`
  SDK with the log body in a `cache_control` block (prompt caching).
- Requires `ANTHROPIC_API_KEY`. Never invoked by `logreduce`; reduction works
  with this script absent (FR-012, Principle V).
- Exit `0` on a returned analysis; non-zero on auth/network failure — and such
  failure has **no effect** on the reducer.
