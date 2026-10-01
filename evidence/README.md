# Real-World Evidence: `logreduce` on GitHub Issues

This directory contains real-world evaluation results and evidence of [`logreduce`](../README.md) tested on actual bug reports and crash logs attached to issues in open-source GitHub repositories.

---

## Benchmark Summary

| Repository & Issue | Ecosystem / Domain | Raw Lines | Reduced Lines | Line Reduction | Token Reduction | Latency | Root Cause Preserved? |
|---|---|---|---|---|---|---|:---:|
| [**open-webui/open-webui #19215**](https://github.com/open-webui/open-webui/issues/19215) | Docker / Python FastAPI Web Server | 30,003 | 2,622 | **-91.3%** | **-73.4%** (1.4M → 377k) | 1.86 s | **Yes** |
| [**koreader/koreader #14632**](https://github.com/koreader/koreader/issues/14632) | Embedded Device / Lua & C Crash Loop | 1,885 | 274 | **-85.5%** | **-71.6%** (21.7k → 6.2k) | 0.47 s | **Yes** |
| [**aws/aws-toolkit-visual-studio #625**](https://github.com/aws/aws-toolkit-visual-studio/issues/625) | Windows / Visual Studio VSIX Installer | 947 | 657 | **-30.6%** | **-23.5%** (33.5k → 25.6k) | 0.49 s | **Yes** |
| [**Virtuoel/Pehkui #614**](https://github.com/Virtuoel/Pehkui/issues/614) | Java / Gradle CI Build Failure | 817 | 660 | **-19.2%** | **-19.7%** (39.1k → 31.4k) | 0.52 s | **Yes** |
| [**Fuzss/diagonal-fences #57**](https://github.com/Fuzss/diagonal-fences/issues/57) | Java Fabric / Server Mod Crash Loop | 13,668 | 11,987 | **-12.3%** | **-5.3%** (901k → 853k) | 2.51 s | **Yes** |

---

## Detailed Findings per Issue

### 1. `open-webui/open-webui #19215` — Dockerized LLM Web Application Server
- **Problem**: User experienced intermittent "Server Connection Failed" errors in admin image generation settings.
- **Log Profile**: 30,003 lines (5.7 MB, ~1.42 million tokens).
- **Result**: Reduced to 2,622 lines (1,848 scored lines), eliminating **91.3% of raw lines**.
- **Noise Filtered**:
  - Over **4,200 repeated web bot scanner requests** (`GET /wp-login.php HTTP/1.1`) collapsed to `(×4211)`.
  - Over **3,300 status / heartbeat polling requests** (`GET /api/v1/chats`, `GET /_app/version.json`) collapsed.
  - Redundant container initialization and pip dependency output collapsed.
- **Critical Diagnostics Isolated**:
  - `Anthropic API (HTTP 400)`: `"Your credit balance is too low to access the Anthropic API."`
  - `OpenAI API (HTTP 403)`: `"Attempt to decode JSON with unexpected mimetype: url=https://api.openai.com/v1/models"`
  - `Container DNS Resolution`: `"HTTPConnectionPool(host='tika', port=9998): Failed to resolve 'tika' ([Errno -2] Name or service not known)"`
  - `Missing Python Dependency`: `"No module named 'chardet'"`
  - `Middleware Exception`: `"'JSONResponse' object is not subscriptable"`

### 2. `koreader/koreader #14632` — Embedded Device Crash Loop
- **Problem**: E-reader UI orientation flipped, touch input broke, app entered restart loop.
- **Log Profile**: 1,885 lines (88 KB, ~21.7k tokens).
- **Result**: Reduced to 274 lines (240 scored lines), a **-85.5% reduction** in 0.47s.
- **Critical Diagnostics Isolated**:
  - Crash count identified: exactly 19 launch cycles (`launching... (×19)`).
  - Termination signal: `lipc-wait-event was killed by signal 15`.
  - Application exit code: `Preparing to quit UIManager with exit code: 85`.

### 3. `aws/aws-toolkit-visual-studio #625` — Dev Tools Extension Installer
- **Problem**: Extension installation failed silently in Visual Studio 2026.
- **Log Profile**: 947 lines (136 KB, ~33.5k tokens).
- **Result**: Reduced to 657 lines in 0.49s.
- **Critical Diagnostics Isolated**:
  - Certificate error: `Certificate is invalid: AWSToolkitPackage.v17.vsix`
  - Argument error: `Activity threw exception System.ArgumentNullException: Value cannot be null. Parameter name: uri`
  - Package failure: `Microsoft.VisualStudio.Setup.PackageFailureException: Package failed to download. Rolling back.`
  - Secret & identifier sanitization: Windows package hashes and machine GUIDs redacted (`<REDACTED:hi>`).

### 4. `Virtuoel/Pehkui #614` — Java Gradle Build Failure
- **Problem**: Build crashed due to Mixin parameter mismatch.
- **Log Profile**: 817 lines (158 KB, ~39.1k tokens).
- **Result**: Reduced to 660 lines in 0.52s.
- **Critical Diagnostics Isolated**:
  - Multi-line stack continuations (`Caused by: ...`, `at ...`) were automatically unified.
  - Pinpointed exact method signature mismatch:
    `InvalidInjectionException: @ModifyExpressionValue expression value modifier method ... pehkui$canUse$xOffset from pehkui.mixins.json:compat1204minus.ScreenHandlerMixin has an invalid signature. Found unexpected argument type Block at index 1, expected Player.`

### 5. `Fuzss/diagonal-fences #57` — Template Guarantee Behavior on Unmasked Unique Paths
- **Problem**: Fabric mod server log with 13,668 lines (3.6 MB).
- **Behavior Observed**:
  - The log contained **11,068 warning lines**, each referencing a distinct file path in a custom resource pack (`Non [a-z0-9_.-] character in namespace .CoffeeGUIAssets in pack C:\...\resourcepacks\...`).
  - Because each file path was unique and not masked by standard regex patterns, each produced a unique template ID.
  - `logreduce`'s **Template Guarantee (FR-004)** in `src/select.rs` ensures that every distinct template receives at least one representative line, accepting budget overage to guarantee zero dropped events.
- **Design Insight**:
  - For logs with thousands of unmasked path-unique warnings, `logreduce` prioritizes completeness over blind truncation. Users wanting hard line caps can use `--max-lines`.

---

## Directory Contents

- [`cases.json`](cases.json) — Manifest of tested GitHub issues, original URLs, and metadata.
- [`eval_results.json`](eval_results.json) — Full numerical benchmark data (line counts, byte counts, percentages, runtimes).
- [`run_eval.py`](run_eval.py) — Standalone reproduction script to download raw logs and run `logreduce`.
- [`reduced_outputs/`](reduced_outputs/) — The reduced log outputs generated by `logreduce` for each issue.
