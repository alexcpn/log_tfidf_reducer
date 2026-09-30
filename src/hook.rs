//! `UserPromptSubmit` hook shared by Claude Code, Codex, and VS Code Copilot:
//! detects a large log file path in the prompt and injects its reduced form
//! as `hookSpecificOutput.additionalContext`, so the model works from the
//! reduced log instead of reading the raw file. None of these hosts allow a
//! hook to rewrite the prompt itself, so logs pasted inline are left alone —
//! injecting a reduced copy next to them would only add tokens. Always
//! fail-open — any error or non-detection prints `{}` (pass-through).

use crate::{reduce, ReduceConfig};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use std::io::{self, Read};

const MIN_LINES: usize = 500;

static FILE_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)(?:^|\s)((?:/|\.\.?/|~/|[A-Z]:\\)[\w./\\-]+)").unwrap());

/// A log file referenced in the prompt, with its full contents.
#[derive(Debug)]
struct DetectedLog {
    path: String,
    content: String,
}

fn detect_file_path(prompt: &str) -> Option<DetectedLog> {
    for caps in FILE_PATH_RE.captures_iter(prompt) {
        let candidate = caps.get(1)?.as_str().trim();
        let Ok(content) = std::fs::read_to_string(candidate) else {
            continue;
        };
        if content.split('\n').count() >= MIN_LINES {
            return Some(DetectedLog {
                path: candidate.to_string(),
                content,
            });
        }
    }
    None
}

fn build_context(detected: &DetectedLog, reduced: &str) -> String {
    let line_count = detected.content.lines().count();
    format!(
        "logreduce hook: `{}` is a {line_count}-line log. Its reduced form is below — \
         work from this instead of reading the raw file. For more detail, run \
         `logreduce {} --context 5` or raise `--budget`.\n\
         --- LOG (reduced from {line_count} lines) ---\n{reduced}---\n",
        detected.path, detected.path
    )
}

#[derive(Deserialize)]
struct HookInput {
    prompt: Option<String>,
}

/// Run the hook: read `{ prompt, ... }` JSON from stdin, write
/// `{ "hookSpecificOutput": { "hookEventName": "UserPromptSubmit",
/// "additionalContext": "..." } }` or `{}` to stdout. Never fails loudly.
pub fn run() {
    let mut raw = String::new();
    if io::stdin().read_to_string(&mut raw).is_err() {
        print!("{{}}");
        return;
    }

    let prompt = match serde_json::from_str::<HookInput>(&raw) {
        Ok(HookInput { prompt: Some(p) }) if !p.is_empty() => p,
        _ => {
            print!("{{}}");
            return;
        }
    };

    let Some(detected) = detect_file_path(&prompt) else {
        print!("{{}}");
        return;
    };

    let config = ReduceConfig {
        budget_tokens: 8000,
        show_stats: true,
        ..ReduceConfig::default()
    };
    let result = reduce(&config, &detected.content);
    let reduced = format!("{}\n{}", result.header, result.body);

    let output = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": build_context(&detected, &reduced),
        }
    });
    print!("{output}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_lines(n: usize) -> String {
        (0..n)
            .map(|i| format!("2024-01-01 12:00:{i:02} INFO message number {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn detects_file_path_when_large_enough() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("logreduce-hook-test-{}.log", std::process::id()));
        std::fs::write(&path, log_lines(600)).unwrap();

        let prompt = format!("Please look at {}", path.display());
        let detected = detect_file_path(&prompt).expect("should detect file path");
        assert_eq!(detected.path, path.display().to_string());
        assert_eq!(detected.content.split('\n').count(), 600);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn ignores_small_file_path() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("logreduce-hook-small-{}.log", std::process::id()));
        std::fs::write(&path, log_lines(5)).unwrap();

        let prompt = format!("Please look at {}", path.display());
        assert!(detect_file_path(&prompt).is_none());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn ignores_inline_log_without_path() {
        let prompt = format!("What's wrong?\n{}", log_lines(600));
        assert!(detect_file_path(&prompt).is_none());
    }

    #[test]
    fn context_names_file_and_wraps_reduced_log() {
        let detected = DetectedLog {
            path: "/var/log/app.log".to_string(),
            content: "line1\nline2".to_string(),
        };
        let out = build_context(&detected, "reduced body\n");
        assert!(out.contains("`/var/log/app.log` is a 2-line log"));
        assert!(out.contains("--- LOG (reduced from 2 lines) ---\nreduced body\n---\n"));
    }
}
