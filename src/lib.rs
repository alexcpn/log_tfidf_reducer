pub mod mask;
pub mod parse;
pub mod render;
pub mod score;
pub mod select;

use std::path::PathBuf;

// ── Domain types (data-model.md) ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Other = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
    Fatal = 4,
}

impl Severity {
    pub fn weight(self) -> f32 {
        match self {
            Severity::Fatal => 1.0,
            Severity::Error => 0.9,
            Severity::Warn => 0.6,
            Severity::Info => 0.2,
            Severity::Other => 0.1,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Fatal => "FATAL",
            Severity::Error => "ERROR",
            Severity::Warn => "WARN",
            Severity::Info => "INFO",
            Severity::Other => "OTHER",
        }
    }
}

pub type TemplateId = u64;

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub index: usize,
    pub timestamp: Option<i64>,
    pub level: Severity,
    pub raw: String,
    /// Raw text with secrets/PII redacted (set by mask stage; equals raw when redact=false)
    pub redacted: String,
    pub template: String,
    pub template_id: TemplateId,
}

#[derive(Debug, Clone)]
pub struct Template {
    pub id: TemplateId,
    pub masked: String,
    pub count: u64,
    pub representative: usize,
    pub rarity: f32,
}

#[derive(Debug, Clone)]
pub struct ScoredLine {
    pub record_index: usize,
    pub score: f32,
    pub kept: bool,
    pub as_context: bool,
}

#[derive(Debug, Default)]
pub struct SeverityCounts {
    pub fatal: u64,
    pub error: u64,
    pub warn: u64,
    pub info: u64,
    pub other: u64,
}

impl SeverityCounts {
    pub fn increment(&mut self, sev: Severity) {
        match sev {
            Severity::Fatal => self.fatal += 1,
            Severity::Error => self.error += 1,
            Severity::Warn => self.warn += 1,
            Severity::Info => self.info += 1,
            Severity::Other => self.other += 1,
        }
    }
}

#[derive(Debug)]
pub struct ReducedOutput {
    pub header: String,
    pub body: String,
    pub input_lines: usize,
    pub kept_lines: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
}

// ── Configuration (data-model.md: ReduceConfig) ───────────────────────────────

#[derive(Debug, Clone)]
pub enum LogFormat {
    Auto,
    Json,
    Logfmt,
    Syslog,
    Klog,
    Plain,
}

#[derive(Debug, Clone)]
pub struct BlendWeights {
    pub rarity: f32,
    pub severity: f32,
    pub burst: f32,
}

impl Default for BlendWeights {
    fn default() -> Self {
        // w_s ≥ w_r so severity rescues recurring errors (research R7, Principle II)
        Self {
            rarity: 0.4,
            severity: 0.5,
            burst: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReduceConfig {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub budget_tokens: u32,
    pub max_lines: Option<u32>,
    pub context_window: u8,
    pub weights: BlendWeights,
    pub redact: bool,
    pub format: LogFormat,
    pub show_stats: bool,
}

impl Default for ReduceConfig {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            budget_tokens: 8000,
            max_lines: None,
            context_window: 2,
            weights: BlendWeights::default(),
            redact: true,
            format: LogFormat::Auto,
            show_stats: false,
        }
    }
}

// ── Pipeline wiring ───────────────────────────────────────────────────────────

pub type TokenCounter = Box<dyn Fn(&str) -> usize + Send + Sync>;

/// Initialise the token counter once per reduce call (tiktoken startup is expensive).
pub fn make_token_counter() -> TokenCounter {
    match tiktoken_rs::cl100k_base() {
        Ok(enc) => {
            let enc = std::sync::Arc::new(enc);
            Box::new(move |s: &str| enc.encode_ordinary(s).len())
        }
        Err(_) => Box::new(|s: &str| s.chars().count().div_ceil(4)),
    }
}

pub fn reduce(config: &ReduceConfig, raw_text: &str) -> ReducedOutput {
    // Initialise once — tiktoken BPE model load is ~50 ms cold
    let counter = make_token_counter();

    // [1] Parse
    let records = parse::parse_lines(raw_text, config);

    // [2] Mask + redact → templates (order matters: redact before mask, R3)
    let records = mask::apply(records, config);

    // [3][4][5] Frequency table + TF-IDF + blended score
    let (records, templates) = score::build_and_score(records, config);

    // [6] Token-budget selection + context windows
    let selected = select::select(records.len(), &records, &templates, config, &counter);

    // [7] Chronological render + header + gap markers
    render::render(&records, &templates, &selected, config)
}
