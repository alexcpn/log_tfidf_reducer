import type { ReduceLogOutput, Stats } from "./types.js";

/**
 * Parse logreduce stdout + stderr into a ReduceLogOutput object.
 * stdout: the full reduced log (# LOG SUMMARY header + body)
 * stderr: stats output produced by --stats flag
 */
export function parseOutput(stdout: string, stderr: string): ReduceLogOutput {
  const stats = parseStats(stdout, stderr);
  return { text: stdout, stats };
}

function parseStats(stdout: string, stderr: string): Stats {
  // Parse input/kept lines from stdout header: "N lines → M kept (P%)"
  const summaryMatch = stdout.match(
    /(\d[\d,]*)\s+lines\s+→\s+(\d[\d,]*)\s+kept/
  );
  const inputLines = summaryMatch
    ? parseInt(summaryMatch[1].replace(/,/g, ""), 10)
    : 0;
  const keptLines = summaryMatch
    ? parseInt(summaryMatch[2].replace(/,/g, ""), 10)
    : 0;

  // Parse token counts from stderr stats (produced by --stats flag)
  // Format: "input_tokens: N  output_tokens: M" or similar
  const inputTokensMatch = stderr.match(/input[_\s]tokens?[:\s]+(\d[\d,]*)/i);
  const outputTokensMatch = stderr.match(
    /output[_\s]tokens?[:\s]+(\d[\d,]*)/i
  );

  // Fallback: estimate from character counts (chars / 4 ≈ tokens)
  const inputTokens = inputTokensMatch
    ? parseInt(inputTokensMatch[1].replace(/,/g, ""), 10)
    : Math.ceil(stdout.length / 4);
  const outputTokens = outputTokensMatch
    ? parseInt(outputTokensMatch[1].replace(/,/g, ""), 10)
    : Math.ceil(stdout.length / 4);

  return { input_lines: inputLines, kept_lines: keptLines, input_tokens: inputTokens, output_tokens: outputTokens };
}
