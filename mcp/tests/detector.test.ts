import { describe, it, expect } from "vitest";
import { detectLog } from "../src/detector.js";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "../..");

// Build a large inline log block (600 log lines)
function makeInlineLog(n = 600): string {
  return Array.from(
    { length: n },
    (_, i) => `2026-06-01 09:00:${String(i % 60).padStart(2, "0")} INFO health check ok`
  ).join("\n");
}

describe("detectLog — file path", () => {
  it("detects a path to a file with ≥500 lines", () => {
    // Use crash_loop.log which has 503 lines
    const logPath = path.join(REPO_ROOT, "sample_logs", "crash_loop.log");
    const prompt = `Please analyse this log: ${logPath}`;
    const result = detectLog(prompt);
    expect(result.detected).toBe(true);
    expect(result.type).toBe("path");
    expect(result.content).toBe(logPath);
  });

  it("does not detect a path to a non-existent file", () => {
    const result = detectLog("check /tmp/does-not-exist-at-all.log");
    expect(result.detected).toBe(false);
  });

  it("does not detect a path to a small file", () => {
    // Use plain.log which has 20 lines — well below 500
    const logPath = path.join(REPO_ROOT, "sample_logs", "plain.log");
    const prompt = `check ${logPath}`;
    const result = detectLog(prompt);
    expect(result.detected).toBe(false);
  });
});

describe("detectLog — inline block", () => {
  it("detects a large inline log block", () => {
    const log = makeInlineLog(600);
    const prompt = `What went wrong?\n${log}\nPlease help.`;
    const result = detectLog(prompt);
    expect(result.detected).toBe(true);
    expect(result.type).toBe("inline");
  });

  it("does not detect a short inline block", () => {
    const log = makeInlineLog(100);
    const result = detectLog(`Some context\n${log}`);
    expect(result.detected).toBe(false);
  });

  it("does not detect non-log text even if long", () => {
    const prose = Array.from({ length: 600 }, (_, i) => `Word ${i}`).join("\n");
    const result = detectLog(prose);
    expect(result.detected).toBe(false);
  });
});

describe("detectLog — no-op", () => {
  it("returns detected:false for a normal question", () => {
    const result = detectLog("What is the capital of France?");
    expect(result.detected).toBe(false);
    expect(result.type).toBeNull();
  });
});
