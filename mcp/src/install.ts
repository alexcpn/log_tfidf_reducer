import { existsSync, mkdirSync, readFileSync, writeFileSync, copyFileSync } from "fs";
import { resolve, join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface InstallOptions {
  projectRoot?: string;
  editor?: "claude-code" | "cursor" | "copilot";
}

export function install(options: InstallOptions = {}): void {
  const projectRoot = options.projectRoot ?? process.cwd();
  const editor = options.editor ?? "claude-code";

  switch (editor) {
    case "claude-code":
      installClaudeCode(projectRoot);
      break;
    case "cursor":
      installCursor(projectRoot);
      break;
    case "copilot":
      installCopilot();
      break;
    default:
      throw new Error(`Unknown editor: ${editor}`);
  }
}

function installClaudeCode(projectRoot: string): void {
  const hooksDir = join(projectRoot, ".claude", "hooks");
  const settingsPath = join(projectRoot, ".claude", "settings.json");
  const hookDest = join(hooksDir, "UserPromptSubmit.js");

  // Find hook source — in dist/ after build (both local and npm-installed)
  const hookSrc = resolve(__dirname, "hook.js");
  // Fallback: src/hook.js for cases where build didn't copy it
  const hookSrcFallback = resolve(__dirname, "../src/hook.js");
  const resolvedHookSrc = existsSync(hookSrc) ? hookSrc : hookSrcFallback;
  if (!existsSync(resolvedHookSrc)) {
    throw new Error(`Hook source not found at ${hookSrc} or ${hookSrcFallback}`);
  }

  // Create .claude/hooks/ if needed
  mkdirSync(hooksDir, { recursive: true });

  // Copy hook script (always overwrite to stay current)
  copyFileSync(resolvedHookSrc, hookDest);
  console.log(`✓ Hook written: ${hookDest}`);

  // Merge settings.json
  let settings: Record<string, unknown> = {};
  if (existsSync(settingsPath)) {
    try {
      settings = JSON.parse(readFileSync(settingsPath, "utf8"));
    } catch {
      console.warn("Warning: existing settings.json is invalid JSON — overwriting");
    }
  }

  // Deep-merge hook entry (Claude Code ≥2.x format: matcher + hooks array)
  const hooks = (settings.hooks as Record<string, unknown>) ?? {};
  const existing = (hooks["UserPromptSubmit"] as unknown[]) ?? [];
  const hookCommand = "node .claude/hooks/UserPromptSubmit.js";
  const hookEntry = {
    matcher: "",
    hooks: [{ type: "command", command: hookCommand }],
  };

  // Idempotent: only add if no entry already references our command
  const alreadyPresent = existing.some((e) => {
    if (typeof e !== "object" || e === null) return false;
    const entry = e as Record<string, unknown>;
    // Old format (command at top level)
    if (entry.command === hookCommand) return true;
    // New format (command nested in hooks array)
    const nested = entry.hooks as unknown[] | undefined;
    return Array.isArray(nested) &&
      nested.some((h) => typeof h === "object" && h !== null &&
        (h as Record<string, unknown>).command === hookCommand);
  });
  if (!alreadyPresent) {
    hooks["UserPromptSubmit"] = [...existing, hookEntry];
  }

  // Merge MCP server entry
  const mcpServers =
    ((settings.mcpServers as Record<string, unknown>) ?? {});
  if (!mcpServers["logreduce"]) {
    mcpServers["logreduce"] = { command: "npx", args: ["logreduce-mcp"] };
  }

  settings.hooks = hooks;
  settings.mcpServers = mcpServers;

  writeFileSync(settingsPath, JSON.stringify(settings, null, 2) + "\n");
  console.log(`✓ Settings merged: ${settingsPath}`);
  console.log("\nClaude Code integration installed. Start a new session to activate.");
}

function installCursor(projectRoot: string): void {
  const configPath = join(projectRoot, ".cursor", "mcp.json");
  mkdirSync(join(projectRoot, ".cursor"), { recursive: true });

  let config: Record<string, unknown> = {};
  if (existsSync(configPath)) {
    try {
      config = JSON.parse(readFileSync(configPath, "utf8"));
    } catch {
      console.warn("Warning: existing .cursor/mcp.json is invalid — overwriting");
    }
  }

  const servers = (config.mcpServers as Record<string, unknown>) ?? {};
  servers["logreduce"] = { command: "npx", args: ["logreduce-mcp"] };
  config.mcpServers = servers;

  writeFileSync(configPath, JSON.stringify(config, null, 2) + "\n");
  console.log(`✓ Cursor config written: ${configPath}`);
  console.log("Reload Cursor to activate the logreduce MCP server.");
}

function installCopilot(): void {
  console.log(
    [
      "GitHub Copilot (VS Code) setup:",
      "",
      "1. Open Command Palette → \"MCP: Open User Configuration\"",
      "2. Add this entry under \"servers\":",
      "",
      JSON.stringify(
        { servers: { logreduce: { command: "npx", args: ["logreduce-mcp"] } } },
        null,
        2
      ),
      "",
      "3. Add a workspace instruction in .github/copilot-instructions.md:",
      "   \"When a user asks about a log file or shares log content, call the",
      "   reduce_log MCP tool first to compress the log before analysing it.\"",
      "",
      "4. Switch to Agent mode in Copilot Chat to use MCP tools.",
    ].join("\n")
  );
}
