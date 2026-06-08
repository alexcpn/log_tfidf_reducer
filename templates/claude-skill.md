---
name: logreduce
description: Reduce large log files with logreduce before reading them. Use whenever you encounter, discover, or are about to read/view/analyze/debug/summarize a log file (or log-like output — console dumps, kubectl logs, journalctl, CI output, stack-trace dumps) that is roughly 500 lines or longer.
---

When you encounter a log file — whether the user names it, you find it via
`find`/`grep`/`ls`, or it appears in command output — that is roughly 500
lines or longer, do NOT read it directly with `Read` or `cat`. Instead:

1. Run `logreduce <path-to-file>` in the terminal first.
2. Read the reduced output. It keeps every error, warning, and unique event
   while dropping repetitive noise — typically 99%+ smaller, so it fits in
   context and saves tokens.
3. If you need more surrounding context for a specific finding, re-run with
   `logreduce <path-to-file> --context 5` or a larger `--budget` (e.g.
   `--budget 32000`).

Files under ~500 lines are small enough to read directly — skip logreduce
for those.

This complements the `logreduce hook` (UserPromptSubmit) already installed in
this project's `.claude/settings.json`, which deterministically reduces logs
*named or pasted directly in your prompt*. This skill covers the case the
hook can't see: logs you discover yourself mid-session.

Requires the `logreduce` binary on PATH — see the project README for install
instructions (a single static binary, no Node/npm required).
