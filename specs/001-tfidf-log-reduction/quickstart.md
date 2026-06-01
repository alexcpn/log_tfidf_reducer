# Quickstart: TF-IDF Log Reduction

## Prerequisites

- Rust (stable) toolchain — `rustup` / `cargo`.
- (Optional, for LLM analysis) Python 3.11+ and an `ANTHROPIC_API_KEY`.

## Build

```bash
cargo build --release      # produces ./target/release/logreduce
```

## Reduce a log

```bash
# basic: reduce to the default 8000-token budget, output to stdout
./target/release/logreduce app.log

# write to a file, larger budget, more context, show stats
./target/release/logreduce app.log -o reduced.log -b 16000 -c 3 --stats

# from a pipe
kubectl logs my-pod | logreduce --budget 4000 > reduced.log
```

## Analyze the reduced log with Claude (optional)

```bash
pip install -r python/requirements.txt
export ANTHROPIC_API_KEY=sk-...
python python/analyze.py reduced.log --question "What caused the errors?"
```

## Develop & verify

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test                 # golden-file tests + masking ablation test
cargo bench                # criterion: MB/s & lines/s (Principle I)
```

## Verify success criteria

- **Token ratio (SC-001)**: `--stats` prints input vs output tokens; expect
  ≤ 10% on a noisy log.
- **Fidelity (SC-002/003)**: on `sample_logs/`, confirm every ERROR/FATAL and at
  least one representative of every template appear in the output.
- **Throughput (SC-004)**: `cargo bench` on a generated ~1 GB synthetic log
  completes reduction in < 60 s on a typical laptop.
- **Determinism (SC-005)**: run twice; `diff` the outputs — must be identical.
- **Redaction (SC-007)**: run on a fixture seeded with fake secrets/emails;
  confirm they appear only as `<REDACTED:…>`.
- **Masking is load-bearing (Principle III ablation)**: the test suite includes a
  run with masking disabled showing the ranking collapses.
