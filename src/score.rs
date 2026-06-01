use crate::mask::template_id;
use crate::{LogRecord, ReduceConfig, ScoredLine, Template, TemplateId};
use rustc_hash::FxHashMap;

struct TemplateStat {
    masked: String,
    count: u64,
    representative: usize,
    term_freq: FxHashMap<String, u32>,
}

pub fn build_and_score(
    mut records: Vec<LogRecord>,
    config: &ReduceConfig,
) -> (Vec<LogRecord>, Vec<Template>) {
    // [3] Build frequency table over distinct templates
    let mut stats: FxHashMap<TemplateId, TemplateStat> = FxHashMap::default();

    for rec in &mut records {
        let tid = template_id(&rec.template);
        rec.template_id = tid;

        let stat = stats.entry(tid).or_insert_with(|| TemplateStat {
            masked: rec.template.clone(),
            count: 0,
            representative: rec.index,
            term_freq: FxHashMap::default(),
        });
        stat.count += 1;
        for term in rec.template.split_whitespace() {
            *stat.term_freq.entry(term.to_string()).or_insert(0) += 1;
        }
    }

    let n_templates = stats.len() as f64;

    // [4] Build IDF table over all terms in all templates
    // df_t = number of distinct templates containing term t
    let mut df: FxHashMap<String, u64> = FxHashMap::default();
    for stat in stats.values() {
        for term in stat.term_freq.keys() {
            *df.entry(term.clone()).or_insert(0) += 1;
        }
    }

    // idf_t = ln(N / df_t) + 1
    let idf = |term: &str| -> f32 {
        let d = *df.get(term).unwrap_or(&1) as f64;
        (n_templates / d).ln() as f32 + 1.0
    };

    // [5] Compute rarity per template: Σ tf(t) · idf(t), un-normalized
    let mut max_rarity: f32 = 0.0;
    let mut rarities: FxHashMap<TemplateId, f32> = FxHashMap::default();
    for (tid, stat) in &stats {
        let rarity: f32 = stat
            .term_freq
            .iter()
            .map(|(term, &tf)| tf as f32 * idf(term))
            .sum();
        rarities.insert(*tid, rarity);
        if rarity > max_rarity {
            max_rarity = rarity;
        }
    }

    // Burst score: templates that appear only once get a small bump
    let burst_score = |count: u64| -> f32 {
        if count == 1 {
            1.0
        } else {
            0.0
        }
    };

    // [5] Blended score per record: w_r·norm(rarity) + w_s·severity + w_f·burst
    let w = &config.weights;
    let scored: Vec<ScoredLine> = records
        .iter()
        .map(|rec| {
            let norm_rarity = if max_rarity > 0.0 {
                rarities.get(&rec.template_id).copied().unwrap_or(0.0) / max_rarity
            } else {
                0.0
            };
            let sev = rec.level.weight();
            let burst = burst_score(stats.get(&rec.template_id).map(|s| s.count).unwrap_or(1));
            let score = w.rarity * norm_rarity + w.severity * sev + w.burst * burst;
            ScoredLine {
                record_index: rec.index,
                score,
                kept: false,
                as_context: false,
            }
        })
        .collect();

    // Attach scored lines back — we return them separately
    // Build the Template vec for downstream use
    let mut templates: Vec<Template> = stats
        .into_iter()
        .map(|(tid, stat)| Template {
            id: tid,
            masked: stat.masked,
            count: stat.count,
            representative: stat.representative,
            rarity: rarities.get(&tid).copied().unwrap_or(0.0),
        })
        .collect();
    templates.sort_by(|a, b| {
        b.rarity
            .partial_cmp(&a.rarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Attach scored info onto records (reuse ScoredLine by embedding score in record?
    // — we return scored separately and let select.rs use it)
    // Store scored lines indexed by record index for select stage
    let _ = scored; // passed via return

    // Re-derive scored for return
    let max_rarity2 = templates.iter().map(|t| t.rarity).fold(0.0f32, f32::max);
    let rarity_map: FxHashMap<TemplateId, f32> =
        templates.iter().map(|t| (t.id, t.rarity)).collect();
    let count_map: FxHashMap<TemplateId, u64> = templates.iter().map(|t| (t.id, t.count)).collect();

    // Re-attach rarity scores to records (needed by select)
    for rec in &mut records {
        let _ = rec; // template_id already set above
    }

    // Build final scored list aligned with records
    let scored: Vec<ScoredLine> = records
        .iter()
        .map(|rec| {
            let norm_rarity = if max_rarity2 > 0.0 {
                rarity_map.get(&rec.template_id).copied().unwrap_or(0.0) / max_rarity2
            } else {
                0.0
            };
            let sev = rec.level.weight();
            let burst = if count_map.get(&rec.template_id).copied().unwrap_or(0) == 1 {
                1.0f32
            } else {
                0.0
            };
            let score = w.rarity * norm_rarity + w.severity * sev + w.burst * burst;
            ScoredLine {
                record_index: rec.index,
                score,
                kept: false,
                as_context: false,
            }
        })
        .collect();

    // Attach scored as a side-channel through records
    // We use a simple approach: embed score in a parallel vec returned alongside
    // (select.rs receives both records and templates, and scores derived from them)
    let _ = scored;

    (records, templates)
}

/// Re-compute blended score for a single record (used by select).
pub fn blended_score(
    rec: &LogRecord,
    templates: &[Template],
    config: &ReduceConfig,
    max_rarity: f32,
) -> f32 {
    let rarity = templates
        .iter()
        .find(|t| t.id == rec.template_id)
        .map(|t| t.rarity)
        .unwrap_or(0.0);
    let norm_rarity = if max_rarity > 0.0 {
        rarity / max_rarity
    } else {
        0.0
    };
    let sev = rec.level.weight();
    let count = templates
        .iter()
        .find(|t| t.id == rec.template_id)
        .map(|t| t.count)
        .unwrap_or(1);
    let burst = if count == 1 { 1.0f32 } else { 0.0 };
    let w = &config.weights;
    w.rarity * norm_rarity + w.severity * sev + w.burst * burst
}
