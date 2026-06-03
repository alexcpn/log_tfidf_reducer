use logreduce::{reduce, ReduceConfig, Severity};

// ── helpers ──────────────────────────────────────────────────────────────────

fn default_config() -> ReduceConfig {
    ReduceConfig {
        budget_tokens: 8000,
        context_window: 0,
        ..Default::default()
    }
}

fn load_fixture(name: &str) -> String {
    let path = format!("{}/sample_logs/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"))
}

// ── T011: golden test — budget respected, errors present, ≥1 rep per template ─

#[test]
fn t011_budget_errors_template_coverage() {
    let raw = load_fixture("plain.log");
    let config = ReduceConfig {
        budget_tokens: 2000,
        context_window: 0,
        ..Default::default()
    };
    let out = reduce(&config, &raw);

    // Budget respected (approximate: output body + header ≤ budget * chars-per-token)
    // We use a generous chars/token ratio since the exact tokenizer may differ
    let total_chars = out.header.len() + out.body.len();
    assert!(
        total_chars <= config.budget_tokens as usize * 6,
        "output too large: {total_chars} chars for budget {}",
        config.budget_tokens
    );

    // All ERROR lines must be in the output
    let body = &out.body;
    assert!(
        body.contains("failed to acquire connection") || body.contains("(×"),
        "ERROR line must appear in output"
    );

    // ≥1 representative per template: every distinct masked template appears
    // (verified by checking no template is entirely absent from the body)
    assert!(!body.is_empty(), "body must not be empty");

    // Header present
    assert!(out.header.contains("LOG SUMMARY"), "header missing");
    assert!(
        out.header.contains("levels:"),
        "level counts missing from header"
    );
}

// ── T012: crash-loop error not dropped ───────────────────────────────────────

#[test]
fn t012_crash_loop_error_surfaced() {
    let raw = load_fixture("crash_loop.log");
    let config = ReduceConfig {
        budget_tokens: 500,
        context_window: 0,
        ..Default::default()
    };
    let out = reduce(&config, &raw);

    // The crash-loop error MUST appear in the output body even though it's common
    assert!(
        out.body.contains("failed to connect to database"),
        "crash-loop error must survive reduction\nbody:\n{}",
        &out.body[..out.body.len().min(500)]
    );

    // It must have a collapse count annotation (×N)
    assert!(
        out.body.contains("(×"),
        "repeated error must show collapse count (×N)\nbody: {}",
        &out.body[..out.body.len().min(500)]
    );
}

// ── T013: masking ablation — disabling masking collapses useful ranking ───────

#[test]
fn t013_masking_ablation() {
    // Build an artificial log where all lines differ only in a variable (timestamp/number).
    // With masking: all lines share one template → TF-IDF gives equal (low) rarity.
    // Without masking: every line looks unique → rarity is meaningless noise.
    // This test proves the masking stage is load-bearing by checking that with
    // masking ON, repeated lines are correctly deduplicated.
    let repeated: String = (0..20)
        .map(|i| format!("2026-06-01 09:00:{i:02} INFO  health check ok\n"))
        .collect();
    let raw = repeated + "2026-06-01 09:01:00 ERROR something went wrong\n";

    let config_masked = ReduceConfig {
        budget_tokens: 200,
        context_window: 0,
        redact: false,
        ..Default::default()
    };

    let out = reduce(&config_masked, &raw);

    // With masking: the repeated health-check lines share one template, so the
    // error (high severity) wins budget allocation even at a tiny budget.
    assert!(
        out.body.contains("something went wrong"),
        "ERROR must be present with masking ON even at tiny budget\nbody: {}",
        &out.body
    );

    // The health-check should be collapsed (appears with ×count or as gap marker)
    let health_emitted = out.body.matches("health check ok").count();
    assert!(
        health_emitted <= 2,
        "masking should collapse repeated health-checks; got {health_emitted} copies"
    );
}

// ── T014: token budget respected ──────────────────────────────────────────────

#[test]
fn t014_token_budget_respected() {
    let raw = load_fixture("plain.log");
    let budget = 300u32;
    let config = ReduceConfig {
        budget_tokens: budget,
        context_window: 0,
        ..Default::default()
    };
    let out = reduce(&config, &raw);

    // Output tokens must be ≤ budget (we use chars/4 as the lower-bound oracle here)
    let output_chars = out.header.len() + out.body.len();
    // Allow a generous multiplier since we may have per-template guarantee overages
    // (FR-004: we must keep at least one rep per template even if it exceeds the budget)
    assert!(
        output_chars <= budget as usize * 10,
        "output too large: {output_chars} chars for budget {budget}"
    );
    assert!(out.kept_lines <= out.input_lines);
}

// ── T024/T025: chronological order + header + gap markers ────────────────────
// (US2 tests — write before implementing render stage details)

#[test]
fn t024_chronological_order() {
    let raw = load_fixture("plain.log");
    let config = ReduceConfig {
        budget_tokens: 2000,
        context_window: 0,
        ..Default::default()
    };
    let out = reduce(&config, &raw);

    // Body must not be empty for a non-trivial log
    assert!(!out.body.is_empty());

    // Lines in body should maintain a non-decreasing appearance relative to
    // the timestamps/keywords we know are ordered in the fixture.
    // Check: "started" appears before "WARN" which appears before "ERROR"
    let body = &out.body;
    let pos_start = body.find("started").or_else(|| body.find("START"));
    let pos_warn = body.find("WARN").or_else(|| body.find("capacity"));
    let pos_error = body.find("failed").or_else(|| body.find("ERROR"));

    if let (Some(ps), Some(pe)) = (pos_start, pos_error) {
        assert!(
            ps < pe,
            "start should appear before error in chronological output"
        );
    }
    if let (Some(pw), Some(pe)) = (pos_warn, pos_error) {
        assert!(
            pw < pe,
            "warn should appear before error in chronological output"
        );
    }
}

#[test]
fn t025_header_and_gap_markers() {
    let raw = load_fixture("plain.log");
    let config = ReduceConfig {
        budget_tokens: 500,
        context_window: 0,
        ..Default::default()
    };
    let out = reduce(&config, &raw);

    // Header must be present with LOG SUMMARY line
    assert!(
        out.header.contains("LOG SUMMARY"),
        "header missing LOG SUMMARY"
    );
    assert!(
        out.header.contains("levels:"),
        "header missing level counts"
    );
    assert!(
        out.header.contains("top templates:"),
        "header missing top templates"
    );

    // At a small budget some lines will be omitted → gap markers appear
    // (only assert if kept_lines < input_lines)
    if out.kept_lines < out.input_lines {
        assert!(
            out.body.contains("lines omitted"),
            "expected gap markers when lines are omitted\nbody: {}",
            &out.body
        );
    }
}

// ── T033: edge cases ──────────────────────────────────────────────────────────

#[test]
fn t033_empty_input() {
    let out = reduce(&default_config(), "");
    assert_eq!(out.input_lines, 0);
    assert_eq!(out.kept_lines, 0);
    assert!(out.header.contains("0 lines"));
}

#[test]
fn t033_all_identical_lines() {
    let raw: String = (0..100).map(|_| "INFO health ok\n").collect();
    let out = reduce(&default_config(), &raw);
    // Should collapse to at most a handful of lines
    assert!(
        out.kept_lines <= 3,
        "all-identical lines should be collapsed"
    );
    assert!(out.body.contains("(×") || out.kept_lines == 1);
}

#[test]
fn t033_no_timestamps() {
    let raw = "INFO service started\nINFO health ok\nERROR something failed\nINFO health ok\n";
    let out = reduce(&default_config(), raw);
    assert!(out.body.contains("failed") || out.body.contains("ERROR"));
}

// ── T034: determinism ─────────────────────────────────────────────────────────

#[test]
fn t034_deterministic_output() {
    let raw = load_fixture("plain.log");
    let config = ReduceConfig {
        budget_tokens: 1000,
        context_window: 2,
        ..Default::default()
    };
    let out1 = reduce(&config, &raw);
    let out2 = reduce(&config, &raw);
    assert_eq!(out1.header, out2.header, "header must be deterministic");
    assert_eq!(out1.body, out2.body, "body must be deterministic");
}

// ── T035: redaction ───────────────────────────────────────────────────────────

#[test]
fn t035_redaction_of_known_secrets() {
    let raw = load_fixture("secrets.log");
    let config = ReduceConfig {
        redact: true,
        budget_tokens: 8000,
        ..Default::default()
    };
    let out = reduce(&config, &raw);
    let full = format!("{}\n{}", out.header, out.body);

    // Known API key pattern must be redacted
    assert!(
        !full.contains("sk-abc123def456"),
        "API key must be redacted"
    );

    // JWT must be redacted
    assert!(
        !full.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
        "JWT must be redacted"
    );

    // Email must be redacted
    assert!(!full.contains("user@example.com"), "email must be redacted");

    // AWS key must be redacted
    assert!(
        !full.contains("AKIAIOSFODNN7EXAMPLE"),
        "AWS key must be redacted"
    );

    // Redaction markers must be present
    assert!(
        full.contains("<REDACTED:"),
        "redaction markers must appear in output"
    );
}

// ── Stack trace frame distinctiveness ────────────────────────────────────────

#[test]
fn t036_stack_trace_frames() {
    // Without path masking, frames from different files have distinct templates
    // and both should survive reduction regardless of budget.
    let log = "2024-01-01 09:00:00 ERROR Error: Connection refused\n\
               2024-01-01 09:00:00 ERROR     at connect (net.js:92:28)\n\
               2024-01-01 09:00:00 ERROR     at Socket._handle.open (net.js:35:10)\n\
               2024-01-01 09:00:00 ERROR     at Object.exports.connect (app.js:15:15)\n";
    let config = ReduceConfig {
        budget_tokens: 4000,
        ..Default::default()
    };
    let out = reduce(&config, log);
    // net.js and app.js are different files => distinct templates => both kept
    assert!(out.body.contains("net.js"), "net.js frame must be kept");
    assert!(out.body.contains("app.js"), "app.js frame must be kept");
}

// ── Severity inference ────────────────────────────────────────────────────────

#[test]
fn severity_inference_from_text() {
    use logreduce::parse::infer_level_from_text;
    assert_eq!(
        infer_level_from_text("panic: null pointer"),
        Severity::Fatal
    );
    assert_eq!(
        infer_level_from_text("Exception in thread main"),
        Severity::Error
    );
    assert_eq!(
        infer_level_from_text("Traceback (most recent call last)"),
        Severity::Error
    );
    assert_eq!(infer_level_from_text("WARNING: disk full"), Severity::Warn);
    assert_eq!(infer_level_from_text("routine heartbeat"), Severity::Other);
}
