#!/usr/bin/env python3
"""
Evaluate logreduce on real GitHub issues.

This script reads evidence/cases.json, downloads raw logs (cached locally in
a scratch or temporary directory if not already downloaded), runs `logreduce`
with `--stats`, and prints/saves quantitative comparison metrics.

Usage:
    python evidence/run_eval.py
    python evidence/run_eval.py --binary ./target/release/logreduce
"""
import argparse
import json
import subprocess
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
MANIFEST_PATH = HERE / "cases.json"
RESULTS_PATH = HERE / "eval_results.json"
REDUCED_DIR = HERE / "reduced_outputs"


def main():
    parser = argparse.ArgumentParser(description="Evaluate logreduce on real GitHub issues")
    parser.add_argument("--binary", default="logreduce", help="Path to logreduce binary (default: 'logreduce' on PATH)")
    parser.add_argument("--cache-dir", default=str(HERE / ".cache"), help="Directory to cache downloaded raw logs")
    args = parser.parse_args()

    cache_dir = Path(args.cache_dir)
    cache_dir.mkdir(parents=True, exist_ok=True)
    REDUCED_DIR.mkdir(parents=True, exist_ok=True)

    cases = json.loads(MANIFEST_PATH.read_text())
    results = []

    print(f"Evaluating {len(cases)} real GitHub issues with binary '{args.binary}'...\n")

    for c in cases:
        case_id = c["id"]
        raw_log = cache_dir / f"{case_id}.log"
        out_reduced = REDUCED_DIR / f"{case_id}.reduced.txt"

        if not raw_log.exists():
            print(f"[{case_id}] Downloading raw log from {c['url']} ...")
            req = urllib.request.Request(c["url"], headers={"User-Agent": "Mozilla/5.0"})
            with urllib.request.urlopen(req, timeout=45) as resp:
                raw_log.write_bytes(resp.read())

        raw_bytes = raw_log.stat().st_size
        raw_lines = len(raw_log.read_text(errors="replace").splitlines())

        print(f"[{case_id}] Running logreduce on {raw_lines} lines ({raw_bytes:,} bytes) ...")
        t0 = time.perf_counter()
        proc = subprocess.run(
            [args.binary, str(raw_log), "--stats", "-o", str(out_reduced)],
            capture_output=True,
            text=True
        )
        t1 = time.perf_counter()
        duration = t1 - t0

        if proc.returncode != 0:
            print(f"  FAILED (exit {proc.returncode}): {proc.stderr}")
            continue

        reduced_bytes = out_reduced.stat().st_size
        reduced_lines = len(out_reduced.read_text(errors="replace").splitlines())
        line_reduction = (1.0 - (reduced_lines / max(raw_lines, 1))) * 100
        byte_reduction = (1.0 - (reduced_bytes / max(raw_bytes, 1))) * 100

        res_entry = {
            "id": case_id,
            "repo": c["repo"],
            "issue_url": c["issue_url"],
            "title": c["title"],
            "domain": c["domain"],
            "raw_lines": raw_lines,
            "reduced_lines": reduced_lines,
            "line_reduction_pct": round(line_reduction, 1),
            "raw_bytes": raw_bytes,
            "reduced_bytes": reduced_bytes,
            "byte_reduction_pct": round(byte_reduction, 1),
            "duration_sec": round(duration, 3),
            "stderr": proc.stderr.strip()
        }
        results.append(res_entry)
        print(f"  → {reduced_lines} lines ({reduced_bytes:,} bytes) | -{line_reduction:.1f}% lines in {duration:.3f}s")
        if proc.stderr:
            print(f"  {proc.stderr.strip()}")

    RESULTS_PATH.write_text(json.dumps(results, indent=2))
    print(f"\nSaved updated results to {RESULTS_PATH}")

    # Summary table
    print("\n" + "=" * 80)
    print(f"{'Repository / Issue':<38} {'Domain':<22} {'Lines':<14} {'Line Red%':<10} {'Time':<6}")
    print("-" * 80)
    for r in results:
        issue_label = f"{r['repo']}#{r['id'].split('-')[-1]}"
        lines_label = f"{r['raw_lines']} → {r['reduced_lines']}"
        print(f"{issue_label:<38} {r['domain'][:20]:<22} {lines_label:<14} {r['line_reduction_pct']:>6.1f}%   {r['duration_sec']:>5.2f}s")
    print("=" * 80)


if __name__ == "__main__":
    main()
