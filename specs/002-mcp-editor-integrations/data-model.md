# Data Model: MCP Editor Integrations

## Entities

### ReduceLogInput
Input accepted by the `reduce_log` MCP tool.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `path` | `string` | No† | — | Absolute path to a local log file |
| `text` | `string` | No† | — | Raw log content inline |
| `budget` | `number` | No | `8000` | Token budget ceiling |
| `context` | `number` | No | `2` | Context lines around each kept line |
| `no_redact` | `boolean` | No | `false` | Disable secret/PII redaction |
| `format` | `string` | No | `"auto"` | Log format hint: `auto \| json \| logfmt \| syslog \| klog \| plain` |

† Exactly one of `path` or `text` must be provided; providing both or neither is an error.

**Validation rules**:
- `path` must be an absolute path to a readable file
- `text`, if provided, must be a non-empty string
- `budget` must be a positive integer ≤ 200,000
- `context` must be a non-negative integer ≤ 10
- `format` must be one of the allowed values

---

### ReduceLogOutput
JSON object returned by the `reduce_log` MCP tool.

| Field | Type | Description |
|---|---|---|
| `text` | `string` | Full binary stdout: `# LOG SUMMARY` header + compressed body |
| `stats.input_lines` | `number` | Total lines in the input log |
| `stats.kept_lines` | `number` | Lines kept after reduction |
| `stats.input_tokens` | `number` | Estimated input token count (tiktoken cl100k_base) |
| `stats.output_tokens` | `number` | Estimated output token count |

---

### HookInput
JSON object that Claude Code sends to the `UserPromptSubmit` hook on stdin.

| Field | Type | Description |
|---|---|---|
| `session_id` | `string` | Claude Code session identifier |
| `prompt` | `string` | The raw user prompt text as typed |
| *(other fields)* | — | Additional Claude Code metadata; hook ignores unknown fields |

---

### HookOutput
JSON object the hook writes to stdout.

| Field | Type | Description |
|---|---|---|
| `prompt` | `string` | The (possibly rewritten) prompt text to send to the model |

If no log content is detected, the hook outputs `{}` (empty object) or the unchanged `{ "prompt": originalPrompt }` — both are valid pass-through responses.

---

### LogDetectionResult
Internal result of the log-detection heuristic (not serialized externally).

| Field | Type | Description |
|---|---|---|
| `detected` | `boolean` | Whether log content was found |
| `type` | `"path" \| "inline" \| null` | Detection mode |
| `content` | `string \| null` | Extracted file path or inline log block |
| `startIndex` | `number \| null` | Character offset of the log fragment in the prompt |
| `endIndex` | `number \| null` | End offset |

---

### InstallConfig
Configuration used by the `--install` CLI command.

| Field | Type | Description |
|---|---|---|
| `projectRoot` | `string` | Absolute path to the project root (cwd by default) |
| `hooksDir` | `string` | `.claude/hooks/` (derived from projectRoot) |
| `settingsPath` | `string` | `.claude/settings.json` (derived from projectRoot) |
| `hookScriptPath` | `string` | `.claude/hooks/UserPromptSubmit.js` |
| `serverCommand` | `string` | Command registered in settings (`npx logreduce-mcp`) |

---

## State Transitions

The MCP server is stateless — each `reduce_log` invocation is independent. No session state is maintained between calls.

The installer is idempotent:

```
settings.json missing     → create with MCP entry + hook registration
settings.json exists, no hook  → merge hook registration (preserve existing keys)
settings.json exists, hook present → validate hook command is current; update if stale
hook script missing       → copy hook script
hook script present       → overwrite (always use the version from the installed package)
```
