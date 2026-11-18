//! Backend selection and settings: `~/.config/diegops/config.toml` + env.

use serde::Deserialize;
use std::path::{Path, PathBuf};

use super::BoxError;

/// Root of `config.toml`.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// `infisical` (default) or `vault`.
    pub backend: Option<String>,
    /// Infisical settings.
    #[serde(default)]
    pub infisical: InfisicalConfig,
    /// Vault/OpenBao settings.
    #[serde(default)]
    pub vault: VaultConfig,
}

/// `[infisical]` table.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfisicalConfig {
    /// Base URL (default Infisical Cloud US).
    pub site_url: Option<String>,
    /// Default project ID for unmapped mounts.
    pub project_id: Option<String>,
    /// Default environment slug (default `prod`).
    pub environment: Option<String>,
    /// Universal Auth client ID (not a secret).
    pub client_id: Option<String>,
    /// File holding the Universal Auth client secret (must be mode 0600).
    pub client_secret_file: Option<String>,
    /// Command printing the client secret (e.g. `secret-tool lookup ...`).
    pub client_secret_command: Option<Vec<String>>,
    /// Per-mount overrides.
    #[serde(default)]
    pub mounts: Vec<MountMapping>,
}

/// `[[infisical.mounts]]` entry: Vault mount → Infisical project/env/folder.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountMapping {
    /// First path segment (Vault KV mount), e.g. `dpinto-work`.
    pub mount: String,
    /// Project ID (falls back to the default project).
    pub project_id: Option<String>,
    /// Environment slug (falls back to the default environment).
    pub environment: Option<String>,
    /// Base folder (default `/<mount>`).
    pub folder: Option<String>,
}

/// `[vault]` table (Vault or OpenBao).
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    /// Server address; no built-in default.
    pub address: Option<String>,
    /// Namespace sent as `X-Vault-Namespace`.
    pub namespace: Option<String>,
    /// Extra CA certificate (PEM) to trust.
    pub ca_cert: Option<String>,
    /// Token file (default `~/.vault-token`, written by `bao/vault login`).
    pub token_file: Option<String>,
}

/// Which backend implementation to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Infisical (default).
    Infisical,
    /// Vault / OpenBao KV v2.
    Vault,
}

/// Resolved Vault connection settings. No `Debug`: holds a token.
pub struct VaultSettings {
    /// Server address without trailing slash.
    pub address: String,
    /// Optional namespace.
    pub namespace: Option<String>,
    /// Optional extra CA certificate.
    pub ca_cert: Option<PathBuf>,
    /// Vault token.
    pub token: String,
}

/// Infisical credentials. No `Debug`: holds secrets.
pub enum InfisicalAuth {
    /// Pre-issued access token (`INFISICAL_TOKEN`).
    AccessToken(String),
    /// Machine identity Universal Auth.
    UniversalAuth {
        /// Client ID.
        client_id: String,
        /// Client secret.
        client_secret: String,
    },
}

/// Resolved Infisical settings.
pub struct InfisicalSettings {
    /// Base URL without trailing slash.
    pub site_url: String,
    /// Credentials.
    pub auth: InfisicalAuth,
    /// Path mapping.
    pub mapping: Mapping,
}

/// Physical Infisical location of a logical path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Project ID.
    pub project_id: String,
    /// Environment slug.
    pub environment: String,
    /// Absolute folder path (`/` separated, sanitised).
    pub folder: String,
}

/// Logical path → Infisical location mapping.
#[derive(Debug, Clone, Default)]
pub struct Mapping {
    default_project: Option<String>,
    default_env: Option<String>,
    mounts: Vec<MountMapping>,
}

/// Parses `config.toml` text.
pub fn parse(_text: &str) -> Result<Config, BoxError> {
    todo!()
}

/// Loads config from `path`; a missing file yields defaults.
pub fn load(_path: &Path) -> Result<Config, BoxError> {
    todo!()
}

/// `$DIEGOPS_CONFIG` > `$XDG_CONFIG_HOME/diegops/config.toml` > `~/.config/diegops/config.toml`.
pub fn default_path(_env: &dyn Fn(&str) -> Option<String>, _home: &Path) -> PathBuf {
    todo!()
}

/// Flag > `$DIEGOPS_SECRETS_BACKEND` > config `backend` > Infisical.
pub fn select_kind(
    _cfg: &Config,
    _flag: Option<&str>,
    _env: &dyn Fn(&str) -> Option<String>,
) -> Result<BackendKind, BoxError> {
    todo!()
}

/// Resolves Vault settings (env `VAULT_*` overrides the file).
pub fn resolve_vault(
    _cfg: &Config,
    _env: &dyn Fn(&str) -> Option<String>,
    _home: &Path,
) -> Result<VaultSettings, BoxError> {
    todo!()
}

/// Resolves Infisical settings (env `INFISICAL_*` overrides the file).
pub fn resolve_infisical(
    _cfg: &Config,
    _env: &dyn Fn(&str) -> Option<String>,
) -> Result<InfisicalSettings, BoxError> {
    todo!()
}

impl Mapping {
    /// Builds the mapping from the `[infisical]` table.
    pub fn from_config(_cfg: &InfisicalConfig) -> Self {
        todo!()
    }

    /// Maps `<mount>/<path>` to its Infisical location.
    pub fn locate(&self, _path: &str) -> Result<Location, BoxError> {
        todo!()
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
