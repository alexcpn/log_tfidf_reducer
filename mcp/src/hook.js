#!/usr/bin/env node
/**
 * Claude Code UserPromptSubmit hook (ES module).
 *
 * Reads JSON from stdin: { session_id, prompt, ... }
 * Writes JSON to stdout: { prompt: "<rewritten>" } or {}
 *
 * Detects log file paths or large inline log blocks in the prompt and
 * reduces them with logreduce before the model sees the prompt.
 * Always fail-open: any error produces {} (pass-through).
 */

import { execFileSync } from "child_process";
import { existsSync, readFileSync } from "fs";
import { join } from "path";
import { homedir } from "os";

const TIMEOUT_MS = 5000;
const MIN_LINES = 500;
const LOG_LINE_RE =
  /\d{2}[:/]\d{2}|^\d{4}-\d{2}-\d{2}|\b(ERROR|WARN|INFO|DEBUG|FATAL|CRITICAL)\b/im;
const FILE_PATH_RE = /(?:^|\s)((?:\/|\.\.?\/|~\/)[^\s'"]+)/gm;

const IS_WIN = process.platform === "win32";
const DOWNLOADED_BIN = join(
  homedir(), ".logreduce", "bin",
  IS_WIN ? "logreduce.exe" : "logreduce"
);

function findBinary() {
  // 1. Check PATH
  try {
    const result = execFileSync(IS_WIN ? "where" : "which", ["logreduce"], {
      encoding: "utf8", stdio: ["pipe", "pipe", "pipe"],
    }).trim().split(/\r?\n/)[0];
    if (result) return result;
  } catch { /* not on PATH */ }

  // 2. Check postinstall download location
  if (existsSync(DOWNLOADED_BIN)) return DOWNLOADED_BIN;

  return null;
}

function detectFilePath(prompt) {
  FILE_PATH_RE.lastIndex = 0;
  let match;
  while ((match = FILE_PATH_RE.exec(prompt)) !== null) {
    const candidate = match[1].trim();
    if (!existsSync(candidate)) continue;
    try {
      const content = readFileSync(candidate, "utf8");
      const lineCount = content.split("\n").length;
      if (lineCount >= MIN_LINES) {
        return {
          type: "path",
          content: candidate,
          start: match.index + match[0].indexOf(match[1]),
          end: match.index + match[0].length,
        };
      }
    } catch {
      continue;
    }
  }
  return null;
}

function detectInlineBlock(prompt) {
  const lines = prompt.split("\n");
  if (lines.length < MIN_LINES) return null;

  let bestStart = -1, bestEnd = -1, runStart = -1, runLength = 0;
  for (let i = 0; i < lines.length; i++) {
    if (LOG_LINE_RE.test(lines[i])) {
      if (runStart === -1) runStart = i;
      runLength++;
    } else {
      if (runLength > bestEnd - bestStart) {
        bestStart = runStart;
        bestEnd = runStart + runLength;
      }
      runStart = -1;
      runLength = 0;
    }
  }
  if (runLength > bestEnd - bestStart) {
    bestStart = runStart;
    bestEnd = runStart + runLength;
  }

  const runSize = bestEnd - bestStart;
  if (runSize < MIN_LINES) return null;

  const runLines = lines.slice(bestStart, bestEnd);
  const matchCount = runLines.filter((l) => LOG_LINE_RE.test(l)).length;
  if (matchCount / runSize < 0.7) return null;

  const charsBefore =
    lines.slice(0, bestStart).join("\n").length + (bestStart > 0 ? 1 : 0);
  const block = runLines.join("\n");
  return { type: "inline", content: block, start: charsBefore, end: charsBefore + block.length };
}

function reduce(detected, binary) {
  try {
    if (detected.type === "path") {
      return execFileSync(
        binary,
        [detected.content, "--budget", "8000", "--stats"],
        { encoding: "utf8", timeout: TIMEOUT_MS }
      );
    } else {
      return execFileSync(binary, ["--budget", "8000", "--stats"], {
        input: detected.content,
        encoding: "utf8",
        timeout: TIMEOUT_MS,
      });
    }
  } catch {
    return null;
  }
}

function rewritePrompt(prompt, detected, reduced) {
  const lineCount =
    detected.type === "inline"
      ? detected.content.split("\n").length
      : "?";
  const replacement =
    `\n--- LOG (reduced from ${lineCount} lines) ---\n` +
    reduced +
    `---\n`;
  return prompt.slice(0, detected.start) + replacement + prompt.slice(detected.end);
}

function main() {
  let input;
  try {
    const raw = readFileSync(0, "utf8"); // fd 0 = stdin
    input = JSON.parse(raw);
  } catch {
    process.stdout.write("{}");
    return;
  }

  const prompt = input.prompt;
  if (typeof prompt !== "string" || prompt.length === 0) {
    process.stdout.write("{}");
    return;
  }

  const binary = findBinary();
  if (!binary) {
    // logreduce not on PATH — fail open, log hint to stderr
    process.stderr.write(
      "logreduce-mcp hook: logreduce binary not found on PATH.\n" +
      "Download from: https://github.com/alexcpn/log_tfidf_reducer/releases/latest\n"
    );
    process.stdout.write("{}");
    return;
  }

  const detected = detectFilePath(prompt) || detectInlineBlock(prompt);
  if (!detected) {
    process.stdout.write("{}");
    return;
  }

  const reduced = reduce(detected, binary);
  if (!reduced) {
    process.stdout.write("{}");
    return;
  }

  const newPrompt = rewritePrompt(prompt, detected, reduced);
  process.stdout.write(JSON.stringify({ prompt: newPrompt }));
}

main();
