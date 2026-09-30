# [Feature]: Hook events for the bundled `bug` extension (log intake as a pre-assess step)

<!-- Draft for github/spec-kit — feature_request.yml. Each H2 below maps to a form field. -->

## Problem Statement

`speckit.bug.assess` ingests a bug report as pasted text or a URL. Real bug
reports often come with a **log**: a CI run, `kubectl logs`, a crash-looping
service. These run to thousands or millions of lines. Today the agent either
reads the raw log (blowing the context window and burying the one relevant
error under repeats) or skims it ad hoc, and whatever it looked at is not
recorded in `.specify/bugs/<slug>/`, so `speckit.bug.fix` and
`speckit.bug.test` can't rely on it.

There is also no way for another extension to add a step to the bug workflow.
Core commands (`plan`, `tasks`, `implement`, …) check `.specify/extensions.yml`
for `before_*` / `after_*` hooks, but the three `speckit.bug.*` commands don't,
and no `*_bug_*` events are defined.

## Proposed Solution

Two small, tool-neutral changes to `extensions/bug/`:

**1. Hook events for the three bug commands**

Add the standard hook-check block (the same block `templates/commands/plan.md`
uses) to each command, for these events:

| Event | Where it fires |
|---|---|
| `before_bug_assess` | After **Slug Resolution**/**Prerequisites** (so `BUG_SLUG` / `BUG_DIR` exist), before **Execution → Ingest** |
| `after_bug_assess` | After `assessment.md` is written |
| `before_bug_fix` / `after_bug_fix` | Around `fix.md` |
| `before_bug_test` / `after_bug_test` | Around `test.md` |

`before_bug_assess` has to run *after* slug resolution rather than at the top
of the command, because a pre-hook needs to know `BUG_DIR` to write into. This
uses the handoff the bug commands already rely on: "downstream commands in
the same session may reuse [the slug] from context without re-prompting."

**2. An `evidence.md` convention in `speckit.bug.assess`**

In **Execution → 1. Ingest the bug report**, add:

> If `BUG_DIR/evidence.md` exists (for example, written by a
> `before_bug_assess` hook), read it as part of the report. Cite it under
> **Report** and prefer it over re-reading any raw log it was derived from.

Also add `evidence.md` (optional) to the per-bug directory layout in the
README, alongside `assessment.md`, `fix.md` and `test.md`.

Neither change names or depends on a specific tool. With no hooks registered
and no `evidence.md`, behavior is exactly as today.

### Motivating hook: log intake

With these in place, a community extension can register:

```yaml
hooks:
  before_bug_assess:
    command: speckit.logreduce.intake
    optional: true
    prompt: "Reduce attached logs before assessing?"
    description: "Condense large logs into BUG_DIR/evidence.md"
```

`speckit.logreduce.intake` finds log files referenced in the report, runs
[`logreduce`](https://github.com/alexcpn/log_tfidf_reducer) (a single static
binary: TF-IDF over masked templates plus severity weighting) with a token
budget, and writes `BUG_DIR/evidence.md` containing the command it ran, the
source path, the summary header, and the reduced log. On a 1M-line log that is
a ~99.9% token reduction. On the LogDx CI-incident benchmark (35 cases) it kept
99% of the human-labelled critical lines at an 8k-token budget. Assess, fix and
test then all work from the same saved evidence file instead of the raw log,
and a reviewer can check what the agent actually saw.

That extension already exists
([`speckit/`](https://github.com/alexcpn/log_tfidf_reducer/tree/main/speckit))
and registers `before_bug_assess` today. The manifest passes validation, but the
hook never fires because the event doesn't exist yet, so users have to run
intake by hand first. I'll maintain it and submit it to the community catalog.
This issue only asks for the extension points.

## Alternatives Considered

- **Bake log reduction into `speckit.bug.assess`.** That adds a tool-specific
  dependency to a bundled extension. The hook keeps core neutral and lets
  other intake steps (Sentry export, trace fetch, core-dump symbolication) plug
  in the same way.
- **Standalone `speckit.logreduce.*` command the user runs first.** This works
  today, but the user has to remember to run it, and assess doesn't know to
  read the result. It duplicates the bug workflow instead of joining it.
- **Hook at the very top of `assess`, before slug resolution.** The hook would
  have nowhere to write, and the intake command would have to guess the slug.

## Component

Extensions: bundled `bug` extension (`extensions/bug/`)

## AI Agent (if applicable)

All. Hooks are plain command-template instructions.

## Use Cases

1. A CI job fails with a 40k-line log. The user runs
   `/speckit.bug.assess ci-failure.log flaky deploy step`. The optional hook
   offers intake, `evidence.md` holds about 7k tokens of reduced log, and the
   assessment cites the first error and the crash-loop pattern from it.
2. `speckit.bug.test` fails and recommends re-running assess. The new failing
   log goes through the same intake, so `evidence.md` shows the before and
   after.
3. Other extensions (Jira sync after `after_bug_assess`, notifications after
   `after_bug_test`) get extension points they don't have today.

## Acceptance Criteria

- [ ] `speckit.bug.{assess,fix,test}` check `.specify/extensions.yml` for their
      `before_*` / `after_*` events using the same block as core commands.
- [ ] `before_bug_assess` fires after `BUG_DIR` exists and before Ingest.
- [ ] `speckit.bug.assess` reads `BUG_DIR/evidence.md` when present and cites it.
- [ ] The new events are listed under **Hook Events** in
      `EXTENSION-API-REFERENCE.md`.
- [ ] With no hooks registered, the command output is unchanged.

I'm happy to send the PR.

## Additional Context

- Builds on #2870 / #2871 (the bundled `bug` extension).
- The community `spec-kit-bugfix` extension (`speckit.bugfix.report`) could
  read `evidence.md` in the same way.

## AI Disclosure

Drafted with Claude Code (Claude Opus 5.5) from my notes. I reviewed it