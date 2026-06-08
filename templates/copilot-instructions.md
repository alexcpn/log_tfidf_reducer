## Reducing large log files with logreduce

When asked to read, view, analyze, debug, or summarize a log file — or any
file that looks like one (`.log`, console/CI output, `kubectl logs` /
`journalctl` dumps, stack-trace dumps, etc.) — that is roughly 500 lines or
longer, do NOT read it directly. Instead:

1. Run `logreduce <path-to-file>` in the terminal first.
2. Read the reduced output. It keeps every error, warning, and unique event
   while dropping repetitive noise — typically 99%+ smaller, so it fits in
   context and saves tokens.
3. If you need more surrounding context for a specific finding, re-run with
   `logreduce <path-to-file> --context 5` or a larger `--budget` (e.g.
   `--budget 32000`).

Files under ~500 lines are small enough to read directly — skip logreduce
for those.

Requires the `logreduce` binary on PATH and **Agent mode** in Copilot Chat
(custom instructions and terminal commands aren't available in standard chat).
