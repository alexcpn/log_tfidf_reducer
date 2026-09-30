---
name: logreduce
description: Reduce large log files with logreduce before reading them. Use whenever you encounter, discover, or are about to read/view/analyze/debug/summarize a log file (or log-like output — console dumps, kubectl logs, journalctl, CI output, stack-trace dumps) that is roughly 500 lines or longer.
---

When you encounter a log file — whether the user names it, you find it via
`find`/`grep`/`ls`, or it appears in command output — that is roughly 500
lines or longer, do NOT read it directly (file-read tool, `cat`, etc.). Instead:

1. Run `logreduce <path-to-file>` in the terminal first.
2. Read the reduced output. It keeps every error, warning, and unique event
   while dropping repetitive noise — typically 99%+ smaller, so it fits in
   context and saves tokens.
3. If you need more surrounding context for a specific finding, re-run with
   `logreduce <path-to-file> --context 5` or a larger `--budget` (e.g.
   `--budget 32000`).

If a hook already injected a reduced log for a file (a block starting
`logreduce hook:`), use that — don't reduce the same file again.

Files under ~500 lines are small enough to read directly — skip logreduce
for those.

Requires the `logreduce` binary on PATH — download from
https://github.com/alexcpn/log_tfidf_reducer/releases for your OS platform.
