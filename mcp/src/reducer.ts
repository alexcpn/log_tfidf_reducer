import { execFile, execFileSync } from "child_process";
import { promisify } from "util";

const execFileAsync = promisify(execFile);

/** Locate the logreduce binary on PATH. Throws with install instructions if not found. */
export function findBinary(): string {
  try {
    const result = execFileSync("which", ["logreduce"], {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    }).trim();
    if (!result) throw new Error("empty");
    return result;
  } catch {
    // On Windows, try 'where'
    try {
      const result = execFileSync("where", ["logreduce"], {
        encoding: "utf8",
        stdio: ["pipe", "pipe", "pipe"],
      })
        .trim()
        .split(/\r?\n/)[0];
      if (result) return result;
    } catch {
      // fall through
    }
    throw new Error(
      "logreduce not found on PATH.\n" +
        "Install with: cargo install logreduce\n" +
        "Then ensure ~/.cargo/bin is on your PATH."
    );
  }
}

export interface InvokeResult {
  stdout: string;
  stderr: string;
}

/**
 * Invoke the logreduce binary asynchronously.
 * @param args  CLI arguments (not including the binary name)
 * @param input Optional stdin text (used when reducing inline log text)
 * @param timeoutMs Timeout in milliseconds (default 30000)
 */
export async function invokeReducer(
  args: string[],
  input?: string,
  timeoutMs = 30_000
): Promise<InvokeResult> {
  const binary = findBinary();

  const opts: Parameters<typeof execFileAsync>[2] = {
    timeout: timeoutMs,
    maxBuffer: 100 * 1024 * 1024, // 100 MB
    encoding: "utf8" as const,
  };
  if (input !== undefined) {
    (opts as Record<string, unknown>).input = input;
  }

  const { stdout, stderr } = await execFileAsync(binary, args, opts);
  return { stdout: stdout as string, stderr: stderr as string };
}
