//! Vault secret management — pull secrets from HashiCorp Vault KV v2 and write `.env` files.
//!
//! Config file (`repo-vault.yaml`) format:
//! ```yaml
//! targets:
//!   - path: $HOME/github/org/my-app
//!     secrets:
//!       - vault_path: secret/my-app/database
//!         keys:
//!           - password
//!           - username
//!       - vault_path: secret/my-app/api
//!         keys: "*"
//! ```
//!
//! Config resolution order:
//! 1. `--config <path>` CLI flag
//! 2. `DIEGOPS_VAULT_CONFIG` environment variable
//! 3. `~/.diegops/repo-vault.yaml` (default)

use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// Clap sub-command definitions
// ---------------------------------------------------------------------------

/// Vault secret management sub-commands.
#[derive(clap::Subcommand)]
pub enum VaultCommand {
    /// Pull secrets from Vault and write .env files. Existing .env files are overwritten.
    Apply {
        /// Only process targets whose expanded path starts with this prefix.
        #[arg(long)]
        path: Option<String>,

        /// Path to the repo-vault.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// List targets that already have a .env file locally.
    List {
        /// Path to the repo-vault.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Show targets in config that do NOT have a .env file locally.
    #[command(name = "list-diff")]
    ListDiff {
        /// Path to the repo-vault.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Create a sample repo-vault.yaml in ~/.diegops/ to get started.
    ///
    /// Does nothing if the file already exists (idempotent).
    Init,
}

// ---------------------------------------------------------------------------
// Config structures
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct VaultConfig {
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    path: String,
    secrets: Vec<SecretEntry>,
}

#[derive(Deserialize)]
struct SecretEntry {
    vault_path: String,
    keys: KeySelector,
}

#[derive(Deserialize)]
#[serde(untagged)]
#[allow(dead_code)] // inner fields used by `apply` (next task) and tests
enum KeySelector {
    All(String),
    List(Vec<String>),
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Loads and validates the vault config from the resolved path.
fn load_config(path: Option<&Path>) -> Result<VaultConfig, Box<dyn std::error::Error>> {
    let config_path =
        super::common::resolve_config_path(path, "DIEGOPS_VAULT_CONFIG", "repo-vault.yaml")?;
    let content = super::common::read_config_file(&config_path)?;
    let config: VaultConfig = serde_yaml::from_str(&content)
        .map_err(|e| format!("invalid vault config {}: {e}", config_path.display()))?;

    // Validate KeySelector::All values — must be exactly "*"
    for target in &config.targets {
        for entry in &target.secrets {
            if let KeySelector::All(ref s) = entry.keys {
                if s != "*" {
                    return Err(format!(
                        "invalid key selector '{}' for vault_path '{}' — use \"*\" or a list of key names",
                        s, entry.vault_path
                    )
                    .into());
                }
            }
        }
    }

    Ok(config)
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Pulls secrets from Vault and writes `.env` files for each target.
pub fn apply(
    _config_path: Option<&std::path::Path>,
    _path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("vault apply")
}

/// Lists targets that already have a `.env` file.
pub fn list(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(config_path)?;
    let mut total: usize = 0;

    for target in &config.targets {
        let dest = super::common::expand_home(&target.path);
        if dest.join(".env").exists() {
            println!("{}", target.path);
            total += 1;
        }
    }

    eprintln!("{total} targets with .env files.");
    Ok(())
}

/// Lists targets in config that do NOT have a `.env` file.
pub fn list_diff(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(config_path)?;
    let mut total: usize = 0;

    for target in &config.targets {
        let dest = super::common::expand_home(&target.path);
        if !dest.join(".env").exists() {
            println!("{}", target.path);
            total += 1;
        }
    }

    if total == 0 {
        println!("All targets have .env files.");
    } else {
        eprintln!(
            "{total} targets missing .env files. Run `diegops vault apply` to generate them."
        );
    }

    Ok(())
}

/// Creates `~/.diegops/repo-vault.yaml` with a sample config.
///
/// Idempotent: if the file already exists it prints its location and exits 0.
/// If `base_dir` is `Some`, writes config there instead of `~/.diegops/`.
pub fn init(base_dir: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = match base_dir {
        Some(d) => d.to_owned(),
        None => super::common::diegops_dir()?,
    };
    let config_path = dir.join("repo-vault.yaml");

    if config_path.exists() {
        println!("Config already exists: {}", config_path.display());
        println!("Edit it directly or run `diegops vault list-diff` to see missing targets.");
        return Ok(());
    }

    fs::create_dir_all(&dir)?;
    fs::write(&config_path, SAMPLE_VAULT_CONFIG)?;

    println!("Created: {}", config_path.display());
    println!("Edit the file to add your Vault secrets, then run `diegops vault apply`.");
    Ok(())
}

const SAMPLE_VAULT_CONFIG: &str = "\
# diegops Vault secret configuration
#
# Each target maps a local directory to Vault secret paths.
# Run `diegops vault apply` to pull secrets and write .env files.
# Run `diegops vault list-diff` to see targets missing a .env file.
#
# Path rules:
#   - Use $HOME as a portable prefix (works on Linux, macOS, and WSL).
#   - vault_path uses the logical Vault path (no /data/ segment).
#   - keys: list specific keys, or use \"*\" to pull all keys.
#   - The vault CLI must be installed and authenticated before running apply.

targets:

  - path: $HOME/github/my-org/products/my-product
    secrets:
      - vault_path: secret/my-product/database
        keys:
          - username
          - password
      - vault_path: secret/my-product/api
        keys: \"*\"
";

// ---------------------------------------------------------------------------
// Internal helpers — used by `apply` (next task)
// ---------------------------------------------------------------------------

/// Formats a single `.env` line with double-quoted, escaped value.
#[allow(dead_code)] // used by `apply` (next task)
fn format_env_line(key: &str, value: &str) -> String {
    let escaped = value.replace('\\', r"\\").replace('"', r#"\""#);
    format!("{key}=\"{escaped}\"")
}

/// Checks that the `vault` CLI is available on PATH.
#[allow(dead_code)] // used by `apply` (next task)
fn check_vault_binary() -> Result<(), Box<dyn std::error::Error>> {
    match Command::new("vault").arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => Err("vault CLI found but returned an error. Check your installation.".into()),
        Err(_) => Err(
            "vault CLI not found on PATH. Install it from https://developer.hashicorp.com/vault/install"
                .into(),
        ),
    }
}

/// Checks that VAULT_ADDR is set.
#[allow(dead_code)] // used by `apply` (next task)
fn check_vault_addr() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("VAULT_ADDR").is_err() {
        return Err("VAULT_ADDR is not set. Export it or configure your Vault client".into());
    }
    Ok(())
}

/// Checks that the current Vault token is valid.
#[allow(dead_code)] // used by `apply` (next task)
fn check_vault_auth() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("vault")
        .args(["token", "lookup"])
        .output()
        .map_err(|e| format!("could not run vault: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err("vault is not authenticated. Run 'vault login' first".into())
    }
}

/// Fetches all key-value pairs from a Vault KV v2 path.
///
/// Returns a map of key → value. On error, returns a user-friendly message.
#[allow(dead_code)] // used by `apply` (next task)
fn vault_kv_get(
    vault_path: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let output = Command::new("vault")
        .args(["kv", "get", "-format=json", vault_path])
        .output()
        .map_err(|e| format!("could not run vault: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.contains("permission denied") || stderr.contains("403") {
            return Err(
                format!("access denied for '{vault_path}'. Check your Vault policies").into(),
            );
        }
        if stderr.contains("no secrets") || stderr.contains("Not Found") || stderr.contains("404") {
            return Err(format!(
                "secret not found at '{vault_path}'. Verify the path exists in Vault"
            )
            .into());
        }
        return Err(format!("vault kv get failed for '{vault_path}': {stderr}").into());
    }

    parse_vault_response(&output.stdout, vault_path)
}

/// Parses the JSON response from `vault kv get -format=json`.
///
/// Extracted for testability — the JSON structure is `{ "data": { "data": { ... } } }`.
#[allow(dead_code)] // used by `vault_kv_get` above and tests
fn parse_vault_response(
    json_bytes: &[u8],
    vault_path: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let json: serde_json::Value = serde_json::from_slice(json_bytes)?;
    let data = json
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(|d| d.as_object())
        .ok_or_else(|| format!("unexpected JSON structure from vault kv get for '{vault_path}'"))?;

    Ok(data.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn init_creates_sample_config() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-vault-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Use explicit base_dir to avoid mutating HOME
        init(Some(&dir)).unwrap();

        let config_path = dir.join("repo-vault.yaml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("targets:"));
        assert!(content.contains("vault_path:"));
        assert!(content.contains("keys:"));

        // Idempotent: second call succeeds
        init(Some(&dir)).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_config_with_explicit_keys() {
        let yaml = r#"
targets:
  - path: $HOME/github/org/app
    secrets:
      - vault_path: secret/app/db
        keys:
          - username
          - password
"#;
        let config: VaultConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].secrets.len(), 1);
        match &config.targets[0].secrets[0].keys {
            KeySelector::List(keys) => assert_eq!(keys, &["username", "password"]),
            KeySelector::All(_) => panic!("expected List"),
        }
    }

    #[test]
    fn parse_config_with_wildcard_keys() {
        let yaml = r#"
targets:
  - path: $HOME/github/org/app
    secrets:
      - vault_path: secret/app/api
        keys: "*"
"#;
        let config: VaultConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.targets[0].secrets[0].keys {
            KeySelector::All(s) => assert_eq!(s, "*"),
            KeySelector::List(_) => panic!("expected All"),
        }
    }

    #[test]
    fn format_env_line_escapes_quotes_and_backslashes() {
        assert_eq!(format_env_line("key", "simple"), "key=\"simple\"");
        assert_eq!(
            format_env_line("key", r#"has "quotes""#),
            r#"key="has \"quotes\"""#
        );
        assert_eq!(
            format_env_line("key", r"back\slash"),
            r#"key="back\\slash""#
        );
        assert_eq!(format_env_line("key", "has spaces"), r#"key="has spaces""#);
    }

    #[test]
    fn parse_vault_response_extracts_data() {
        let json = br#"{
            "data": {
                "data": {
                    "username": "admin",
                    "password": "s3cret",
                    "port": 5432
                },
                "metadata": { "version": 1 }
            }
        }"#;
        let data = parse_vault_response(json, "secret/test").unwrap();
        assert_eq!(data.get("username").unwrap().as_str().unwrap(), "admin");
        assert_eq!(data.get("password").unwrap().as_str().unwrap(), "s3cret");
        assert_eq!(data.get("port").unwrap().as_u64().unwrap(), 5432);
        assert_eq!(data.len(), 3);
    }

    #[test]
    fn parse_vault_response_rejects_bad_json() {
        let json = br#"{"data": {"wrong": "shape"}}"#;
        assert!(parse_vault_response(json, "secret/test").is_err());
    }
}
