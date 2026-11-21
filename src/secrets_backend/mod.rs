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
    /// Checks that the backend accepts our credentials (one cheap call).
    fn ping(&self) -> Result<(), BoxError> {
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
    let (path, cfg) = load_config()?;
    let kind = config::select_kind(&cfg, flag, &real_env)?;
    open_kind(&cfg, kind).map_err(|e| with_config_hint(e, &path, path.exists()))
}

/// Adds where the config lives (and whether it exists) to a backend
/// credential/connection error, so the user knows which file to edit.
pub fn with_config_hint(err: BoxError, path: &std::path::Path, exists: bool) -> BoxError {
    let where_ = if exists {
        format!("edit {}", path.display())
    } else {
        format!("config file {} does not exist yet", path.display())
    };
    format!("{err}\n  hint: {where_}\n  hint: see 'diegops explain backend' and 'diegops kv --help' to set one up")
        .into()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn credential_error_gets_config_path_hint_when_missing() {
        let out = super::with_config_hint(
            "no Infisical credentials: x".into(),
            Path::new("/h/.config/diegops/config.toml"),
            false,
        )
        .to_string();
        assert_eq!(
            out,
            "no Infisical credentials: x\n  hint: config file /h/.config/diegops/config.toml does not exist yet\n  hint: see 'diegops explain backend' and 'diegops kv --help' to set one up"
        );
    }

    #[test]
    fn credential_error_names_existing_config() {
        let out = super::with_config_hint("no Vault token: y".into(), Path::new("/c.toml"), true)
            .to_string();
        assert!(out.contains("\n  hint: edit /c.toml"), "{out}");
    }
}
