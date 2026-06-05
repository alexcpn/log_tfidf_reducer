#!/usr/bin/env node
/**
 * Postinstall script — downloads the logreduce binary for the current
 * platform from GitHub Releases if it is not already on PATH.
 *
 * Saves to ~/.logreduce/bin/logreduce (or logreduce.exe on Windows).
 * Never fails the npm install — all errors are warnings only.
 */

"use strict";

const https = require("https");
const fs = require("fs");
const path = require("path");
const os = require("os");
const { execFileSync } = require("child_process");

const REPO = "alexcpn/log_tfidf_reducer";
const BIN_DIR = path.join(os.homedir(), ".logreduce", "bin");
const IS_WIN = process.platform === "win32";
const BIN_NAME = IS_WIN ? "logreduce.exe" : "logreduce";
const BIN_DEST = path.join(BIN_DIR, BIN_NAME);

// Platform → release asset name
const PLATFORM_MAP = {
  "linux-x64":    "logreduce-linux-x64",
  "darwin-arm64": "logreduce-darwin-arm64",
  "darwin-x64":   "logreduce-darwin-x64",
  "win32-x64":    "logreduce-win32-x64.exe",
};

function log(msg) {
  console.log(`[logreduce-mcp] ${msg}`);
}

function warn(msg) {
  console.warn(`[logreduce-mcp] WARNING: ${msg}`);
}

function alreadyOnPath() {
  try {
    execFileSync(IS_WIN ? "where" : "which", ["logreduce"], {
      stdio: ["pipe", "pipe", "pipe"],
    });
    return true;
  } catch {
    return false;
  }
}

function alreadyDownloaded() {
  return fs.existsSync(BIN_DEST);
}

function getAssetName() {
  const key = `${process.platform}-${process.arch}`;
  return PLATFORM_MAP[key] || null;
}

function fetchRedirect(url) {
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { "User-Agent": "logreduce-mcp-installer" } }, (res) => {
      if (res.statusCode === 302 || res.statusCode === 301) {
        resolve(res.headers.location);
      } else if (res.statusCode === 200) {
        resolve(url);
      } else {
        reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
    }).on("error", reject);
  });
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const tmp = dest + ".tmp";

    // Clean up any leftover .tmp from a previous failed attempt
    try { fs.unlinkSync(tmp); } catch { /* ignore */ }

    // Follow all redirects first, then stream the final 200 response to disk.
    // GitHub releases always redirect: releases/latest/download → CDN URL.
    // Creating the write stream *after* all redirects avoids the bug where
    // closing and re-using the same stream on redirect produces a 0-byte file.
    const follow = (u, depth) => {
      if (depth > 10) return reject(new Error("Too many redirects"));
      https.get(u, { headers: { "User-Agent": "logreduce-mcp-installer" } }, (res) => {
        if (res.statusCode === 301 || res.statusCode === 302) {
          res.resume(); // drain and discard redirect body
          return follow(res.headers.location, depth + 1);
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode}`));
        }

        const total = parseInt(res.headers["content-length"] || "0", 10);
        let received = 0;
        let lastPct = -1;
        const file = fs.createWriteStream(tmp);

        res.on("data", (chunk) => {
          received += chunk.length;
          if (total > 0) {
            const pct = Math.floor((received / total) * 100);
            if (pct !== lastPct && pct % 10 === 0) {
              process.stdout.write(`\r[logreduce-mcp] Downloading... ${pct}%`);
              lastPct = pct;
            }
          }
        });

        res.pipe(file);

        file.on("finish", () => {
          process.stdout.write("\n");
          file.close(() => {
            fs.rename(tmp, dest, (err) => {
              if (err) reject(err); else resolve();
            });
          });
        });

        file.on("error", (e) => { fs.unlink(tmp, () => {}); reject(e); });
        res.on("error",  (e) => { file.destroy(); fs.unlink(tmp, () => {}); reject(e); });
      }).on("error", (e) => { fs.unlink(tmp, () => {}); reject(e); });
    };

    follow(url, 0);
  });
}

async function main() {
  // Skip if binary is already available on PATH
  if (alreadyOnPath()) {
    log("logreduce already on PATH — skipping download.");
    return;
  }

  // Skip if already downloaded to our bin dir
  if (alreadyDownloaded()) {
    log(`logreduce already downloaded at ${BIN_DEST}`);
    printPathHint();
    return;
  }

  const asset = getAssetName();
  if (!asset) {
    warn(`Unsupported platform: ${process.platform}-${process.arch}`);
    warn("Build from source: cargo install logreduce  (https://rustup.rs)");
    return;
  }

  // Resolve latest release download URL
  const latestUrl = `https://github.com/${REPO}/releases/latest/download/${asset}`;

  log(`Downloading logreduce for ${process.platform}-${process.arch}...`);
  log(`Source: ${latestUrl}`);

  try {
    fs.mkdirSync(BIN_DIR, { recursive: true });
    await download(latestUrl, BIN_DEST);

    // Make executable on Unix
    if (!IS_WIN) {
      fs.chmodSync(BIN_DEST, 0o755);
    }

    log(`Saved to: ${BIN_DEST}`);
    printPathHint();
  } catch (err) {
    warn(`Download failed: ${err.message}`);
    warn("You can install logreduce manually:");
    warn(`  Download: https://github.com/${REPO}/releases/latest`);
    warn("  Or build from source: cargo install logreduce");
    // Exit 0 — never fail the npm install
  }
}

function printPathHint() {
  if (alreadyOnPath()) return; // Already found it

  log("");
  log("──────────────────────────────────────────────");
  if (IS_WIN) {
    log(`Add logreduce to your PATH:`);
    log(`  $env:PATH += ";${BIN_DIR}"`);
    log(`  Or permanently via System → Environment Variables`);
  } else {
    log(`Add logreduce to your PATH by adding this line to ~/.bashrc or ~/.zshrc:`);
    log(`  export PATH="${BIN_DIR}:$PATH"`);
    log(`Then run: source ~/.bashrc`);
  }
  log("──────────────────────────────────────────────");
  log("");
}

main().catch((err) => {
  warn(`Unexpected error: ${err.message}`);
  // Always exit 0 — never block npm install
  process.exit(0);
});
