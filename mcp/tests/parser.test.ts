import { describe, it, expect } from "vitest";
import { parseOutput } from "../src/parser.js";

const SAMPLE_STDOUT = `# LOG SUMMARY  |  1000 lines → 42 kept (4%)
# levels: FATAL 0  ERROR 3  WARN 5  INFO 992  OTHER 0
# top templates: "<TS> INFO health check ok" ×800
# Note: token counts use tiktoken cl100k_base

2026-06-01 09:00:01 ERROR something went wrong
… 958 lines omitted …
2026-06-01 09:01:00 INFO service starting
`;

const SAMPLE_STDERR = `input_tokens: 12800
output_tokens: 420
`;

describe("parseOutput", () => {
  it("extracts input_lines and kept_lines from the summary header", () => {
    const result = parseOutput(SAMPLE_STDOUT, SAMPLE_STDERR);
    expect(result.stats.input_lines).toBe(1000);
    expect(result.stats.kept_lines).toBe(42);
  });

  it("extracts token counts from stderr", () => {
    const result = parseOutput(SAMPLE_STDOUT, SAMPLE_STDERR);
    expect(result.stats.input_tokens).toBe(12800);
    expect(result.stats.output_tokens).toBe(420);
  });

  it("returns the full stdout as text", () => {
    const result = parseOutput(SAMPLE_STDOUT, SAMPLE_STDERR);
    expect(result.text).toBe(SAMPLE_STDOUT);
  });

  it("returns zero stats for malformed header without throwing", () => {
    const result = parseOutput("no header here\nsome content", "");
    expect(result.stats.input_lines).toBe(0);
    expect(result.stats.kept_lines).toBe(0);
    expect(result.text).toBe("no header here\nsome content");
  });

  it("handles empty log output gracefully", () => {
    const result = parseOutput("# LOG SUMMARY  |  0 lines → 0 kept (0%)\n", "");
    expect(result.stats.input_lines).toBe(0);
    expect(result.stats.kept_lines).toBe(0);
  });

  it("falls back to char-count estimate when stderr has no token info", () => {
    const result = parseOutput(SAMPLE_STDOUT, "");
    expect(result.stats.input_tokens).toBeGreaterThan(0);
    expect(result.stats.output_tokens).toBeGreaterThan(0);
  });
});
