use crate::{LogFormat, LogRecord, ReduceConfig, Severity};

pub fn parse_lines(raw: &str, config: &ReduceConfig) -> Vec<LogRecord> {
    if raw.trim().is_empty() {
        return vec![];
    }

    let format = match config.format {
        LogFormat::Auto => detect_format(raw),
        LogFormat::Json => LogFormat::Json,
        LogFormat::Logfmt => LogFormat::Logfmt,
        LogFormat::Syslog => LogFormat::Syslog,
        LogFormat::Klog => LogFormat::Klog,
        LogFormat::Plain => LogFormat::Plain,
    };

    let lines: Vec<&str> = raw.lines().collect();
    let joined = join_continuations(&lines, &format);

    joined
        .into_iter()
        .enumerate()
        .map(|(idx, raw_line)| {
            let (ts, level, _msg) = extract_fields(&raw_line, &format);
            LogRecord {
                index: idx,
                timestamp: ts,
                level,
                raw: raw_line.clone(),
                redacted: String::new(), // filled by mask stage
                template: String::new(), // filled by mask stage
                template_id: 0,          // filled by score stage
            }
        })
        .collect()
}

fn detect_format(raw: &str) -> LogFormat {
    // Sniff first 200 non-empty lines
    let sample: Vec<&str> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(200)
        .collect();
    let json_score = sample
        .iter()
        .filter(|l| l.trim_start().starts_with('{'))
        .count();
    let klog_score = sample.iter().filter(|l| is_klog_line(l)).count();
    let syslog_score = sample.iter().filter(|l| is_syslog_line(l)).count();
    let logfmt_score = sample
        .iter()
        .filter(|l| l.contains('=') && !l.trim_start().starts_with('{'))
        .count();

    let n = sample.len().max(1);
    if json_score * 100 / n > 50 {
        LogFormat::Json
    } else if klog_score * 100 / n > 30 {
        LogFormat::Klog
    } else if syslog_score * 100 / n > 30 {
        LogFormat::Syslog
    } else if logfmt_score * 100 / n > 30 {
        LogFormat::Logfmt
    } else {
        LogFormat::Plain
    }
}

fn is_klog_line(line: &str) -> bool {
    // klog/glog: "I0601 12:34:56.789012 …" or "E0601 …"
    let b = line.as_bytes();
    b.first()
        .map(|c| matches!(c, b'I' | b'W' | b'E' | b'F'))
        .unwrap_or(false)
        && b.get(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
}

fn is_syslog_line(line: &str) -> bool {
    // Rough syslog heuristic: "Jun  1 12:34:56 host …"
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    months.iter().any(|m| line.starts_with(m))
}

// Join continuation lines (e.g. stack frames that don't start with a timestamp)
fn join_continuations(lines: &[&str], format: &LogFormat) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for &line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let is_continuation = match format {
            LogFormat::Json => !line.trim_start().starts_with('{'),
            LogFormat::Klog => !is_klog_line(line),
            LogFormat::Syslog => !is_syslog_line(line),
            LogFormat::Plain | LogFormat::Logfmt | LogFormat::Auto => {
                // Indented line → continuation
                line.starts_with("    ") || line.starts_with('\t')
            }
        };

        if is_continuation {
            if let Some(last) = out.last_mut() {
                last.push('\n');
                last.push_str(line);
            } else {
                out.push(line.to_string());
            }
        } else {
            out.push(line.to_string());
        }
    }
    out
}

pub fn extract_fields(line: &str, format: &LogFormat) -> (Option<i64>, Severity, String) {
    match format {
        LogFormat::Json => extract_json(line),
        LogFormat::Klog => extract_klog(line),
        LogFormat::Syslog => extract_syslog(line),
        LogFormat::Logfmt => extract_logfmt(line),
        _ => extract_plain(line),
    }
}

fn extract_json(line: &str) -> (Option<i64>, Severity, String) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
        let level = v
            .get("level")
            .or_else(|| v.get("severity"))
            .or_else(|| v.get("lvl"))
            .and_then(|l| l.as_str())
            .map(parse_level_str)
            .unwrap_or(Severity::Other);
        let msg = v
            .get("message")
            .or_else(|| v.get("msg"))
            .or_else(|| v.get("text"))
            .and_then(|m| m.as_str())
            .unwrap_or(line)
            .to_string();
        let ts = v
            .get("timestamp")
            .or_else(|| v.get("time"))
            .or_else(|| v.get("ts"))
            .and_then(|t| t.as_i64());
        (ts, level, msg)
    } else {
        extract_plain(line)
    }
}

fn extract_klog(line: &str) -> (Option<i64>, Severity, String) {
    // E0601 12:34:56.789 threadid file.go:123] message
    let bytes = line.as_bytes();
    let level = match bytes.first() {
        Some(b'I') => Severity::Info,
        Some(b'W') => Severity::Warn,
        Some(b'E') => Severity::Error,
        Some(b'F') => Severity::Fatal,
        _ => Severity::Other,
    };
    let msg = line
        .find(']')
        .map(|i| line[i + 1..].trim())
        .unwrap_or(line)
        .to_string();
    (None, level, msg)
}

fn extract_syslog(line: &str) -> (Option<i64>, Severity, String) {
    // "Jun  1 12:34:56 host process[pid]: message"
    let level = infer_level_from_text(line);
    (None, level, line.to_string())
}

fn extract_logfmt(line: &str) -> (Option<i64>, Severity, String) {
    let level = line
        .split_whitespace()
        .find_map(|tok| {
            let (k, v) = tok.split_once('=')?;
            if k == "level" || k == "lvl" || k == "severity" {
                Some(parse_level_str(v.trim_matches('"')))
            } else {
                None
            }
        })
        .unwrap_or_else(|| infer_level_from_text(line));
    let msg = line.to_string();
    (None, level, msg)
}

fn extract_plain(line: &str) -> (Option<i64>, Severity, String) {
    (None, infer_level_from_text(line), line.to_string())
}

pub fn parse_level_str(s: &str) -> Severity {
    match s.to_uppercase().as_str() {
        "FATAL" | "CRITICAL" | "PANIC" | "EMERG" | "ALERT" => Severity::Fatal,
        "ERROR" | "ERR" | "E" => Severity::Error,
        "WARN" | "WARNING" | "W" => Severity::Warn,
        "INFO" | "INFORMATION" | "I" | "NOTICE" => Severity::Info,
        _ => Severity::Other,
    }
}

// Keyword-based severity inference for unstructured logs (FR-005a, Q3)
pub fn infer_level_from_text(text: &str) -> Severity {
    let upper = text.to_uppercase();
    if upper.contains("PANIC") || upper.contains("FATAL") || upper.contains("CRITICAL") {
        Severity::Fatal
    } else if upper.contains("ERROR")
        || upper.contains("EXCEPTION")
        || upper.contains("TRACEBACK")
        || upper.contains("STACKTRACE")
    {
        Severity::Error
    } else if upper.contains("WARN") {
        Severity::Warn
    } else if upper.contains(" INFO ") || upper.contains("[INFO]") {
        Severity::Info
    } else {
        Severity::Other
    }
}
