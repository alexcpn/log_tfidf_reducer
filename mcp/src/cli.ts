import { findBinary } from "./reducer.js";
import { startServer } from "./server.js";
import { install } from "./install.js";

const args = process.argv.slice(2);

if (args.includes("--version") || args.includes("-v")) {
  console.log("logreduce-mcp 0.1.0");
  process.exit(0);
}

if (args.includes("--install")) {
  const editorFlag = args.find((a) => a.startsWith("--editor="));
  const editor = editorFlag ? editorFlag.split("=")[1] : "claude-code";
  install({ editor: editor as "claude-code" | "cursor" | "copilot" });
  process.exit(0);
}

if (args.includes("--help") || args.includes("-h")) {
  console.log(
    [
      "logreduce-mcp — MCP server for logreduce",
      "",
      "Usage:",
      "  logreduce-mcp              Start the MCP stdio server",
      "  logreduce-mcp --install    Install Claude Code hook + MCP config",
      "  logreduce-mcp --install --editor=cursor    Copy Cursor MCP config",
      "  logreduce-mcp --install --editor=copilot   Print Copilot config",
      "  logreduce-mcp --version    Print version",
    ].join("\n")
  );
  process.exit(0);
}

// Default: start MCP server
try {
  findBinary();
} catch (e: unknown) {
  console.error("Error:", (e as Error).message);
  process.exit(1);
}

startServer().catch((e: unknown) => {
  console.error("Fatal:", (e as Error).message);
  process.exit(1);
});
