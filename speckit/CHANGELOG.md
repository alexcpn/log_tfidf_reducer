# Changelog

## 0.3.1 — 2026-10-01

Security hardening from a pre-submission review.

- Paths from the bug report are vetted before any command runs on them.
  Paths with shell metacharacters are skipped, and paths outside the project,
  under `.git/` or dotfiles need the user's confirmation. Commands take the
  path single-quoted after `--`.
- `evidence.md` opens with an "untrusted log data" banner, and each reduced
  log is fenced longer than any backtick run inside it, so log text cannot
  break out of its code block.
- Slugs are limited to `a-z`, `0-9` and `-`.
- Pasted logs go to a `mktemp` file that is deleted afterwards.
- Requires `logreduce` >= 0.3.1, which redacts JSON `"api_key": "…"` values,
  quoted values with spaces, passwords in URLs, `Authorization: Basic`
  credentials and `--password <value>` flags.

## 0.3.0 — 2026-10-01

- Version aligned with the `logreduce` release that ships it (`v0.3.0`), as
  the community catalog requires. No functional change from 0.1.0.
- The extension archive now includes `LICENSE`.

## 0.1.0 — 2026-09-30

- `/speckit.logreduce.intake`: reduce the logs in a bug report into
  `.specify/bugs/<slug>/evidence.md` (appends, never overwrites).
- Optional `before_bug_assess` hook for the bundled `bug` extension.
