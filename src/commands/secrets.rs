//! Secrets file management — push, pull, and status for secret files across
//! local directories using a YAML config that maps Vault paths to destination
//! folders.
//!
//! Config file (`secrets.yaml`) format:
//! ```yaml
//! folders:
//!   - dest: $HOME/github/org/my-app
//!     vault_path: secret/my-app
//!     dir_mode: "0700"
//!     keys:
//!       - name: database.env
//!         mode: "0600"
//!       - name: api.env
//! ```
//!
//! Config resolution order:
//! 1. `--config <path>` CLI flag
//! 2. `DIEGOPS_SECRETS_CONFIG` environment variable
//! 3. `~/.diegops/secrets.yaml` (default)

use serde::Deserialize;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Clap sub-command definitions
// ---------------------------------------------------------------------------

/// Secrets file management sub-commands.
#[derive(clap::Subcommand)]
pub enum SecretsCommand {
    /// Push local secret files to the configured destinations.
    Push {
        /// Only process folders whose expanded dest starts with this prefix.
        #[arg(long)]
        path: Option<String>,

        /// Path to the secrets.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Pull secret files from the configured sources into local destinations.
    Pull {
        /// Only process folders whose expanded dest starts with this prefix.
        #[arg(long)]
        path: Option<String>,

        /// Path to the secrets.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Show the sync status of secret files for each configured folder.
    Status {
        /// Path to the secrets.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Create a sample secrets.yaml in ~/.diegops/ to get started.
    ///
    /// Does nothing if the file already exists (idempotent).
    Init,
}

// ---------------------------------------------------------------------------
// Config structures
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SecretsConfig {
    folders: Vec<Folder>,
}

#[derive(Deserialize)]
struct Folder {
    dest: String,
    vault_path: String,
    keys: KeySelector,
    dir_mode: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum KeySelector {
    All(String),
    List(Vec<KeyEntry>),
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum KeyEntry {
    WithMode { name: String, mode: Option<String> },
    Simple(String),
}

impl KeyEntry {
    /// Returns the key name (file name).
    fn name(&self) -> &str {
        match self {
            KeyEntry::WithMode { name, .. } => name.as_str(),
            KeyEntry::Simple(s) => s.as_str(),
        }
    }

    /// Returns the explicit mode string if one was set, or `None`.
    fn mode(&self) -> Option<&str> {
        match self {
            KeyEntry::WithMode { mode, .. } => mode.as_deref(),
            KeyEntry::Simple(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Permission helpers
// ---------------------------------------------------------------------------

/// Returns the default mode for a given filename.
///
/// Files ending in `.env` or that are named exactly `*.key`, `*.pem`, or
/// `*.crt` default to `0o600`; everything else defaults to `0o644`.
fn default_mode(filename: &str) -> u32 {
    let lower = filename.to_lowercase();
    if lower.ends_with(".env")
        || lower.ends_with(".key")
        || lower.ends_with(".pem")
        || lower.ends_with(".crt")
    {
        0o600
    } else {
        0o644
    }
}

/// Parses a mode string such as `"0600"` or `"600"` into a `u32`.
///
/// The string is interpreted as an octal number. Returns an error if the
/// string is not a valid octal mode.
fn parse_mode(mode_str: &str) -> Result<u32, Box<dyn std::error::Error>> {
    u32::from_str_radix(mode_str.trim_start_matches('0'), 8)
        .map_err(|e| format!("invalid mode '{}': {e}", mode_str).into())
        .and_then(|v| {
            // Re-parse with leading zero stripped; re-add the octal prefix
            // by parsing the whole string directly.
            Ok(v)
        })
}

/// Returns the effective octal mode for a `KeyEntry`.
///
/// Resolution order:
/// 1. Explicit `mode` field on the entry (parsed as octal)
/// 2. `default_mode(name)`
fn effective_mode(entry: &KeyEntry) -> Result<u32, Box<dyn std::error::Error>> {
    match entry.mode() {
        Some(m) => parse_mode(m),
        None => Ok(default_mode(entry.name())),
    }
}

/// Returns the effective directory mode for a `Folder`.
///
/// Resolution order:
/// 1. Explicit `dir_mode` field on the folder (parsed as octal)
/// 2. Default `0o755`
fn dir_mode(folder: &Folder) -> Result<u32, Box<dyn std::error::Error>> {
    match &folder.dir_mode {
        Some(m) => parse_mode(m),
        None => Ok(0o755),
    }
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Loads and validates the secrets config from the resolved path.
fn load_config(path: Option<&Path>) -> Result<SecretsConfig, Box<dyn std::error::Error>> {
    let config_path = super::common::resolve_config_path(
        path,
        "DIEGOPS_SECRETS_CONFIG",
        "secrets.yaml",
    )?;
    let content = super::common::read_config_file(&config_path)?;
    let config: SecretsConfig = serde_yaml::from_str(&content)
        .map_err(|e| format!("invalid secrets config {}: {e}", config_path.display()))?;

    // Validate KeySelector::All values — must be exactly "*"
    for folder in &config.folders {
        if let KeySelector::All(ref s) = folder.keys {
            if s != "*" {
                return Err(format!(
                    "invalid key selector '{}' for vault_path '{}' — use \"*\" or a list of key names",
                    s, folder.vault_path
                )
                .into());
            }
        }
    }

    Ok(config)
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Creates `~/.diegops/secrets.yaml` with a sample config.
///
/// Idempotent: if the file already exists it prints its location and exits 0.
/// If `base_dir` is `Some`, writes config there instead of `~/.diegops/`.
pub fn init(base_dir: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = match base_dir {
        Some(d) => d.to_owned(),
        None => super::common::diegops_dir()?,
    };
    let config_path = dir.join("secrets.yaml");

    if config_path.exists() {
        println!("Config already exists: {}", config_path.display());
        println!("Edit it directly or run `diegops secrets status` to see sync state.");
        return Ok(());
    }

    fs::create_dir_all(&dir)?;
    fs::write(&config_path, SAMPLE_SECRETS_CONFIG)?;

    println!("Created: {}", config_path.display());
    println!("Edit the file to add your secrets folders, then run `diegops secrets push`.");
    Ok(())
}

const SAMPLE_SECRETS_CONFIG: &str = "\
# diegops secrets configuration
#
# Each folder maps a local destination directory to a Vault secret path.
# Run `diegops secrets push` to write secret files.
# Run `diegops secrets status` to see the current sync state.
#
# Path rules:
#   - Use $HOME as a portable prefix (works on Linux, macOS, and WSL).
#   - vault_path uses the logical Vault path (no /data/ segment).
#   - keys: list specific file names, or use \"*\" to use all keys.
#   - dir_mode sets the directory permissions (default: \"0755\").
#   - mode per key sets the file permissions (default: \"0600\" for .env/.key/.pem/.crt, else \"0644\").

folders:

  - dest: $HOME/github/my-org/products/my-product/secrets
    vault_path: secret/my-product
    dir_mode: \"0700\"
    keys:
      - name: database.env
        mode: \"0600\"
      - name: api.env
";

/// Push secret files — stub implementation.
pub fn push(
    _config_path: Option<&Path>,
    _path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("not yet implemented".into())
}

/// Pull secret files — stub implementation.
pub fn pull(
    _config_path: Option<&Path>,
    _path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("not yet implemented".into())
}

/// Show sync status — stub implementation.
pub fn status(_config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    Err("not yet implemented".into())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // -----------------------------------------------------------------------
    // Config parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_config_with_explicit_keys() {
        let yaml = r#"
folders:
  - dest: $HOME/github/org/app/secrets
    vault_path: secret/app
    keys:
      - name: database.env
      - name: api.env
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.folders.len(), 1);
        match &config.folders[0].keys {
            KeySelector::List(entries) => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].name(), "database.env");
                assert_eq!(entries[1].name(), "api.env");
            }
            KeySelector::All(_) => panic!("expected List"),
        }
    }

    #[test]
    fn parse_config_with_wildcard() {
        let yaml = r#"
folders:
  - dest: $HOME/github/org/app/secrets
    vault_path: secret/app
    keys: "*"
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.folders[0].keys {
            KeySelector::All(s) => assert_eq!(s, "*"),
            KeySelector::List(_) => panic!("expected All"),
        }
    }

    #[test]
    fn parse_config_with_mixed_mode_keys() {
        let yaml = r#"
folders:
  - dest: $HOME/github/org/app/secrets
    vault_path: secret/app
    keys:
      - name: database.env
        mode: "0600"
      - name: readme.txt
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.folders[0].keys {
            KeySelector::List(entries) => {
                assert_eq!(entries[0].name(), "database.env");
                assert_eq!(entries[0].mode(), Some("0600"));
                assert_eq!(entries[1].name(), "readme.txt");
                assert_eq!(entries[1].mode(), None);
            }
            KeySelector::All(_) => panic!("expected List"),
        }
    }

    #[test]
    fn parse_config_with_no_dir_mode() {
        let yaml = r#"
folders:
  - dest: $HOME/github/org/app/secrets
    vault_path: secret/app
    keys:
      - name: database.env
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.folders[0].dir_mode.is_none());
    }

    #[test]
    fn parse_config_with_dir_mode() {
        let yaml = r#"
folders:
  - dest: $HOME/github/org/app/secrets
    vault_path: secret/app
    dir_mode: "0700"
    keys:
      - name: database.env
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.folders[0].dir_mode.as_deref(), Some("0700"));
    }

    // -----------------------------------------------------------------------
    // default_mode
    // -----------------------------------------------------------------------

    #[test]
    fn default_mode_env_files() {
        assert_eq!(default_mode("database.env"), 0o600);
        assert_eq!(default_mode("api.env"), 0o600);
        assert_eq!(default_mode(".env"), 0o600);
    }

    #[test]
    fn default_mode_key_pem_crt_files() {
        assert_eq!(default_mode("id_rsa.key"), 0o600);
        assert_eq!(default_mode("server.pem"), 0o600);
        assert_eq!(default_mode("server.crt"), 0o600);
    }

    #[test]
    fn default_mode_other_files() {
        assert_eq!(default_mode("readme.txt"), 0o644);
        assert_eq!(default_mode("config.yaml"), 0o644);
        assert_eq!(default_mode("Makefile"), 0o644);
    }

    // -----------------------------------------------------------------------
    // parse_mode
    // -----------------------------------------------------------------------

    #[test]
    fn parse_mode_with_leading_zero() {
        assert_eq!(parse_mode("0600").unwrap(), 0o600);
        assert_eq!(parse_mode("0755").unwrap(), 0o755);
        assert_eq!(parse_mode("0700").unwrap(), 0o700);
        assert_eq!(parse_mode("0644").unwrap(), 0o644);
    }

    #[test]
    fn parse_mode_without_leading_zero() {
        assert_eq!(parse_mode("600").unwrap(), 0o600);
        assert_eq!(parse_mode("755").unwrap(), 0o755);
    }

    #[test]
    fn parse_mode_invalid_returns_error() {
        assert!(parse_mode("not-a-mode").is_err());
        assert!(parse_mode("9999").is_err()); // 9 is not octal
    }

    // -----------------------------------------------------------------------
    // effective_mode
    // -----------------------------------------------------------------------

    #[test]
    fn effective_mode_uses_explicit_mode() {
        let entry = KeyEntry::WithMode {
            name: "readme.txt".to_string(),
            mode: Some("0600".to_string()),
        };
        assert_eq!(effective_mode(&entry).unwrap(), 0o600);
    }

    #[test]
    fn effective_mode_falls_back_to_default_for_env() {
        let entry = KeyEntry::Simple("database.env".to_string());
        assert_eq!(effective_mode(&entry).unwrap(), 0o600);
    }

    #[test]
    fn effective_mode_falls_back_to_default_for_txt() {
        let entry = KeyEntry::Simple("readme.txt".to_string());
        assert_eq!(effective_mode(&entry).unwrap(), 0o644);
    }

    #[test]
    fn effective_mode_with_no_mode_field() {
        let entry = KeyEntry::WithMode {
            name: "config.yaml".to_string(),
            mode: None,
        };
        assert_eq!(effective_mode(&entry).unwrap(), 0o644);
    }

    // -----------------------------------------------------------------------
    // dir_mode
    // -----------------------------------------------------------------------

    #[test]
    fn dir_mode_uses_explicit_value() {
        let folder = Folder {
            dest: "$HOME/secrets".to_string(),
            vault_path: "secret/app".to_string(),
            keys: KeySelector::List(vec![]),
            dir_mode: Some("0700".to_string()),
        };
        assert_eq!(dir_mode(&folder).unwrap(), 0o700);
    }

    #[test]
    fn dir_mode_defaults_to_0755() {
        let folder = Folder {
            dest: "$HOME/secrets".to_string(),
            vault_path: "secret/app".to_string(),
            keys: KeySelector::List(vec![]),
            dir_mode: None,
        };
        assert_eq!(dir_mode(&folder).unwrap(), 0o755);
    }

    // -----------------------------------------------------------------------
    // init
    // -----------------------------------------------------------------------

    #[test]
    fn init_creates_sample_config() {
        let dir = std::env::temp_dir()
            .join(format!("diegops-test-secrets-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Use explicit base_dir to avoid mutating HOME
        init(Some(&dir)).unwrap();

        let config_path = dir.join("secrets.yaml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("folders:"));
        assert!(content.contains("vault_path:"));
        assert!(content.contains("keys:"));

        // Idempotent: second call succeeds without error
        init(Some(&dir)).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}
