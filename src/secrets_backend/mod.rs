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
    /// Rejects key names this backend cannot store verbatim.
    fn validate_key(&self, _key: &str) -> Result<(), BoxError> {
        Ok(())
    }
    /// Identifier of the physical location `path` maps to (collision checks).
    fn location_id(&self, path: &str) -> Result<String, BoxError> {
        Ok(path.to_string())
    }
}

/// Real environment lookup.
pub fn real_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Home directory (`$HOME`, else `$USERPROFILE`).
pub fn home() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
}

/// Loads the diegops config from its default location.
pub fn load_config() -> Result<(std::path::PathBuf, config::Config), BoxError> {
    let path = config::default_path(&real_env, &home());
    let cfg = config::load(&path)?;
    Ok((path, cfg))
}

/// Opens a backend of the given kind using the real config and env.
pub fn open_kind(
    cfg: &config::Config,
    kind: config::BackendKind,
) -> Result<Box<dyn SecretsBackend>, BoxError> {
    Ok(match kind {
        config::BackendKind::Infisical => Box::new(infisical::InfisicalBackend::new(
            config::resolve_infisical(cfg, &real_env)?,
        )),
        config::BackendKind::Vault => Box::new(vault::VaultBackend::new(config::resolve_vault(
            cfg,
            &real_env,
            &home(),
        )?)?),
    })
}

/// Opens the selected backend (flag > env > config > Infisical).
pub fn open(flag: Option<&str>) -> Result<Box<dyn SecretsBackend>, BoxError> {
    let (_, cfg) = load_config()?;
    let kind = config::select_kind(&cfg, flag, &real_env)?;
    open_kind(&cfg, kind)
}
