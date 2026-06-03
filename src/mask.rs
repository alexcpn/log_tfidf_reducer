use crate::{LogRecord, ReduceConfig};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use rustc_hash::FxHasher;
use std::hash::Hasher;

// ── Secret / PII redaction patterns (FR-014, research R3) ──────────────────
// Applied BEFORE variable masking so secrets never inflate rarity.

static RE_JWT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").unwrap());

static RE_AWS_KEY: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap());

static RE_BEARER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]+=*").unwrap());

static RE_PEM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----")
        .unwrap()
});

static RE_EMAIL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap());

// key=value or "key":"value" where key is a secret keyword
static RE_KV_SECRET: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)(?:password|passwd|secret|token|api[_-]?key|apikey|authorization|access[_-]?key|private[_-]?key)\s*[=:]\s*["']?([^\s"',}\]\n]{6,})["']?"#,
    )
    .unwrap()
});

// ── Variable masking patterns (FR-003, research R4) ─────────────────────────
// Applied AFTER redaction.

static RE_TS_ISO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:\d{2})?").unwrap()
});
static RE_TS_UNIX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b1[5-9]\d{8,12}\b").unwrap());
static RE_TS_LOG: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d{2}:\d{2}:\d{2}(?:[.,]\d+)?").unwrap());
static RE_UUID: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")
        .unwrap()
});
static RE_HEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b0x[0-9a-fA-F]{4,}\b").unwrap());
static RE_HASH: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[0-9a-f]{32,64}\b").unwrap());
static RE_IP4: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}(?::\d{2,5})?\b").unwrap());
static RE_IP6: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:[0-9a-fA-F]{1,4}:){3,7}[0-9a-fA-F]{1,4}\b").unwrap());
static RE_DUR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d+(?:\.\d+)?(?:ms|us|µs|ns|s|m|h)\b").unwrap());
static RE_NUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{2,}\b").unwrap());

pub fn apply(mut records: Vec<LogRecord>, config: &ReduceConfig) -> Vec<LogRecord> {
    records.par_iter_mut().for_each(|rec| {
        let redacted = if config.redact {
            redact(&rec.raw)
        } else {
            rec.raw.clone()
        };
        rec.template = mask_variables(&redacted);
        rec.redacted = redacted;
    });
    records
}

fn redact(s: &str) -> String {
    let s = RE_JWT.replace_all(s, "<REDACTED:jwt>");
    let s = RE_AWS_KEY.replace_all(&s, "<REDACTED:aws_key>");
    let s = RE_BEARER.replace_all(&s, "<REDACTED:bearer>");
    let s = RE_PEM.replace_all(&s, "<REDACTED:pem>");
    let s = RE_EMAIL.replace_all(&s, "<REDACTED:email>");
    // For KV secrets: keep the key name, redact the value
    let s = RE_KV_SECRET.replace_all(&s, |caps: &regex::Captures| {
        let full = &caps[0];
        let val = &caps[1];
        full.replace(val, "<REDACTED:secret>")
    });
    // High-entropy backstop (research R3): base64/hex tokens ≥ 20 chars with entropy ≥ 3.5
    let s = s
        .split_whitespace()
        .map(|tok| {
            let clean = tok.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.len() >= 20 && looks_base64_or_hex(clean) && shannon_entropy(clean) >= 3.5 {
                tok.replace(clean, "<REDACTED:hi>")
            } else {
                tok.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    s.to_string()
}

fn mask_variables(s: &str) -> String {
    let s = RE_TS_ISO.replace_all(s, "<TS>");
    let s = RE_TS_UNIX.replace_all(&s, "<TS>");
    let s = RE_TS_LOG.replace_all(&s, "<TS>");
    let s = RE_UUID.replace_all(&s, "<ID>");
    let s = RE_HEX.replace_all(&s, "<ID>");
    let s = RE_HASH.replace_all(&s, "<ID>");
    let s = RE_IP4.replace_all(&s, "<IP>");
    let s = RE_IP6.replace_all(&s, "<IP>");
    let s = RE_DUR.replace_all(&s, "<DUR>");
    let s = RE_NUM.replace_all(&s, "<NUM>");
    // Collapse whitespace for stable template keys
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn looks_base64_or_hex(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=' || c == '-' || c == '_')
}

fn shannon_entropy(s: &str) -> f64 {
    let len = s.len() as f64;
    if len == 0.0 {
        return 0.0;
    }
    let mut freq = [0u32; 128];
    for b in s.bytes() {
        if (b as usize) < 128 {
            freq[b as usize] += 1;
        }
    }
    freq.iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

pub fn template_id(template: &str) -> u64 {
    let mut h = FxHasher::default();
    for b in template.bytes() {
        h.write_u8(b);
    }
    h.finish()
}
