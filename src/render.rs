use crate::ReduceConfig;
use crate::{LogRecord, ReducedOutput, ScoredLine, SeverityCounts, Template};
use std::collections::HashMap;

pub fn render(
    records: &[LogRecord],
    templates: &[Template],
    scored: &[ScoredLine],
    _config: &ReduceConfig,
) -> ReducedOutput {
    // Build a template count map
    let template_counts: HashMap<u64, u64> = templates.iter().map(|t| (t.id, t.count)).collect();

    // Collect kept records sorted chronologically (timestamp then index, FR-008/010)
    let mut kept: Vec<(usize, &LogRecord)> = records
        .iter()
        .enumerate()
        .filter(|(i, _)| scored.get(*i).map(|s| s.kept).unwrap_or(false))
        .map(|(_, rec)| (rec.index, rec))
        .collect();
    kept.sort_by_key(|(idx, rec)| (rec.timestamp.unwrap_or(i64::MAX), *idx));

    // Severity counts over ALL records (for header summary)
    let mut sev_counts = SeverityCounts::default();
    for rec in records {
        sev_counts.increment(rec.level);
    }

    // Top templates by count for header
    let mut top_templates: Vec<&Template> = templates.iter().collect();
    top_templates.sort_by_key(|t: &&Template| std::cmp::Reverse(t.count));
    let top_n = 5;

    // Timestamp range
    let (ts_start, ts_end) = records
        .iter()
        .fold((None::<i64>, None::<i64>), |(mn, mx), r| {
            (
                Some(
                    mn.map(|m| m.min(r.timestamp.unwrap_or(i64::MAX)))
                        .unwrap_or(r.timestamp.unwrap_or(i64::MAX)),
                ),
                Some(
                    mx.map(|m| m.max(r.timestamp.unwrap_or(i64::MIN)))
                        .unwrap_or(r.timestamp.unwrap_or(i64::MIN)),
                ),
            )
        });

    let time_range = match (ts_start, ts_end) {
        (Some(s), Some(e)) if s != i64::MAX && e != i64::MIN => {
            format!(" | epoch {s}–{e}")
        }
        _ => String::new(),
    };

    let input_lines = records.len();
    let kept_count = kept.len();

    let header = build_header(
        input_lines,
        kept_count,
        &sev_counts,
        &top_templates[..top_n.min(top_templates.len())],
        &time_range,
    );

    // Build body with gap markers and collapse counts (FR-007/008)
    let body = build_body(records, &kept, scored, &template_counts);

    // Token counts (approximate via chars/4 ceiling)
    let input_tokens = records
        .iter()
        .map(|r| r.raw.len())
        .sum::<usize>()
        .div_ceil(4);
    let output_tokens = (header.len() + body.len()).div_ceil(4);

    ReducedOutput {
        header,
        body,
        input_lines,
        kept_lines: kept_count,
        input_tokens,
        output_tokens,
    }
}

fn build_header(
    input: usize,
    kept: usize,
    sev: &SeverityCounts,
    top: &[&Template],
    time_range: &str,
) -> String {
    let ratio = if input > 0 {
        (kept * 100).checked_div(input).unwrap_or(0)
    } else {
        0
    };
    let mut h = format!("# LOG SUMMARY  |  {input} lines → {kept} kept ({ratio}%){time_range}\n");
    h.push_str(&format!(
        "# levels: FATAL {f}  ERROR {e}  WARN {w}  INFO {i}  OTHER {o}\n",
        f = sev.fatal,
        e = sev.error,
        w = sev.warn,
        i = sev.info,
        o = sev.other,
    ));
    if !top.is_empty() {
        let top_str: Vec<String> = top
            .iter()
            .map(|t| format!("\"{}\" ×{}", t.masked, t.count))
            .collect();
        h.push_str(&format!("# top templates: {}\n", top_str.join(" ; ")));
    }
    h.push_str(
        "# Note: token counts use tiktoken cl100k_base — slightly conservative for Claude.\n",
    );
    h
}

fn build_body(
    records: &[LogRecord],
    kept: &[(usize, &LogRecord)],
    scored: &[ScoredLine],
    template_counts: &HashMap<u64, u64>,
) -> String {
    if kept.is_empty() {
        return String::new();
    }

    let kept_set: std::collections::HashSet<usize> = kept.iter().map(|(idx, _)| *idx).collect();

    // Walk records in index order to detect gaps
    let mut body = String::new();
    let mut prev_kept_idx: Option<usize> = None;
    let mut omit_start: Option<usize> = None;
    let mut omit_count: usize = 0;

    // Track which templates we've already emitted (for collapse annotation)
    let mut template_emitted: std::collections::HashSet<u64> = std::collections::HashSet::new();

    let flush_gap = |body: &mut String, omit_count: usize, omit_start: Option<usize>| {
        if omit_count > 0 {
            let at = omit_start
                .map(|s| format!(" (from line {s})"))
                .unwrap_or_default();
            body.push_str(&format!("… {omit_count} lines omitted{at} …\n"));
        }
    };

    for (i, rec) in records.iter().enumerate() {
        if kept_set.contains(&i) {
            flush_gap(&mut body, omit_count, omit_start);
            omit_count = 0;
            omit_start = None;

            let is_context = scored.get(i).map(|s| s.as_context).unwrap_or(false);
            let count = template_counts.get(&rec.template_id).copied().unwrap_or(1);
            let already_emitted = template_emitted.contains(&rec.template_id);

            // Show count annotation for first occurrence if it's a repeat (FR-007)
            let count_annotation = if count > 1 && !already_emitted {
                format!("   (×{count})")
            } else {
                String::new()
            };

            // Context lines get a subtle marker
            let ctx_marker = if is_context { "  ·" } else { "" };

            template_emitted.insert(rec.template_id);
            // Emit redacted text (secrets replaced) not raw (FR-014)
            let display = if rec.redacted.is_empty() {
                &rec.raw
            } else {
                &rec.redacted
            };
            // Multi-line records (joined continuation lines, e.g. stack traces):
            // put annotation on the first line, emit remaining lines verbatim.
            let mut display_lines = display.lines();
            if let Some(first) = display_lines.next() {
                body.push_str(&format!("{first}{count_annotation}{ctx_marker}\n"));
                for cont in display_lines {
                    body.push_str(cont);
                    body.push('\n');
                }
            }

            prev_kept_idx = Some(i);
        } else {
            if omit_count == 0 {
                omit_start = prev_kept_idx.map(|p| p + 1);
            }
            omit_count += 1;
        }
    }
    flush_gap(&mut body, omit_count, omit_start);

    body
}
