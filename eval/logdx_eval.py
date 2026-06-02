#!/usr/bin/env python3
"""
Evaluate logreduce recall against the LogDx benchmark.

For each case:
  1. Run `logreduce` on raw.log at the specified budget
  2. For each "critical" required_signal, check if at least one of its
     evidence_lines appears in the reduced output
  3. Report per-case and aggregate recall

Usage:
  python eval/logdx_eval.py /tmp/LogDx/cases --budget 8000
  python eval/logdx_eval.py /tmp/LogDx/cases --split dev --budget 8000
"""
import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path


ANSI_RE = re.compile(r'\x1b\[[0-9;]*m|\x1b\[K')
GHA_TS_RE = re.compile(r'^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z\s*')


def strip_noise(line: str) -> str:
    """Strip GHA timestamp and ANSI escape codes."""
    s = ANSI_RE.sub('', line)
    s = GHA_TS_RE.sub('', s)
    return s.strip()


def fingerprint(line: str, min_len: int = 12) -> str | None:
    """Return a content fingerprint for matching, or None if line is too short."""
    s = strip_noise(line)
    # Remove leading/trailing whitespace and collapse runs
    s = ' '.join(s.split())
    return s if len(s) >= min_len else None


def run_logreduce(binary: str, log_path: str, budget: int) -> str:
    """Run logreduce and return the reduced output as a string."""
    result = subprocess.run(
        [binary, log_path, '--budget', str(budget), '--no-redact'],
        capture_output=True,
        text=True,
        timeout=120,
    )
    return result.stdout


def check_signal_recall(raw_lines: list[str], reduced: str, signal: dict) -> bool:
    """
    Return True if at least one evidence line for this signal appears in the
    reduced output (matched by content fingerprint substring).
    """
    for span in signal.get('evidence_lines', []):
        # span is [start, end] (1-based, inclusive)
        start, end = span[0], span[1]
        for ln in range(start, end + 1):
            if ln < 1 or ln > len(raw_lines):
                continue
            fp = fingerprint(raw_lines[ln - 1])
            if fp is None:
                continue
            # Use a key substring: first 40 chars is usually enough to be unique
            key = fp[:60]
            if key in reduced:
                return True
    return False


def evaluate_case(case_dir: Path, binary: str, budget: int) -> dict:
    raw_log = case_dir / 'raw.log'
    gt_file = case_dir / 'ground_truth.json'
    if not raw_log.exists() or not gt_file.exists():
        return None

    raw_lines = raw_log.read_text(errors='replace').splitlines()
    gt = json.loads(gt_file.read_text())
    reduced = run_logreduce(binary, str(raw_log), budget)

    # Strip noise from reduced output for matching; also collapse whitespace
    # so fingerprints (which collapse whitespace) can match against the haystack.
    reduced_stripped = '\n'.join(' '.join(strip_noise(l).split()) for l in reduced.splitlines())

    signals = gt.get('required_signals', [])
    critical = [s for s in signals if s.get('importance') == 'critical']
    important = [s for s in signals if s.get('importance') == 'important']

    def recall_for(sig_list):
        if not sig_list:
            return 1.0, 0, 0
        hits = sum(1 for s in sig_list if check_signal_recall(raw_lines, reduced_stripped, s))
        return hits / len(sig_list), hits, len(sig_list)

    crit_recall, crit_hits, crit_total = recall_for(critical)
    imp_recall, imp_hits, imp_total = recall_for(important)

    # Count kept lines (non-header, non-gap lines in output)
    header_lines = sum(1 for l in reduced.splitlines() if l.startswith('#'))
    gap_lines = sum(1 for l in reduced.splitlines() if l.startswith('…'))
    kept = len(reduced.splitlines()) - header_lines - gap_lines

    return {
        'case_id': case_dir.name,
        'raw_lines': len(raw_lines),
        'kept_lines': kept,
        'keep_pct': round(kept / max(len(raw_lines), 1) * 100, 1),
        'critical_recall': round(crit_recall, 3),
        'critical_hits': crit_hits,
        'critical_total': crit_total,
        'important_recall': round(imp_recall, 3),
        'important_hits': imp_hits,
        'important_total': imp_total,
        'failure_category': json.loads((case_dir / 'case.json').read_text()).get('failure_category', '?'),
    }


def main():
    parser = argparse.ArgumentParser(description='Evaluate logreduce recall on LogDx benchmark')
    parser.add_argument('cases_dir', help='Path to LogDx/cases directory')
    parser.add_argument('--budget', type=int, default=8000, help='Token budget (default 8000)')
    parser.add_argument('--split', default=None, help='Only evaluate one split (dev/holdout/stress/v2)')
    parser.add_argument('--binary', default='./target/release/logreduce', help='Path to logreduce binary')
    parser.add_argument('--json', action='store_true', help='Output raw JSON results')
    args = parser.parse_args()

    cases_root = Path(args.cases_dir)
    splits = [args.split] if args.split else ['dev', 'holdout', 'stress', 'v2']

    def iter_cases(split_dir: Path, split_name: str):
        """Yield (split_label, case_dir) — handles one-level-nested v2 layout."""
        for entry in sorted(split_dir.iterdir()):
            if entry.name in ('split_manifest.json',):
                continue
            if (entry / 'raw.log').exists():
                yield split_name, entry
            elif entry.is_dir():
                # v2 has sub-splits (dev/holdout/stress) inside cases/v2/
                for sub in sorted(entry.iterdir()):
                    if sub.is_dir() and (sub / 'raw.log').exists():
                        yield f'{split_name}/{entry.name}', sub

    results = []
    for split in splits:
        split_dir = cases_root / split
        if not split_dir.exists():
            continue
        for split_label, case_dir in iter_cases(split_dir, split):
            print(f'  evaluating {split_label}/{case_dir.name} ...', end=' ', flush=True)
            r = evaluate_case(case_dir, args.binary, args.budget)
            if r:
                r['split'] = split_label
                results.append(r)
                print(f"critical={r['critical_hits']}/{r['critical_total']} ({r['critical_recall']:.0%})  kept={r['keep_pct']}%")
            else:
                print('SKIP (missing files)')

    if args.json:
        print(json.dumps(results, indent=2))
        return

    print()
    print('=' * 72)
    print(f'LOGREDUCE RECALL  |  budget={args.budget} tokens')
    print('=' * 72)
    print(f"{'Case':<45} {'Cat':<18} {'Keep%':>5}  {'Critical':>10}  {'Important':>10}")
    print('-' * 72)
    for r in results:
        crit = f"{r['critical_hits']}/{r['critical_total']} ({r['critical_recall']:.0%})"
        imp  = f"{r['important_hits']}/{r['important_total']} ({r['important_recall']:.0%})" if r['important_total'] else 'n/a'
        print(f"{r['case_id']:<45} {r['failure_category']:<18} {r['keep_pct']:>4.0f}%  {crit:>10}  {imp:>10}")

    if results:
        print('-' * 72)
        avg_crit = sum(r['critical_recall'] for r in results) / len(results)
        total_crit_hits = sum(r['critical_hits'] for r in results)
        total_crit = sum(r['critical_total'] for r in results)
        avg_keep = sum(r['keep_pct'] for r in results) / len(results)
        print(f"{'AGGREGATE':<45} {'':<18} {avg_keep:>4.0f}%  {total_crit_hits}/{total_crit} ({total_crit_hits/max(total_crit,1):.0%})")
        print()
        print(f"Cases: {len(results)}  |  Avg critical recall: {avg_crit:.1%}  |  Avg keep%: {avg_keep:.1f}%")


if __name__ == '__main__':
    main()
