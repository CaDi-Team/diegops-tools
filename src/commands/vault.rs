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

#[allow(dead_code)]
#[derive(Deserialize)]
struct VaultConfig {
    targets: Vec<Target>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Target {
    path: String,
    secrets: Vec<SecretEntry>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct SecretEntry {
    vault_path: String,
    keys: KeySelector,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(untagged)]
enum KeySelector {
    All(String),
    List(Vec<String>),
}

// ---------------------------------------------------------------------------
// Public entry points (stubs)
// ---------------------------------------------------------------------------

/// Pulls secrets from Vault and writes `.env` files for each target.
pub fn apply(
    _config_path: Option<&std::path::Path>,
    _path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("vault apply")
}

/// Lists targets that already have a `.env` file.
pub fn list(_config_path: Option<&std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    todo!("vault list")
}

/// Lists targets in config that do NOT have a `.env` file.
pub fn list_diff(_config_path: Option<&std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    todo!("vault list-diff")
}

/// Creates `~/.diegops/repo-vault.yaml` with a sample config.
///
/// If `base_dir` is `Some`, writes config under that directory instead of `~/.diegops/`.
/// This parameter exists for testability — production code passes `None`.
pub fn init(_base_dir: Option<&std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    todo!("vault init")
}
