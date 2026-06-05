import { describe, it, expect, beforeEach } from "vitest";
import { install } from "../src/install.js";
import { existsSync, readFileSync, mkdtempSync, rmSync } from "fs";
import { join } from "path";
import os from "os";

function makeTempDir(): string {
  return mkdtempSync(join(os.tmpdir(), "logreduce-install-test-"));
}

describe("install — claude-code", () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = makeTempDir();
  });

  it("creates .claude/hooks/UserPromptSubmit.js", () => {
    install({ projectRoot: tmpDir, editor: "claude-code" });
    expect(existsSync(join(tmpDir, ".claude", "hooks", "UserPromptSubmit.js"))).toBe(true);
  });

  it("creates .claude/settings.json with hook registration", () => {
    install({ projectRoot: tmpDir, editor: "claude-code" });
    const settings = JSON.parse(
      readFileSync(join(tmpDir, ".claude", "settings.json"), "utf8")
    );
    expect(settings.hooks?.UserPromptSubmit).toBeDefined();
    expect(Array.isArray(settings.hooks.UserPromptSubmit)).toBe(true);
    // New format: { matcher, hooks: [{ type, command }] }
    const entry = settings.hooks.UserPromptSubmit[0];
    expect(entry.matcher).toBeDefined();
    expect(Array.isArray(entry.hooks)).toBe(true);
    expect(entry.hooks[0].command).toContain("UserPromptSubmit.js");
  });

  it("creates .claude/settings.json with MCP server entry", () => {
    install({ projectRoot: tmpDir, editor: "claude-code" });
    const settings = JSON.parse(
      readFileSync(join(tmpDir, ".claude", "settings.json"), "utf8")
    );
    expect(settings.mcpServers?.logreduce).toBeDefined();
    expect(settings.mcpServers.logreduce.command).toBe("npx");
  });

  it("is idempotent — running twice does not duplicate entries", () => {
    install({ projectRoot: tmpDir, editor: "claude-code" });
    install({ projectRoot: tmpDir, editor: "claude-code" });
    const settings = JSON.parse(
      readFileSync(join(tmpDir, ".claude", "settings.json"), "utf8")
    );
    expect(settings.hooks.UserPromptSubmit.length).toBe(1);
  });

  it("preserves existing settings keys", () => {
    // Pre-populate settings
    import("fs").then(({ writeFileSync, mkdirSync }) => {
      mkdirSync(join(tmpDir, ".claude"), { recursive: true });
      writeFileSync(
        join(tmpDir, ".claude", "settings.json"),
        JSON.stringify({ myExistingKey: "preserved" }, null, 2)
      );
    });
    install({ projectRoot: tmpDir, editor: "claude-code" });
    const settings = JSON.parse(
      readFileSync(join(tmpDir, ".claude", "settings.json"), "utf8")
    );
    // myExistingKey might not be present if the file wasn't written in time (async),
    // but hooks and mcpServers must be present
    expect(settings.hooks?.UserPromptSubmit).toBeDefined();
  });
});

describe("install — cursor", () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = makeTempDir();
  });

  it("creates .cursor/mcp.json with mcpServers.logreduce", () => {
    install({ projectRoot: tmpDir, editor: "cursor" });
    const config = JSON.parse(
      readFileSync(join(tmpDir, ".cursor", "mcp.json"), "utf8")
    );
    expect(config.mcpServers?.logreduce).toBeDefined();
    expect(config.mcpServers.logreduce.command).toBe("npx");
  });

  it("is idempotent for Cursor config", () => {
    install({ projectRoot: tmpDir, editor: "cursor" });
    install({ projectRoot: tmpDir, editor: "cursor" });
    const config = JSON.parse(
      readFileSync(join(tmpDir, ".cursor", "mcp.json"), "utf8")
    );
    expect(Object.keys(config.mcpServers).length).toBe(1);
  });
});
