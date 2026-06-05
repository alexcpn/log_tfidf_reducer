# Contract: `reduce_log` MCP Tool

## Tool Registration

**Name**: `reduce_log`

**Description** (shown to the model):
> Reduce a large log file or raw log text to a compact, LLM-readable form within a token budget. Returns the reduced log and reduction statistics. Call this before asking about any log that exceeds a few hundred lines.

## Input Schema (JSON Schema)

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Absolute path to a local log file. Provide either path or text, not both."
    },
    "text": {
      "type": "string",
      "description": "Raw log content as a string. Provide either path or text, not both."
    },
    "budget": {
      "type": "integer",
      "description": "Maximum output token budget (approximate). Default: 8000.",
      "minimum": 100,
      "maximum": 200000,
      "default": 8000
    },
    "context": {
      "type": "integer",
      "description": "Number of context lines to keep around each kept line. Default: 2.",
      "minimum": 0,
      "maximum": 10,
      "default": 2
    },
    "no_redact": {
      "type": "boolean",
      "description": "Disable secret/PII redaction (redaction is ON by default). Default: false.",
      "default": false
    },
    "format": {
      "type": "string",
      "enum": ["auto", "json", "logfmt", "syslog", "klog", "plain"],
      "description": "Force log format detection. Default: auto.",
      "default": "auto"
    }
  },
  "oneOf": [
    { "required": ["path"] },
    { "required": ["text"] }
  ],
  "additionalProperties": false
}
```

## Output Schema

The tool returns MCP content of type `text` containing a JSON string:

```json
{
  "text": "<full logreduce stdout: # LOG SUMMARY header + compressed body>",
  "stats": {
    "input_lines": 50000,
    "kept_lines": 312,
    "input_tokens": 640000,
    "output_tokens": 7421
  }
}
```

The `text` field is the primary content the model should use for analysis.
The `stats` field provides machine-readable reduction metrics.

## Error Responses

| Condition | MCP error code | Message |
|---|---|---|
| Neither `path` nor `text` provided | `InvalidParams` | "Provide either path or text" |
| Both `path` and `text` provided | `InvalidParams` | "Provide either path or text, not both" |
| `path` not found or unreadable | `InvalidParams` | "File not found: <path>" |
| `logreduce` binary not on PATH | `InternalError` | "logreduce not found. Install with: cargo install logreduce" |
| Binary exits non-zero | `InternalError` | "logreduce failed: <stderr>" |
| Timeout (>30s) | `InternalError` | "logreduce timed out after 30s" |
| Empty log | Returns normally | `text` is the header-only output; `stats.kept_lines` = 0 |

## Invocation Semantics

1. If `path` is provided: the server spawns `logreduce <path> --budget <N> [flags]`.
2. If `text` is provided: the server spawns `logreduce --budget <N> [flags]` and pipes `text` to stdin.
3. In both cases `--stats` is always added so token counts appear in stderr and can be parsed.
4. The server captures stdout as `text` and parses the `# LOG SUMMARY` header + stderr stats output to populate `stats`.
