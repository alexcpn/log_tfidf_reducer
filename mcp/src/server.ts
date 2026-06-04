import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { invokeReducer } from "./reducer.js";
import { parseOutput } from "./parser.js";

// ── Input validation ─────────────────────────────────────────────────────────

const ReduceLogSchema = z.object({
  path: z.string().optional().describe("Absolute path to a local log file"),
  text: z.string().optional().describe("Raw log content as a string"),
  budget: z.number().int().min(100).max(200_000).default(8000).describe(
    "Maximum output token budget (default 8000)"
  ),
  context: z.number().int().min(0).max(10).default(2).describe(
    "Context lines around each kept line (default 2)"
  ),
  no_redact: z.boolean().default(false).describe(
    "Disable secret/PII redaction (default false)"
  ),
  format: z
    .enum(["auto", "json", "logfmt", "syslog", "klog", "plain"])
    .default("auto")
    .describe("Log format hint"),
});

type ReduceLogInput = z.infer<typeof ReduceLogSchema>;

/** Validate that exactly one of path/text is provided. */
export function validateInput(input: Partial<ReduceLogInput>): void {
  const hasPath = input.path !== undefined && input.path !== "";
  const hasText = input.text !== undefined && input.text !== "";
  if (hasPath && hasText) {
    throw new Error("Provide either path or text, not both");
  }
  if (!hasPath && !hasText) {
    throw new Error("Provide either path or text");
  }
}

/** Build CLI args for the logreduce binary. */
export function buildArgs(input: Partial<ReduceLogInput>): string[] {
  const args: string[] = [];

  // File path goes first (positional arg); stdin is used when text is provided
  if (input.path) {
    args.push(input.path);
  }

  args.push("--budget", String(input.budget ?? 8000));
  args.push("--context", String(input.context ?? 2));
  args.push("--stats"); // always — needed for token count parsing

  if (input.no_redact) {
    args.push("--no-redact");
  }
  if (input.format && input.format !== "auto") {
    args.push("--format", input.format);
  }

  return args;
}

// ── MCP server ───────────────────────────────────────────────────────────────

export function createServer(): McpServer {
  const server = new McpServer({
    name: "logreduce-mcp",
    version: "0.1.0",
  });

  server.tool(
    "reduce_log",
    "Reduce a large log file or raw log text to a compact, LLM-readable form within a token budget. " +
      "Returns the reduced log and reduction statistics. " +
      "Call this before asking about any log that exceeds a few hundred lines.",
    ReduceLogSchema.shape,
    async (rawInput) => {
      const input = rawInput as ReduceLogInput;
      try {
        validateInput(input);
      } catch (e: unknown) {
        throw new Error((e as Error).message);
      }

      const args = buildArgs(input);
      const stdin = input.text;

      const { stdout, stderr } = await invokeReducer(args, stdin);
      const result = parseOutput(stdout, stderr);

      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify(result),
          },
        ],
      };
    }
  );

  return server;
}

/** Start the MCP stdio server. */
export async function startServer(): Promise<void> {
  const server = createServer();
  const transport = new StdioServerTransport();
  await server.connect(transport);
  // Log to stderr only — stdout is the MCP JSON-RPC channel
  console.error("logreduce-mcp server started");
}
