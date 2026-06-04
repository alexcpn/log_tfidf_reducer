import { existsSync, readFileSync } from "fs";
import type { LogDetectionResult } from "./types.js";

const MIN_LINES = 500;
const LOG_LINE_RATIO = 0.7; // 70% of lines must match the log pattern

// Matches: timestamps (ISO, syslog, common log formats) or severity keywords
const LOG_LINE_RE =
  /\d{2}[:/]\d{2}|^\d{4}-\d{2}-\d{2}|\b(ERROR|WARN|INFO|DEBUG|FATAL|CRITICAL)\b/im;

// Matches an absolute or relative file path
const FILE_PATH_RE = /(?:^|\s)((?:\/|\.\.?\/|~\/|[A-Z]:\\)[\w./\\-]+)/gm;

/**
 * Detect whether a prompt contains a log file path or inline log content.
 */
export function detectLog(prompt: string): LogDetectionResult {
  // 1. Check for file path references
  const pathResult = detectFilePath(prompt);
  if (pathResult.detected) return pathResult;

  // 2. Check for inline log block
  return detectInlineBlock(prompt);
}

function detectFilePath(prompt: string): LogDetectionResult {
  FILE_PATH_RE.lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = FILE_PATH_RE.exec(prompt)) !== null) {
    const candidate = match[1].trim();
    if (!existsSync(candidate)) continue;

    // Count lines to confirm it's large enough
    try {
      const content = readFileSync(candidate, "utf8");
      const lineCount = content.split("\n").length;
      if (lineCount < MIN_LINES) continue;

      return {
        detected: true,
        type: "path",
        content: candidate,
        startIndex: match.index + (match[0].length - match[1].length),
        endIndex: match.index + match[0].length,
      };
    } catch {
      continue;
    }
  }

  return { detected: false, type: null, content: null, startIndex: null, endIndex: null };
}

function detectInlineBlock(prompt: string): LogDetectionResult {
  const lines = prompt.split("\n");
  if (lines.length < MIN_LINES) {
    return { detected: false, type: null, content: null, startIndex: null, endIndex: null };
  }

  // Find the longest run of log-looking lines
  let bestStart = -1;
  let bestEnd = -1;
  let runStart = -1;
  let runLength = 0;

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
  if (runSize < MIN_LINES) {
    return { detected: false, type: null, content: null, startIndex: null, endIndex: null };
  }

  // Check the ratio of matching lines within the run
  const runLines = lines.slice(bestStart, bestEnd);
  const matchingCount = runLines.filter((l) => LOG_LINE_RE.test(l)).length;
  if (matchingCount / runSize < LOG_LINE_RATIO) {
    return { detected: false, type: null, content: null, startIndex: null, endIndex: null };
  }

  // Compute character offsets
  const charsBefore = lines.slice(0, bestStart).join("\n").length + (bestStart > 0 ? 1 : 0);
  const block = runLines.join("\n");

  return {
    detected: true,
    type: "inline",
    content: block,
    startIndex: charsBefore,
    endIndex: charsBefore + block.length,
  };
}
