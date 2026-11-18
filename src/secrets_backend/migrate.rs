//! Copy secrets between backends with dry-run and SHA-256 verification.

use std::collections::BTreeMap;

use super::{codec, BoxError, SecretMap, SecretsBackend};

/// Outcome for one logical path. Holds counts only, never values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathReport {
    /// Logical source path.
    pub source: String,
    /// Target location id.
    pub target: String,
    /// Keys to create.
    pub create: usize,
    /// Keys whose value differs.
    pub update: usize,
    /// Keys already identical.
    pub unchanged: usize,
    /// `Some(true)` when every key verified after writing; `None` on dry-run.
    pub verified: Option<bool>,
}

/// Lists every leaf secret path under `prefix` (or `prefix` itself), sorted.
pub fn collect_paths(src: &dyn SecretsBackend, prefix: &str) -> Result<Vec<String>, BoxError> {
    let prefix = prefix.trim_matches('/');
    let mut out = Vec::new();
    // A path may be both a secret and a folder in KV v2.
    if prefix.contains('/') && src.get(prefix)?.is_some() {
        out.push(prefix.to_string());
    }
    let mut stack = vec![prefix.to_string()];
    while let Some(dir) = stack.pop() {
        for child in src.list(&dir)? {
            let full = format!("{dir}/{}", child.trim_end_matches('/'));
            if child.ends_with('/') {
                stack.push(full);
            } else {
                out.push(full);
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// SHA-256 per key (values are hashed in memory, never printed).
fn digests(map: &SecretMap) -> BTreeMap<&str, String> {
    map.iter()
        .map(|(k, v)| (k.as_str(), codec::sha256_hex(v.as_bytes())))
        .collect()
}

/// Copies all paths under `prefixes` from `src` to `dst`.
///
/// Plans everything first (and aborts on target collisions) before writing.
/// Re-running is a no-op once the target matches. Nothing is deleted.
pub fn migrate(
    src: &dyn SecretsBackend,
    dst: &dyn SecretsBackend,
    prefixes: &[String],
    dry_run: bool,
) -> Result<Vec<PathReport>, BoxError> {
    let mut paths = Vec::new();
    for p in prefixes {
        paths.extend(collect_paths(src, p)?);
    }
    paths.sort();
    paths.dedup();

    // Plan: detect collisions before touching the target.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut plan: Vec<(String, String, SecretMap, PathReport)> = Vec::new();
    for path in paths {
        let target = dst.location_id(&path)?;
        if let Some(other) = seen.insert(target.clone(), path.clone()) {
            return Err(format!(
                "'{other}' and '{path}' both map to {target}; rename one in the source or add a [[infisical.mounts]] mapping"
            )
            .into());
        }
        let Some(data) = src.get(&path)? else {
            continue;
        };
        let current = dst.get(&path)?.unwrap_or_default();
        let (want, have) = (digests(&data), digests(&current));
        let mut report = PathReport {
            source: path.clone(),
            target: target.clone(),
            create: 0,
            update: 0,
            unchanged: 0,
            verified: None,
        };
        let mut changed = SecretMap::new();
        for (k, h) in &want {
            match have.get(k) {
                None => report.create += 1,
                Some(cur) if cur != h => report.update += 1,
                Some(_) => {
                    report.unchanged += 1;
                    continue;
                }
            }
            changed.insert(k.to_string(), data[*k].clone());
        }
        plan.push((path, target, changed, report));
    }

    let mut reports = Vec::new();
    for (path, _target, changed, mut report) in plan {
        if !dry_run {
            if !changed.is_empty() {
                dst.put(&path, &changed)?;
            }
            let src_now = src.get(&path)?.unwrap_or_default();
            let dst_now = dst.get(&path)?.unwrap_or_default();
            let (want, have) = (digests(&src_now), digests(&dst_now));
            report.verified = Some(want.iter().all(|(k, h)| have.get(k) == Some(h)));
        }
        reports.push(report);
    }
    Ok(reports)
}

/// Human-readable table (no values).
pub fn render(reports: &[PathReport]) -> String {
    let mut out = String::new();
    for r in reports {
        let status = match r.verified {
            None => "PLAN",
            Some(true) => "OK",
            Some(false) => "MISMATCH",
        };
        out.push_str(&format!(
            "{status:<8} {} -> {}  (create {}, update {}, unchanged {})\n",
            r.source, r.target, r.create, r.update, r.unchanged
        ));
    }
    let sum = |f: fn(&PathReport) -> usize| reports.iter().map(f).sum::<usize>();
    out.push_str(&format!(
        "{} paths: {} keys to create, {} to update, {} unchanged",
        reports.len(),
        sum(|r| r.create),
        sum(|r| r.update),
        sum(|r| r.unchanged)
    ));
    let bad = reports.iter().filter(|r| r.verified == Some(false)).count();
    if bad > 0 {
        out.push_str(&format!(", {bad} MISMATCHED"));
    }
    out.push('\n');
    out
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
