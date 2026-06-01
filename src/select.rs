use crate::score::blended_score;
use crate::{LogRecord, ReduceConfig, ScoredLine, Template, TokenCounter};

pub fn select(
    n_records: usize,
    records: &[LogRecord],
    templates: &[Template],
    config: &ReduceConfig,
    count_tokens: &TokenCounter,
) -> Vec<ScoredLine> {
    if records.is_empty() {
        return vec![];
    }

    let max_rarity = templates.iter().map(|t| t.rarity).fold(0.0f32, f32::max);

    // Compute score for every record
    let mut scored: Vec<ScoredLine> = records
        .iter()
        .map(|rec| {
            let score = blended_score(rec, templates, config, max_rarity);
            ScoredLine {
                record_index: rec.index,
                score,
                kept: false,
                as_context: false,
            }
        })
        .collect();

    // Sort descending by score, then by original index (stable tiebreak, FR-010)
    let mut order: Vec<usize> = (0..n_records).collect();
    order.sort_by(|&a, &b| {
        scored[b]
            .score
            .partial_cmp(&scored[a].score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });

    let budget = config.budget_tokens as usize;
    let max_lines = config.max_lines.map(|m| m as usize).unwrap_or(usize::MAX);
    let mut used_tokens: usize = 0;
    let mut used_lines: usize = 0;

    // Track which templates have had a representative kept (Principle II guarantee)
    let mut template_seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut kept_indices: Vec<usize> = Vec::new();

    // Phase 1: guarantee ≥1 representative per template (FR-004)
    // Sort templates by rarity desc so we admit the most informative first
    let mut template_order: Vec<u64> = templates.iter().map(|t| t.id).collect();
    template_order.sort_by(|a, b| {
        let ra = templates
            .iter()
            .find(|t| t.id == *a)
            .map(|t| t.rarity)
            .unwrap_or(0.0);
        let rb = templates
            .iter()
            .find(|t| t.id == *b)
            .map(|t| t.rarity)
            .unwrap_or(0.0);
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });

    // First pass: one representative per template (ordered by score within template)
    for &tid in &template_order {
        // Find best-scoring record for this template
        if let Some(&best_idx) = order.iter().find(|&&i| records[i].template_id == tid) {
            let tokens = count_tokens(&records[best_idx].raw);
            // Always admit the single best-scoring rep even if we'd exceed budget
            // (we must not drop a template entirely — FR-004)
            if used_tokens + tokens <= budget && used_lines < max_lines {
                scored[best_idx].kept = true;
                template_seen.insert(tid);
                kept_indices.push(best_idx);
                used_tokens += tokens;
                used_lines += 1;
            } else if !template_seen.contains(&tid) {
                // Admit anyway to honour the guarantee, accepting budget overage
                scored[best_idx].kept = true;
                template_seen.insert(tid);
                kept_indices.push(best_idx);
                used_tokens += tokens;
                used_lines += 1;
            }
        }
    }

    // Second pass: fill remaining budget with highest-scoring records.
    // Additional records for an already-represented template are only admitted
    // when fewer than 25% of the budget has been used (truly ample budget).
    // This prevents a large block of identical lines from filling the budget
    // at the expense of diverse template coverage (Principle II).
    let initial_budget = budget;
    for &idx in &order {
        if scored[idx].kept {
            continue;
        }
        if used_tokens >= budget || used_lines >= max_lines {
            break;
        }
        let tid = records[idx].template_id;
        // Skip additional occurrences of already-represented templates.
        // Allow extras only for high-severity lines (ERROR/FATAL) when budget
        // is still mostly available — so crash-loops can show a few occurrences
        // but repetitive INFO lines don't fill the budget.
        if template_seen.contains(&tid) {
            let is_high_sev = matches!(
                records[idx].level,
                crate::Severity::Error | crate::Severity::Fatal
            );
            if !is_high_sev || used_tokens * 2 > initial_budget {
                continue;
            }
        }
        let tokens = count_tokens(&records[idx].raw);
        if used_tokens + tokens <= budget {
            scored[idx].kept = true;
            template_seen.insert(tid);
            kept_indices.push(idx);
            used_tokens += tokens;
            used_lines += 1;
        }
    }

    // Context windows (FR-015): pull in ±context adjacent records for each kept line
    let ctx = config.context_window as usize;
    if ctx > 0 {
        let kept_set: std::collections::HashSet<usize> = kept_indices.iter().cloned().collect();
        let mut context_candidates: Vec<usize> = Vec::new();
        for &ki in &kept_indices {
            let start = ki.saturating_sub(ctx);
            let end = (ki + ctx + 1).min(n_records);
            (start..end)
                .filter(|i| !kept_set.contains(i) && !scored[*i].as_context)
                .for_each(|i| context_candidates.push(i));
        }
        context_candidates.sort_unstable();
        context_candidates.dedup();
        for &cidx in &context_candidates {
            let rec = &records[cidx];
            let sc = &mut scored[cidx];
            let tokens = count_tokens(&rec.raw);
            if used_tokens + tokens <= budget {
                sc.as_context = true;
                sc.kept = true;
                used_tokens += tokens;
            }
        }
    }

    scored
}

