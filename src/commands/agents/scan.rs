//! Secret scanner for agent files before they reach git.
//!
//! Same rules as the legacy `cadi-hermes-state/scripts/scan-secrets.sh`
//! (case-insensitive, `#`-commented lines ignored) and the same allowlist
//! format: one `sha256(line)` hex digest per line, so reviewed placeholders
//! stay allowed. Reports carry the line number and the rule, never the value.

use std::collections::HashSet;
use std::fmt;
use std::sync::OnceLock;

use regex::Regex;
use sha2::{Digest, Sha256};

/// One suspicious line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 1-based line number.
    pub line: usize,
    /// Rule that matched (never the value).
    pub rule: &'static str,
}

impl fmt::Display for Hit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {} ({})", self.line, self.rule)
    }
}

/// Reviewed false positives, by line hash.
#[derive(Debug, Default, Clone)]
pub struct Allowlist(HashSet<String>);

impl Allowlist {
    /// Parses an allowlist file (one hex digest per line).
    pub fn parse(text: &str) -> Self {
        Allowlist(
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_lowercase)
                .collect(),
        )
    }

    /// Loads `path`; a missing file is an empty allowlist.
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        match std::fs::read_to_string(path) {
            Ok(t) => Ok(Self::parse(&t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("could not read {}: {e}", path.display()).into()),
        }
    }

    fn allows(&self, line: &str) -> bool {
        self.0.contains(&line_hash(line))
    }
}

/// `sha256(line)` as lowercase hex (legacy allowlist format).
pub fn line_hash(line: &str) -> String {
    Sha256::digest(line.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn rules() -> &'static [(&'static str, Regex)] {
    static RULES: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    RULES.get_or_init(|| {
        [
            ("sk- api key", r"(?:^|[^A-Za-z0-9_-])sk-[A-Za-z0-9_-]{20,}"),
            (
                "github token",
                r"gh[po]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}",
            ),
            ("slack token", r"xox[abp]-[A-Za-z0-9-]{10,}"),
            ("aws access key", r"AKIA[0-9A-Z]{16}"),
            ("google api key", r"AIza[0-9A-Za-z_-]{35}"),
            ("private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
            ("telegram bot token", r"[0-9]{8,10}:AA[A-Za-z0-9_-]{30,}"),
            ("vault token", r"hvs\.[A-Za-z0-9_-]{20,}"),
            (
                "credential assignment",
                r#"^\s*[A-Za-z_]*(?:api_?key|token|secret|password)[A-Za-z_]*\s*[:=]\s*["']?[A-Za-z0-9_./+-]{16,}"#,
            ),
        ]
        .into_iter()
        .filter_map(|(name, re)| Regex::new(&format!("(?i){re}")).ok().map(|r| (name, r)))
        .collect()
    })
}

/// Scans text; one hit per suspicious, non-commented, non-allowlisted line.
pub fn scan_text(text: &str, allow: &Allowlist) -> Vec<Hit> {
    let mut out = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((rule, _)) = rules().iter().find(|(_, re)| re.is_match(line)) {
            if !allow.allows(line) {
                out.push(Hit { line: i + 1, rule });
            }
        }
    }
    out
}

/// Scans arbitrary bytes (binary content is decoded lossily).
pub fn scan_bytes(bytes: &[u8], allow: &Allowlist) -> Vec<Hit> {
    scan_text(&String::from_utf8_lossy(bytes), allow)
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
