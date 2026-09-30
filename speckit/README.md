# Log Intake — a Spec Kit extension for logreduce

Adds `/speckit.logreduce.intake`, a log-intake step for Spec Kit's bundled
`bug` triage workflow. It runs [`logreduce`](../README.md) on the logs attached
to a bug report and saves the result as `.specify/bugs/<slug>/evidence.md`.
`/speckit.bug.assess`, `fix` and `test` then work from one saved, reviewable,
reduced log (8k tokens by default) instead of each re-reading a raw log that
may run to thousands or millions of lines.

## Install

1. Put the `logreduce` binary (≥ 0.2.0) on your PATH — see the
   [releases page](https://github.com/alexcpn/log_tfidf_reducer/releases).
2. Add the bundled bug workflow and this extension to your Spec Kit project:

   ```bash
   specify extension add bug
   specify extension add logreduce --from \
     https://github.com/alexcpn/log_tfidf_reducer/releases/latest/download/logreduce-speckit.zip
   ```

   To install from a checkout: `specify extension add --dev path/to/log_tfidf_reducer/speckit`.

## Use

```text
/speckit.logreduce.intake slug=deploy-timeout ci/run-4812.log
/speckit.bug.assess slug=deploy-timeout Deploy step times out; log in ci/run-4812.log, evidence in .specify/bugs/deploy-timeout/evidence.md
```

Options in the input: `slug=<name>` and `budget=<tokens>` (default `8000`).
Running intake again (for example after `/speckit.bug.test` fails) adds a new
dated section to `evidence.md` rather than overwriting the earlier one.

### As a hook

The extension registers an optional `before_bug_assess` hook, so
`/speckit.bug.assess` offers the intake step itself — once the bundled `bug`
extension emits that event. That change is
[proposed upstream](../docs/spec-kit-proposal.md). Until it lands, run intake
yourself first, as shown above.

## What goes in `evidence.md`

For each log: the source path, the original line count, the exact `logreduce`
command, its stats, and the reduced log verbatim. Logs under ~500 lines are
listed as "read directly". Secret/PII redaction stays on.
