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

/// Default Infisical Cloud URL (US region).
pub const DEFAULT_INFISICAL_URL: &str = "https://app.infisical.com";

/// Default environment slug.
pub const DEFAULT_ENVIRONMENT: &str = "prod";

/// Parses `config.toml` text.
pub fn parse(text: &str) -> Result<Config, BoxError> {
    toml::from_str(text).map_err(|e| format!("invalid diegops config: {e}").into())
}

/// Loads config from `path`; a missing file yields defaults.
pub fn load(path: &Path) -> Result<Config, BoxError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).map_err(|e| format!("{}: {e}", path.display()).into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("could not read {}: {e}", path.display()).into()),
    }
}

/// `$DIEGOPS_CONFIG` > `$XDG_CONFIG_HOME/diegops/config.toml` > `~/.config/diegops/config.toml`.
pub fn default_path(env: &dyn Fn(&str) -> Option<String>, home: &Path) -> PathBuf {
    if let Some(p) = non_empty(env("DIEGOPS_CONFIG")) {
        return PathBuf::from(p);
    }
    let base = non_empty(env("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    base.join("diegops").join("config.toml")
}

/// Flag > `$DIEGOPS_SECRETS_BACKEND` > config `backend` > Infisical.
pub fn select_kind(
    cfg: &Config,
    flag: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<BackendKind, BoxError> {
    let (chosen, from) = if let Some(f) = flag {
        (Some(f.to_string()), "--backend")
    } else if let Some(e) = non_empty(env("DIEGOPS_SECRETS_BACKEND")) {
        (Some(e), "$DIEGOPS_SECRETS_BACKEND")
    } else if let Some(c) = cfg.backend.clone() {
        (Some(c), "config.toml")
    } else {
        (None, "default")
    };
    crate::debug!(
        "secrets backend: {} ({from})",
        chosen.as_deref().unwrap_or("infisical")
    );
    match chosen.as_deref().map(str::trim) {
        None | Some("infisical") => Ok(BackendKind::Infisical),
        Some("vault") | Some("openbao") => Ok(BackendKind::Vault),
        Some(other) => {
            Err(format!("unknown secrets backend '{other}' (use 'infisical' or 'vault')").into())
        }
    }
}

/// Resolves Vault settings (env `VAULT_*` overrides the file).
pub fn resolve_vault(
    cfg: &Config,
    env: &dyn Fn(&str) -> Option<String>,
    home: &Path,
) -> Result<VaultSettings, BoxError> {
    let v = &cfg.vault;
    let address = non_empty(env("VAULT_ADDR"))
        .or_else(|| v.address.clone())
        .ok_or(
            "no Vault address configured\n  hint: set address under [vault] in the config, or export VAULT_ADDR",
        )?;
    let namespace = non_empty(env("VAULT_NAMESPACE")).or_else(|| v.namespace.clone());
    let ca_cert = non_empty(env("VAULT_CACERT"))
        .or_else(|| v.ca_cert.clone())
        .map(|p| expand(&p, home));
    let token = match non_empty(env("VAULT_TOKEN")) {
        Some(t) => t,
        None => {
            let file = v
                .token_file
                .as_deref()
                .map(|p| expand(p, home))
                .unwrap_or_else(|| home.join(".vault-token"));
            let t = std::fs::read_to_string(&file).map_err(|_| {
                format!(
                    "no Vault token: set VAULT_TOKEN or log in (bao/vault login -method=oidc) to create {}",
                    file.display()
                )
            })?;
            t.trim().to_string()
        }
    };
    Ok(VaultSettings {
        address: address.trim_end_matches('/').to_string(),
        namespace,
        ca_cert,
        token,
    })
}

/// Where the backend credential comes from, mirroring [`resolve_infisical`]
/// and [`resolve_vault`] (ADR 0001 D6). Never returns the secret itself.
pub fn credential_source(
    cfg: &Config,
    kind: BackendKind,
    env: &dyn Fn(&str) -> Option<String>,
    home: &Path,
) -> String {
    match kind {
        BackendKind::Infisical => {
            let c = &cfg.infisical;
            if non_empty(env("INFISICAL_TOKEN")).is_some() {
                "$INFISICAL_TOKEN".into()
            } else if non_empty(env("INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET")).is_some() {
                "$INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET".into()
            } else if let Some(cmd) = c.client_secret_command.as_ref().filter(|c| !c.is_empty()) {
                format!("client_secret_command '{}'", cmd[0])
            } else if let Some(f) = c.client_secret_file.as_deref() {
                format!("client_secret_file {}", expand(f, home).display())
            } else {
                "none".into()
            }
        }
        BackendKind::Vault => {
            if non_empty(env("VAULT_TOKEN")).is_some() {
                "$VAULT_TOKEN".into()
            } else {
                let file = cfg
                    .vault
                    .token_file
                    .as_deref()
                    .map(|p| expand(p, home))
                    .unwrap_or_else(|| home.join(".vault-token"));
                format!("token file {}", file.display())
            }
        }
    }
}

/// Resolves Infisical settings (env `INFISICAL_*` overrides the file).
pub fn resolve_infisical(
    cfg: &Config,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<InfisicalSettings, BoxError> {
    let c = &cfg.infisical;
    let site_url = non_empty(env("INFISICAL_API_URL"))
        .or_else(|| c.site_url.clone())
        .unwrap_or_else(|| DEFAULT_INFISICAL_URL.to_string())
        .trim_end_matches('/')
        .trim_end_matches("/api")
        .to_string();
    let auth = if let Some(t) = non_empty(env("INFISICAL_TOKEN")) {
        InfisicalAuth::AccessToken(t)
    } else {
        let client_id = non_empty(env("INFISICAL_UNIVERSAL_AUTH_CLIENT_ID"))
            .or_else(|| c.client_id.clone())
            .ok_or(
                "no Infisical credentials found\n  \
                 hint: add client_id and client_secret_file under [infisical] in the config \
                 (see 'diegops explain machine-identity')\n  \
                 hint: or export INFISICAL_UNIVERSAL_AUTH_CLIENT_ID and \
                 INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET, or INFISICAL_TOKEN",
            )?;
        let client_secret = client_secret(c, env)?;
        InfisicalAuth::UniversalAuth {
            client_id,
            client_secret,
        }
    };
    Ok(InfisicalSettings {
        site_url,
        auth,
        mapping: Mapping::from_config(c),
    })
}

/// Client secret: env > command > private file.
fn client_secret(
    c: &InfisicalConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<String, BoxError> {
    if let Some(s) = non_empty(env("INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET")) {
        return Ok(s);
    }
    if let Some(cmd) = c.client_secret_command.as_ref().filter(|c| !c.is_empty()) {
        let out = std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .output()
            .map_err(|e| format!("client_secret_command '{}' failed to start: {e}", cmd[0]))?;
        if !out.status.success() {
            return Err(format!(
                "client_secret_command '{}' exited with {}",
                cmd[0], out.status
            )
            .into());
        }
        let s = String::from_utf8(out.stdout)
            .map_err(|_| "client_secret_command output is not UTF-8")?
            .trim()
            .to_string();
        if s.is_empty() {
            return Err("[infisical] client_secret_command printed nothing to stdout".into());
        }
        return Ok(s);
    }
    if let Some(f) = c.client_secret_file.as_deref() {
        let path = if f.starts_with("~/") {
            let home = non_empty(env("HOME")).ok_or_else(|| {
                format!("HOME is not set, so client_secret_file '{f}' cannot be found\n  hint: set HOME or use an absolute path")
            })?;
            expand(f, Path::new(&home))
        } else {
            PathBuf::from(f)
        };
        check_private(&path)?;
        let s = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        return Ok(s.trim().to_string());
    }
    Err("no Infisical client secret found\n  \
         hint: add client_secret_file (a file with mode 0600) or client_secret_command \
         under [infisical] in the config, or export INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET"
        .into())
}

/// Refuses secret files readable by group/others.
#[cfg(unix)]
fn check_private(path: &Path) -> Result<(), BoxError> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(format!(
            "{} must not be accessible by group/others (chmod 600)",
            path.display()
        )
        .into());
    }
    Ok(())
}

/// No POSIX permission bits on Windows.
#[cfg(not(unix))]
fn check_private(_path: &Path) -> Result<(), BoxError> {
    Ok(())
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.trim().is_empty())
}

pub fn expand(p: &str, home: &Path) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(p),
    }
}

/// Makes a folder segment valid for Infisical (`[A-Za-z0-9_-]`).
fn sanitise_segment(seg: &str) -> String {
    seg.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn join_folder(base: &str, rest: &[&str]) -> String {
    let mut parts: Vec<String> = base
        .split('/')
        .filter(|s| !s.is_empty())
        .map(sanitise_segment)
        .collect();
    parts.extend(
        rest.iter()
            .filter(|s| !s.is_empty())
            .map(|s| sanitise_segment(s)),
    );
    format!("/{}", parts.join("/"))
}

impl Mapping {
    /// Builds the mapping from the `[infisical]` table.
    pub fn from_config(cfg: &InfisicalConfig) -> Self {
        Self {
            default_project: cfg.project_id.clone(),
            default_env: cfg.environment.clone(),
            mounts: cfg.mounts.clone(),
        }
    }

    /// Maps `<mount>/<path>` to its Infisical location.
    pub fn locate(&self, path: &str) -> Result<Location, BoxError> {
        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let Some((mount, rest)) = segs.split_first() else {
            return Err("empty secret path".into());
        };
        let m = self.mounts.iter().find(|m| m.mount == *mount);
        let project_id = m
            .and_then(|m| m.project_id.clone())
            .or_else(|| self.default_project.clone())
            .ok_or_else(|| {
                format!("no Infisical project for mount '{mount}': set [infisical] project_id or a [[infisical.mounts]] entry")
            })?;
        let environment = m
            .and_then(|m| m.environment.clone())
            .or_else(|| self.default_env.clone())
            .unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string());
        let folder = match m {
            Some(m) => join_folder(m.folder.as_deref().unwrap_or(&format!("/{mount}")), rest),
            None => join_folder(&format!("/{mount}"), rest),
        };
        Ok(Location {
            project_id,
            environment,
            folder,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
