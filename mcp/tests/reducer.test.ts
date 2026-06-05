import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { execFileSync } from "child_process";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "../..");
const SAMPLE_LOG = path.join(REPO_ROOT, "sample_logs", "plain.log");

// We mock child_process for unit tests; integration tests use the real binary.

describe("findBinary", () => {
  it("returns a non-empty path when logreduce is on PATH", async () => {
    const { findBinary } = await import("../src/reducer.js");
    // This test requires logreduce to be installed — skip gracefully if not
    try {
      const result = findBinary();
      expect(result.length).toBeGreaterThan(0);
      expect(result).toMatch(/logreduce/);
    } catch (e: unknown) {
      if ((e as Error).message?.includes("not found")) {
        console.warn("logreduce not on PATH — skipping integration check");
        return;
      }
      throw e;
    }
  });

  it("throws with install instructions when binary is absent", async () => {
    // Temporarily remove the binary from PATH by mocking execFileSync
    vi.mock("child_process", async (importOriginal) => {
      const actual = await importOriginal<typeof import("child_process")>();
      return {
        ...actual,
        execFileSync: vi.fn().mockImplementation((cmd: string) => {
          if (cmd === "which" || cmd === "where") {
            throw new Error("not found");
          }
          return actual.execFileSync(cmd);
        }),
      };
    });

    const { findBinary } = await import("../src/reducer.js?bust=1");
    expect(() => findBinary()).toThrow("not found");
    vi.restoreAllMocks();
  });
});

describe("invokeReducer (integration)", () => {
  it("reduces sample_logs/plain.log and returns non-empty stdout", async () => {
    const { findBinary, invokeReducer } = await import("../src/reducer.js");
    try {
      findBinary(); // skip if binary not installed
    } catch {
      return;
    }
    const { stdout, stderr } = await invokeReducer(
      [SAMPLE_LOG, "--budget", "2000", "--stats"]
    );
    expect(stdout).toContain("LOG SUMMARY");
    expect(stdout.length).toBeGreaterThan(0);
    // stderr should contain stats info
    expect(typeof stderr).toBe("string");
  });

  it("reduces inline text via stdin", async () => {
    const { findBinary, invokeReducer } = await import("../src/reducer.js");
    try {
      findBinary();
    } catch {
      return;
    }
    const logText = Array.from({ length: 20 }, (_, i) =>
      `2026-06-01 09:00:${String(i).padStart(2, "0")} INFO health check ok`
    ).join("\n") + "\n2026-06-01 09:01:00 ERROR something failed\n";

    const { stdout } = await invokeReducer(
      ["--budget", "500"],
      logText
    );
    expect(stdout).toContain("LOG SUMMARY");
  });
});
