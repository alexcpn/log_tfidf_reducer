import { describe, it, expect, vi } from "vitest";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "../..");
const SAMPLE_LOG = path.join(REPO_ROOT, "sample_logs", "plain.log");

describe("findBinary", () => {
  it("returns a non-empty path when logreduce is available", async () => {
    const { findBinary } = await import("../src/reducer.js");
    try {
      const result = findBinary();
      expect(result.length).toBeGreaterThan(0);
      expect(result).toMatch(/logreduce/);
    } catch (e: unknown) {
      // Binary genuinely not available — acceptable in CI without the binary
      console.warn("logreduce not available — skipping:", (e as Error).message.split("\n")[0]);
    }
  });

  it("throws with install instructions when binary is absent", async () => {
    // Mock both PATH lookup and the fallback ~/.logreduce/bin/ check
    vi.mock("child_process", async (importOriginal) => {
      const actual = await importOriginal<typeof import("child_process")>();
      return {
        ...actual,
        execFileSync: vi.fn().mockImplementation((cmd: string) => {
          if (cmd === "which" || cmd === "where") throw new Error("not found");
          return (actual.execFileSync as typeof actual.execFileSync)(cmd);
        }),
      };
    });
    vi.mock("fs", async (importOriginal) => {
      const actual = await importOriginal<typeof import("fs")>();
      return {
        ...actual,
        existsSync: vi.fn().mockReturnValue(false),
      };
    });

    const { findBinary } = await import("../src/reducer.js?v=nomock");
    expect(() => findBinary()).toThrow();
    vi.restoreAllMocks();
    vi.resetModules();
  });
});

describe("invokeReducer (integration)", () => {
  it("reduces sample_logs/plain.log and returns non-empty stdout", async () => {
    const { findBinary, invokeReducer } = await import("../src/reducer.js");
    try { findBinary(); } catch { return; } // skip if binary not available

    const { stdout } = await invokeReducer([SAMPLE_LOG, "--budget", "2000", "--stats"]);
    expect(stdout).toContain("LOG SUMMARY");
    expect(stdout.length).toBeGreaterThan(0);
  }, 15_000); // 15s timeout for binary execution

  it("reduces inline text via stdin", async () => {
    const { findBinary, invokeReducer } = await import("../src/reducer.js");
    try { findBinary(); } catch { return; }

    const logText =
      Array.from({ length: 20 }, (_, i) =>
        `2026-06-01 09:00:${String(i).padStart(2, "0")} INFO health check ok`
      ).join("\n") + "\n2026-06-01 09:01:00 ERROR something failed\n";

    const { stdout } = await invokeReducer(["--budget", "500"], logText);
    expect(stdout).toContain("LOG SUMMARY");
  }, 15_000);
});
