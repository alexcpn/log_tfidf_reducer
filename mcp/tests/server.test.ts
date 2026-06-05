import { describe, it, expect } from "vitest";
import { buildArgs, validateInput } from "../src/server.js";

// Unit tests for argument building and input validation.
// Full MCP integration tests require a running server; these cover the logic.

describe("validateInput", () => {
  it("accepts path-only input", () => {
    expect(() => validateInput({ path: "/tmp/app.log" })).not.toThrow();
  });

  it("accepts text-only input", () => {
    expect(() => validateInput({ text: "2026-01-01 INFO ok" })).not.toThrow();
  });

  it("throws when both path and text provided", () => {
    expect(() =>
      validateInput({ path: "/tmp/app.log", text: "log" })
    ).toThrow("Provide either path or text, not both");
  });

  it("throws when neither path nor text provided", () => {
    expect(() => validateInput({})).toThrow("Provide either path or text");
  });
});

describe("buildArgs", () => {
  it("includes --budget when provided", () => {
    const args = buildArgs({ path: "/tmp/a.log", budget: 4000 });
    expect(args).toContain("--budget");
    expect(args).toContain("4000");
  });

  it("includes --context when provided", () => {
    const args = buildArgs({ text: "log", context: 3 });
    expect(args).toContain("--context");
    expect(args).toContain("3");
  });

  it("includes --no-redact when no_redact is true", () => {
    const args = buildArgs({ text: "log", no_redact: true });
    expect(args).toContain("--no-redact");
  });

  it("does not include --no-redact when false", () => {
    const args = buildArgs({ text: "log", no_redact: false });
    expect(args).not.toContain("--no-redact");
  });

  it("includes --format when provided", () => {
    const args = buildArgs({ text: "log", format: "json" });
    expect(args).toContain("--format");
    expect(args).toContain("json");
  });

  it("always includes --stats", () => {
    const args = buildArgs({ text: "log" });
    expect(args).toContain("--stats");
  });

  it("includes file path as first arg when path provided", () => {
    const args = buildArgs({ path: "/tmp/a.log" });
    expect(args[0]).toBe("/tmp/a.log");
  });
});
