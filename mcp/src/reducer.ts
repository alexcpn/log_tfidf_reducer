import { execFile, execFileSync } from "child_process";
import { existsSync } from "fs";
import { join } from "path";
import { homedir } from "os";
import { promisify } from "util";

const execFileAsync = promisify(execFile);

const IS_WIN = process.platform === "win32";
const BIN_NAME = IS_WIN ? "logreduce.exe" : "logreduce";
/** Location where the postinstall script saves the downloaded binary. */
const DOWNLOADED_BIN = join(homedir(), ".logreduce", "bin", BIN_NAME);

/** Locate the logreduce binary — checks PATH first, then ~/.logreduce/bin/. */
export function findBinary(): string {
  // 1. Check PATH
  try {
    const result = execFileSync(IS_WIN ? "where" : "which", ["logreduce"], {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    }).trim().split(/\r?\n/)[0];
    if (result) return result;
  } catch {
    // not on PATH
  }

  // 2. Check postinstall download location (~/.logreduce/bin/)
  if (existsSync(DOWNLOADED_BIN)) {
    return DOWNLOADED_BIN;
  }

  throw new Error(
    "logreduce not found on PATH or ~/.logreduce/bin/.\n\n" +
      "Option 1 — Re-run npm install to auto-download:\n" +
      "  npm install -g logreduce-mcp\n\n" +
      "Option 2 — Download a pre-built binary manually:\n" +
      "  https://github.com/alexcpn/log_tfidf_reducer/releases/latest\n\n" +
      "Option 3 — Build from source (requires Rust):\n" +
      "  cargo install logreduce"
  );
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
