//! Pluggable secrets backends (ADR: docs/secrets-backends.md).
//!
//! A logical secret path is `<mount>/<path...>` (the Vault KV v2 notation
//! already used in `secrets.yaml`). Each backend maps it onto its own storage:
//! - Infisical: mount → project/environment, path → folder, keys → secrets.
//! - Vault/OpenBao: KV v2 `<mount>/data/<path>`.

pub mod codec;
pub mod config;
pub mod infisical;
pub mod migrate;
pub mod vault;

use std::collections::BTreeMap;

/// Boxed error used across backends (matches the rest of the crate).
pub type BoxError = Box<dyn std::error::Error>;

/// Key → value map stored at one logical path.
pub type SecretMap = BTreeMap<String, String>;

/// A key/value secrets store addressed by logical `<mount>/<path>` paths.
pub trait SecretsBackend {
    /// Short backend name for messages.
    fn name(&self) -> &'static str;
    /// Reads all keys at `path`; `None` when nothing is stored there.
    fn get(&self, path: &str) -> Result<Option<SecretMap>, BoxError>;
    /// Lists children of `path`; sub-paths end with `/`.
    fn list(&self, path: &str) -> Result<Vec<String>, BoxError>;
    /// Upserts `data` at `path`; keys not in `data` are left untouched.
    fn put(&self, path: &str, data: &SecretMap) -> Result<(), BoxError>;
    /// Identifier of the physical location `path` maps to (collision checks).
    fn location_id(&self, path: &str) -> Result<String, BoxError> {
        Ok(path.to_string())
    }
}
