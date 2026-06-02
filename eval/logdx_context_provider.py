#!/usr/bin/env python3
"""
LogDx context provider: run logreduce on each case and write the context
files + JSONL manifest that run_diagnosis.py expects.

Usage (from the log_tfidf repo root):
    python eval/logdx_context_provider.py \
        --logdx /tmp/LogDx \
        --binary ./target/release/logreduce \
        --budget 8000 \
        --split dev

Then run diagnosis + eval from the LogDx repo:
    cd /tmp/LogDx
    export DIAGNOSIS_COMMAND="python3 examples/diagnosis_shim_claude_cli.py"
    export CILOGBENCH_ALLOW_EXTERNAL_LLM=1
    python tools/run_diagnosis.py --split dev \
        --diagnoser real-debugger-v1 \
        --context-method logreduce-8k \
        --command "$DIAGNOSIS_COMMAND" \
        --diagnoser-name logreduce-reducer-v1
    python tools/evaluate_diagnosis.py --split dev \
        --diagnoser logreduce-reducer-v1
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path


def iter_cases(cases_root: Path, split: str):
    """Yield (split_label, case_id, raw_log_path) for all cases in a split."""
    split_dir = cases_root / split
    if not split_dir.exists():
        return
    for entry in sorted(split_dir.iterdir()):
        if entry.name == 'split_manifest.json' or not entry.is_dir():
            continue
        raw = entry / 'raw.log'
        if raw.exists():
            yield split, entry.name, raw
        else:
            # v2 layout: sub-splits inside the split dir
            for sub in sorted(entry.iterdir()):
                r = sub / 'raw.log'
                if sub.is_dir() and r.exists():
                    yield f'{split}/{entry.name}', sub.name, r


def run_logreduce(binary: str, raw_log: Path, budget: int) -> tuple[str, int]:
    """Run logreduce and return (output_text, exit_code)."""
    result = subprocess.run(
        [binary, str(raw_log), '--budget', str(budget)],
        capture_output=True, text=True, timeout=120,
    )
    return result.stdout, result.returncode


def count_lines(text: str) -> int:
    return len(text.splitlines())


def main():
    ap = argparse.ArgumentParser(description='Generate LogDx context files using logreduce')
    ap.add_argument('--logdx', required=True, help='Path to LogDx repo root')
    ap.add_argument('--binary', default='./target/release/logreduce')
    ap.add_argument('--budget', type=int, default=8000)
    ap.add_argument('--split', default=None, help='Only process one split')
    args = ap.parse_args()

    logdx = Path(args.logdx)
    cases_root = logdx / 'cases'
    method_name = f'logreduce-{args.budget // 1000}k'

    splits = [args.split] if args.split else ['dev', 'holdout', 'stress', 'v2']
    total = skipped = 0

    for split in splits:
        manifest_rows = []
        results_dir = logdx / 'results' / split
        context_dir = results_dir / method_name
        context_dir.mkdir(parents=True, exist_ok=True)

        for split_label, case_id, raw_log in iter_cases(cases_root, split):
            total += 1
            print(f'  {split_label}/{case_id} ...', end=' ', flush=True)

            raw_lines = len(raw_log.read_text(errors='replace').splitlines())
            raw_bytes = raw_log.stat().st_size

            context_text, rc = run_logreduce(args.binary, raw_log, args.budget)
            if rc != 0:
                print(f'FAIL (exit {rc})')
                skipped += 1
                continue

            out_path = context_dir / f'{case_id}.txt'
            out_path.write_text(context_text, encoding='utf-8')

            out_lines = count_lines(context_text)
            out_bytes = len(context_text.encode('utf-8'))
            reduction = round(1.0 - out_lines / max(raw_lines, 1), 6)

            # LogDx schema requires included_line_ranges. logreduce does not
            # currently expose per-line original indices, so we leave this
            # empty. The diagnosis evaluator does not use this field;
            # evaluate_signal_recall.py does but is separate from the
            # leaderboard score.
            manifest_rows.append({
                'case_id': case_id,
                'method': method_name,
                'mode': 'context_provider',
                'raw_log_path': str(raw_log.relative_to(logdx)),
                'context_path': str(out_path.relative_to(logdx)),
                'input_line_count': raw_lines,
                'output_line_count': out_lines,
                'input_byte_size': raw_bytes,
                'output_byte_size': out_bytes,
                'reduction_ratio': reduction,
                'included_line_ranges': [],   # not tracked by logreduce yet
                'metadata': {
                    'budget_tokens': args.budget,
                    'binary': args.binary,
                },
            })
            print(f'{raw_lines} → {out_lines} lines ({reduction:.1%} reduction)')

        # Write JSONL manifest
        manifest_path = results_dir / f'{method_name}.jsonl'
        with manifest_path.open('w', encoding='utf-8') as f:
            for row in manifest_rows:
                f.write(json.dumps(row) + '\n')
        print(f'\n  Wrote {manifest_path.relative_to(logdx)}  ({len(manifest_rows)} cases)\n')

    print(f'Done. {total} cases processed, {skipped} skipped.')
    print(f'\nNext steps (from the LogDx repo):')
    print(f'  cd {args.logdx}')
    print(f'  export DIAGNOSIS_COMMAND="python3 examples/diagnosis_shim_claude_cli.py"')
    print(f'  export CILOGBENCH_ALLOW_EXTERNAL_LLM=1')
    print(f'  python tools/run_diagnosis.py --split dev \\')
    print(f'      --diagnoser real-debugger-v1 \\')
    print(f'      --context-method {method_name} \\')
    print(f'      --command "$DIAGNOSIS_COMMAND" \\')
    print(f'      --diagnoser-name logreduce-reducer-v1')
    print(f'  python tools/evaluate_diagnosis.py --split dev \\')
    print(f'      --diagnoser logreduce-reducer-v1')


if __name__ == '__main__':
    main()
