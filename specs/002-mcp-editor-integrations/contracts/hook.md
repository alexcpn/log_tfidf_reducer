# Contract: Claude Code UserPromptSubmit Hook

## Hook Registration

**Event**: `UserPromptSubmit`

**Script**: `.claude/hooks/UserPromptSubmit.js` (installed by `npx logreduce-mcp --install`)

**Settings entry** (`.claude/settings.json`):
```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "type": "command",
      "command": "node .claude/hooks/UserPromptSubmit.js"
    }]
  }
}
```

## Input Contract (stdin)

Claude Code writes a JSON object to the hook's stdin:

```json
{
  "session_id": "<string>",
  "prompt": "<the user's raw prompt text>"
}
```

The hook must read stdin completely before processing (use synchronous read or buffer stdin events).

## Output Contract (stdout)

The hook writes a JSON object to stdout:

**Pass-through (no log detected)**:
```json
{}
```

**Prompt rewritten (log detected and reduced)**:
```json
{
  "prompt": "<rewritten prompt with reduced log content>"
}
```

The rewritten prompt replaces the log fragment (file path or inline block) with the `# LOG SUMMARY` header + compressed body. The rest of the prompt (the user's question) is preserved verbatim.

**Blocked (error, fail-open)**:
The hook MUST NOT block — if reduction fails or times out, it outputs `{}` (pass-through) and logs the error to stderr. The hook MUST NOT exit with code 2 for reduction failures.

## Detection Heuristics

The hook applies these checks in order to decide whether to reduce:

1. **File path pattern**: Prompt contains a string matching a local file path (absolute or relative). The file exists and has ≥ 500 lines → reduce via `logreduce <path>`.
2. **Inline block**: Prompt contains ≥ 500 consecutive lines where ≥ 70% match a log-line pattern (timestamp + optional severity keyword) → reduce the block via stdin.
3. **No match**: Output `{}` immediately (zero latency overhead — SC-007).

## Timing Constraints

- **Timeout**: 5 seconds (FR-006). If `logreduce` does not return within 5s, output `{}`.
- **Target latency**: < 3s for a 10,000-line log (SC-004).
- **Zero overhead**: If no log is detected, the hook must complete in < 50ms.

## Prompt Rewrite Format

When a log is detected and reduced, the hook rewrites the prompt as:

```
[original question text — everything before/after the log fragment]

--- LOG (reduced from N lines to M kept) ---
[logreduce output: # LOG SUMMARY header + compressed body]
---
```

The log fragment in the original prompt (path or inline block) is replaced by this block.
