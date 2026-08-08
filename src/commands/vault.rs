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
    config_path: Option<&Path>,
    path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Pre-flight checks — fatal, exit immediately on failure
    super::common::check_vault_binary()?;
    super::common::check_vault_addr()?;
    super::common::check_vault_auth()?;

    let config = load_config(config_path)?;
    let filter = path_filter.map(super::common::expand_home);

    let mut n_written: usize = 0;
    let mut n_skipped: usize = 0;
    let mut failures: Vec<String> = Vec::new();

    for target in &config.targets {
        let dest = super::common::expand_home(&target.path);

        if let Some(ref f) = filter {
            if !dest.starts_with(f) {
                continue;
            }
        }

        eprintln!("\n{}", target.path);

        if !dest.exists() {
            eprintln!(
                "  FAIL  target directory '{}' does not exist. Clone the repo first",
                dest.display()
            );
            failures.push(format!("{} (directory missing)", target.path));
            continue;
        }

        // Collect secrets for this target
        let mut all_secrets: Vec<(String, String)> = Vec::new();
        let mut target_failed = false;

        for entry in &target.secrets {
            match super::common::vault_kv_get(&entry.vault_path) {
                Ok(data) => match &entry.keys {
                    KeySelector::All(s) if s == "*" => {
                        for (k, v) in &data {
                            all_secrets.push((
                                k.clone(),
                                v.as_str().unwrap_or(&v.to_string()).to_string(),
                            ));
                        }
                    }
                    KeySelector::All(s) => {
                        eprintln!("  FAIL  invalid key selector '{}' (expected \"*\")", s);
                        target_failed = true;
                        break;
                    }
                    KeySelector::List(keys) => {
                        let available: Vec<&String> = data.keys().collect();
                        for key in keys {
                            match data.get(key) {
                                Some(v) => {
                                    all_secrets.push((
                                        key.clone(),
                                        v.as_str().unwrap_or(&v.to_string()).to_string(),
                                    ));
                                }
                                None => {
                                    eprintln!(
                                        "  FAIL  key '{}' not found at '{}'. Available keys: {:?}",
                                        key, entry.vault_path, available
                                    );
                                    target_failed = true;
                                    break;
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    eprintln!("  FAIL  {e}");
                    target_failed = true;
                }
            }
            if target_failed {
                break;
            }
        }

        if target_failed {
            failures.push(target.path.clone());
            continue;
        }

        // Build .env content and write it (backing up an existing, differing
        // file first — matches secrets.rs's write_file_with_backup behavior).
        let env_content = build_env_content(&all_secrets);
        match write_env_file(&dest, &env_content) {
            Ok((written, backed_up)) => {
                if !written {
                    eprintln!("  SKIP  .env unchanged");
                    n_skipped += 1;
                } else if backed_up {
                    eprintln!(
                        "  WRITE .env ({} secrets, backup created)",
                        all_secrets.len()
                    );
                    n_written += 1;
                } else {
                    eprintln!("  WRITE .env ({} secrets)", all_secrets.len());
                    n_written += 1;
                }
            }
            Err(e) => {
                eprintln!("  FAIL  could not write .env: {e}");
                failures.push(target.path.clone());
                continue;
            }
        }
    }

    eprintln!();
    println!(
        "Done: {n_written} written, {n_skipped} unchanged, {} failed",
        failures.len()
    );

    if !failures.is_empty() {
        eprintln!("\nFailed targets:");
        for f in &failures {
            eprintln!("  {f}");
        }
        return Err(format!("{} target(s) failed", failures.len()).into());
    }

    Ok(())
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
// Internal helpers
// ---------------------------------------------------------------------------

/// Ensures `.env` and `.env.bak` are both listed in the target directory's
/// `.gitignore`. `.env.bak` is needed because `write_env_file` may create one
/// when overwriting a changed `.env`.
fn ensure_gitignore(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let gitignore = dir.join(".gitignore");
    const REQUIRED: [&str; 2] = [".env", ".env.bak"];

    let content = if gitignore.exists() {
        fs::read_to_string(&gitignore)?
    } else {
        String::new()
    };

    let existing: Vec<&str> = content.lines().map(str::trim).collect();
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|pattern| !existing.contains(pattern))
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    let prefix = if content.is_empty() || content.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let addition: String = missing.iter().map(|p| format!("{p}\n")).collect();
    fs::write(&gitignore, format!("{content}{prefix}{addition}"))?;

    Ok(())
}

/// Builds the `.env` file content from a list of (key, value) pairs.
fn build_env_content(secrets: &[(String, String)]) -> String {
    let mut content = String::from("# Generated by diegops vault apply — do not edit\n");
    for (key, value) in secrets {
        content.push_str(&format_env_line(key, value));
        content.push('\n');
    }
    content
}

/// Formats a single `.env` line with double-quoted, escaped value.
fn format_env_line(key: &str, value: &str) -> String {
    let escaped = value.replace('\\', r"\\").replace('"', r#"\""#);
    format!("{key}=\"{escaped}\"")
}

/// Writes `env_content` to `dest/.env`, backing up an existing file first if
/// its content differs. Sets `0600` permissions and ensures `.env` is
/// gitignored — but only when something was actually written.
///
/// Returns `(written, backed_up)`.
fn write_env_file(
    dest: &Path,
    env_content: &str,
) -> Result<(bool, bool), Box<dyn std::error::Error>> {
    let env_path = dest.join(".env");
    let file_existed = env_path.exists();
    let backed_up = super::common::write_file_with_backup(&env_path, env_content.as_bytes())?;
    let written = backed_up || !file_existed;

    if written {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&env_path, fs::Permissions::from_mode(0o600))?;
        }
        ensure_gitignore(dest)?;
    }

    Ok((written, backed_up))
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
    fn ensure_gitignore_adds_env_and_env_bak_entries() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-gitignore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // No .gitignore exists — should create one with both entries
        ensure_gitignore(&dir).unwrap();
        let content = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(content.lines().any(|l| l.trim() == ".env"));
        assert!(content.lines().any(|l| l.trim() == ".env.bak"));

        // Already present — should not duplicate
        ensure_gitignore(&dir).unwrap();
        let content = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(content.lines().filter(|l| l.trim() == ".env").count(), 1);
        assert_eq!(
            content.lines().filter(|l| l.trim() == ".env.bak").count(),
            1
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_gitignore_migrates_existing_env_only_entry() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-gitignore-migrate-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Simulates a repo that already ran an older `vault apply` before this fix.
        fs::write(dir.join(".gitignore"), ".env\n").unwrap();
        ensure_gitignore(&dir).unwrap();
        let content = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(content.lines().any(|l| l.trim() == ".env"));
        assert!(content.lines().any(|l| l.trim() == ".env.bak"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_gitignore_handles_missing_trailing_newline() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-gitignore-newline-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join(".gitignore"), "node_modules").unwrap();
        ensure_gitignore(&dir).unwrap();
        let content = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(content.starts_with("node_modules\n.env"));
        assert!(content.lines().any(|l| l.trim() == ".env.bak"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_env_content_formats_correctly() {
        let mut secrets = Vec::new();
        secrets.push(("db_pass".to_string(), "s3cret".to_string()));
        secrets.push(("api_key".to_string(), "tok \"en".to_string()));

        let content = build_env_content(&secrets);
        assert!(content.starts_with("# Generated by diegops vault apply"));
        assert!(content.contains(r#"db_pass="s3cret""#));
        assert!(content.contains(r#"api_key="tok \"en""#));
    }

    #[test]
    fn write_env_file_creates_backup_when_content_differs() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-vault-env-backup-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join(".env"), "OLD=\"value\"\n").unwrap();

        let (written, backed_up) = write_env_file(&dir, "NEW=\"value\"\n").unwrap();
        assert!(written);
        assert!(backed_up);
        assert_eq!(
            fs::read_to_string(dir.join(".env.bak")).unwrap(),
            "OLD=\"value\"\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join(".env")).unwrap(),
            "NEW=\"value\"\n"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_env_file_skips_when_unchanged() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-vault-env-unchanged-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join(".env"), "SAME=\"value\"\n").unwrap();
        let (written, backed_up) = write_env_file(&dir, "SAME=\"value\"\n").unwrap();

        assert!(!written);
        assert!(!backed_up);
        assert!(!dir.join(".env.bak").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_env_file_writes_new_file_without_backup() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-vault-env-new-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let (written, backed_up) = write_env_file(&dir, "FRESH=\"value\"\n").unwrap();

        assert!(written);
        assert!(!backed_up);
        assert!(!dir.join(".env.bak").exists());
        assert_eq!(
            fs::read_to_string(dir.join(".env")).unwrap(),
            "FRESH=\"value\"\n"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
