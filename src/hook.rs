//! Claude Code `UserPromptSubmit` hook: detects log file paths or large
//! inline log blocks in a prompt and reduces them with `logreduce` before
//! the model sees the prompt. Always fail-open — any error or non-detection
//! prints `{}` (pass-through).

use crate::{reduce, ReduceConfig};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{self, Read};

const MIN_LINES: usize = 500;
const LOG_LINE_RATIO: f64 = 0.7;

static LOG_LINE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)\d{2}[:/]\d{2}|^\d{4}-\d{2}-\d{2}|\b(ERROR|WARN|INFO|DEBUG|FATAL|CRITICAL)\b")
        .unwrap()
});

static FILE_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)(?:^|\s)((?:/|\.\.?/|~/|[A-Z]:\\)[\w./\\-]+)").unwrap());

#[derive(Debug, PartialEq)]
enum DetectionKind {
    Path,
    Inline,
}

#[derive(Debug)]
struct Detection {
    kind: DetectionKind,
    /// File path (for `Path`) or the inline block text (for `Inline`).
    content: String,
    start: usize,
    end: usize,
}

fn detect_file_path(prompt: &str) -> Option<Detection> {
    for caps in FILE_PATH_RE.captures_iter(prompt) {
        let group = caps.get(1)?;
        let candidate = group.as_str().trim();
        let path = std::path::Path::new(candidate);
        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        if content.split('\n').count() >= MIN_LINES {
            let whole = caps.get(0).unwrap();
            return Some(Detection {
                kind: DetectionKind::Path,
                content: candidate.to_string(),
                start: group.start(),
                end: whole.end(),
            });
        }
    }
    None
}

fn detect_inline_block(prompt: &str) -> Option<Detection> {
    let lines: Vec<&str> = prompt.split('\n').collect();
    if lines.len() < MIN_LINES {
        return None;
    }

    let mut best_start: isize = -1;
    let mut best_end: isize = -1;
    let mut run_start: isize = -1;
    let mut run_length: isize = 0;

    for (i, line) in lines.iter().enumerate() {
        if LOG_LINE_RE.is_match(line) {
            if run_start == -1 {
                run_start = i as isize;
            }
            run_length += 1;
        } else {
            if run_length > best_end - best_start {
                best_start = run_start;
                best_end = run_start + run_length;
            }
            run_start = -1;
            run_length = 0;
        }
    }
    if run_length > best_end - best_start {
        best_start = run_start;
        best_end = run_start + run_length;
    }

    let run_size = best_end - best_start;
    if run_size < MIN_LINES as isize {
        return None;
    }

    let (start, end) = (best_start as usize, best_end as usize);
    let run_lines = &lines[start..end];
    let match_count = run_lines.iter().filter(|l| LOG_LINE_RE.is_match(l)).count();
    if (match_count as f64) / (run_size as f64) < LOG_LINE_RATIO {
        return None;
    }

    let chars_before = lines[..start].join("\n").len() + usize::from(start > 0);
    let block = run_lines.join("\n");
    let block_len = block.len();
    Some(Detection {
        kind: DetectionKind::Inline,
        content: block,
        start: chars_before,
        end: chars_before + block_len,
    })
}

fn detect_log(prompt: &str) -> Option<Detection> {
    detect_file_path(prompt).or_else(|| detect_inline_block(prompt))
}

fn rewrite_prompt(prompt: &str, detected: &Detection, reduced: &str) -> String {
    let line_count = match detected.kind {
        DetectionKind::Inline => detected.content.split('\n').count().to_string(),
        DetectionKind::Path => "?".to_string(),
    };
    let replacement = format!("\n--- LOG (reduced from {line_count} lines) ---\n{reduced}---\n");
    format!(
        "{}{}{}",
        &prompt[..detected.start],
        replacement,
        &prompt[detected.end..]
    )
}

#[derive(Deserialize)]
struct HookInput {
    prompt: Option<String>,
}

#[derive(Serialize)]
struct HookOutput {
    prompt: String,
}

/// Run the hook: read `{ prompt, ... }` JSON from stdin, write
/// `{ "prompt": "<rewritten>" }` or `{}` to stdout. Never fails loudly.
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

    let Some(detected) = detect_log(&prompt) else {
        print!("{{}}");
        return;
    };

    let raw_text = match detected.kind {
        DetectionKind::Path => match std::fs::read_to_string(&detected.content) {
            Ok(s) => s,
            Err(_) => {
                print!("{{}}");
                return;
            }
        },
        DetectionKind::Inline => detected.content.clone(),
    };

    let config = ReduceConfig {
        budget_tokens: 8000,
        show_stats: true,
        ..ReduceConfig::default()
    };
    let result = reduce(&config, &raw_text);
    let reduced = format!("{}\n{}", result.header, result.body);

    let new_prompt = rewrite_prompt(&prompt, &detected, &reduced);
    print!(
        "{}",
        serde_json::to_string(&HookOutput { prompt: new_prompt }).unwrap()
    );
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
    fn detects_inline_block_above_threshold() {
        let prompt = format!(
            "Here is some context.\n\n{}\n\nWhat's wrong?",
            log_lines(600)
        );
        let detected = detect_inline_block(&prompt).expect("should detect inline block");
        assert_eq!(detected.kind, DetectionKind::Inline);
        assert_eq!(detected.content.split('\n').count(), 600);
    }

    #[test]
    fn ignores_short_inline_block() {
        let prompt = format!("Context\n{}\nDone", log_lines(10));
        assert!(detect_inline_block(&prompt).is_none());
    }

    #[test]
    fn ignores_block_below_match_ratio() {
        // Interleave log-looking lines with plain prose so the run never
        // reaches the 70% match ratio within any 500+ line window.
        let mut lines = Vec::new();
        for i in 0..800 {
            if i % 2 == 0 {
                lines.push(format!("2024-01-01 12:00:00 INFO event {i}"));
            } else {
                lines.push(format!(
                    "just some unrelated prose line {i} with no markers"
                ));
            }
        }
        let prompt = lines.join("\n");
        assert!(detect_inline_block(&prompt).is_none());
    }

    #[test]
    fn detects_file_path_when_large_enough() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("logreduce-hook-test-{}.log", std::process::id()));
        std::fs::write(&path, log_lines(600)).unwrap();

        let prompt = format!("Please look at {}", path.display());
        let detected = detect_file_path(&prompt).expect("should detect file path");
        assert_eq!(detected.kind, DetectionKind::Path);
        assert_eq!(detected.content, path.display().to_string());

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
    fn rewrite_prompt_splices_reduced_content() {
        let prompt = "before XXXX after".to_string();
        let detected = Detection {
            kind: DetectionKind::Inline,
            content: "line1\nline2".to_string(),
            start: 7,
            end: 11,
        };
        let out = rewrite_prompt(&prompt, &detected, "reduced body\n");
        assert!(out.starts_with("before \n--- LOG (reduced from 2 lines) ---\n"));
        assert!(out.ends_with(" after"));
    }
}
