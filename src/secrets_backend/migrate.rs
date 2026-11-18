//! Copy secrets between backends with dry-run and SHA-256 verification.

use super::{BoxError, SecretMap, SecretsBackend};

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

/// Lists every leaf secret path under `prefix` (or `prefix` itself).
pub fn collect_paths(_src: &dyn SecretsBackend, _prefix: &str) -> Result<Vec<String>, BoxError> {
    todo!()
}

/// Copies all paths under `prefixes` from `src` to `dst`.
pub fn migrate(
    _src: &dyn SecretsBackend,
    _dst: &dyn SecretsBackend,
    _prefixes: &[String],
    _dry_run: bool,
) -> Result<Vec<PathReport>, BoxError> {
    let _: Option<SecretMap> = None;
    todo!()
}

/// Human-readable table (no values).
pub fn render(_reports: &[PathReport]) -> String {
    todo!()
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
