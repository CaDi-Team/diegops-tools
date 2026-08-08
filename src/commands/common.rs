//! Shared helpers used across command modules.

use std::path::{Path, PathBuf};
use std::{fs, io};

/// Returns the current user's home directory.
///
/// Checks `$HOME` first (Unix convention), then `$USERPROFILE` (Windows convention).
pub fn home_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| "home directory not set ($HOME / $USERPROFILE)".into())
}

/// Returns `~/.diegops/`.
pub fn diegops_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(home_dir()?.join(".diegops"))
}

/// Expands a `$HOME`-prefixed path string to an absolute `PathBuf`.
///
/// Uses `.join()` per component — never string concatenation — so the result
/// is always a valid `PathBuf` regardless of platform path separator.
///
/// Also handles the `/$HOME/...` form (leading slash before `$HOME`).
pub fn expand_home(path: &str) -> PathBuf {
    let path = if path.starts_with("/$HOME") {
        &path[1..]
    } else {
        path
    };

    if let Some(rest) = path.strip_prefix("$HOME") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_default();

        let relative = rest.trim_start_matches('/');
        if relative.is_empty() {
            return home;
        }
        let mut p = home;
        for component in relative.split('/') {
            if !component.is_empty() {
                p = p.join(component);
            }
        }
        p
    } else {
        PathBuf::from(path)
    }
}

/// Resolves a config file path from: CLI flag > env var > default.
///
/// `cli_path` is the `--config` flag value, `env_var` is the env var name
/// (e.g. `DIEGOPS_REPOS_CONFIG`), and `default_filename` is the YAML file
/// name inside `~/.diegops/`.
pub fn resolve_config_path(
    cli_path: Option<&Path>,
    env_var: &str,
    default_filename: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match cli_path {
        Some(p) => Ok(p.to_owned()),
        None => {
            if let Ok(env_path) = std::env::var(env_var) {
                Ok(PathBuf::from(env_path))
            } else {
                Ok(diegops_dir()?.join(default_filename))
            }
        }
    }
}

/// Reads a config file and returns its content as a string.
///
/// Provides user-friendly errors for missing files.
pub fn read_config_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    fs::read_to_string(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            format!(
                "config not found: {}\n  Hint: create it or pass --config <path>",
                path.display()
            )
            .into()
        } else {
            format!("could not read {}: {e}", path.display()).into()
        }
    })
}

/// Checks that the `vault` CLI is available on PATH.
pub fn check_vault_binary() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
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
pub fn check_vault_addr() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("VAULT_ADDR").is_err() {
        return Err("VAULT_ADDR is not set. Export it or configure your Vault client".into());
    }
    Ok(())
}

/// Checks that the current Vault token is valid.
pub fn check_vault_auth() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
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

// ---------------------------------------------------------------------------
// Vault KV helpers (shared by secrets.rs and vault.rs)
// ---------------------------------------------------------------------------

/// Fetches all key-value pairs from a Vault KV v2 path.
///
/// Returns a map of key → value. On error, returns a user-friendly message.
pub fn vault_kv_get(
    vault_path: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    use std::process::Command;
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

    parse_vault_kv_response(&output.stdout, vault_path)
}

/// Parses the JSON response from `vault kv get -format=json`.
///
/// Extracted for testability — the JSON structure is `{ "data": { "data": { ... } } }`.
fn parse_vault_kv_response(
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

/// Writes `content` to `path`, creating a `.bak` backup if the file already
/// exists and its content differs.
///
/// Returns `true` if a backup was created (i.e. the file existed and differed),
/// `false` otherwise (new file, or content identical).
pub fn write_file_with_backup(
    path: &Path,
    content: &[u8],
) -> Result<bool, Box<dyn std::error::Error>> {
    if path.exists() {
        let existing = fs::read(path)?;
        if existing == content {
            return Ok(false);
        }
        // Content differs — back up before overwriting
        let bak = path.with_extension(
            path.extension()
                .map(|e| format!("{}.bak", e.to_string_lossy()))
                .unwrap_or_else(|| "bak".to_string()),
        );
        fs::copy(path, &bak)?;
        fs::write(path, content)?;
        return Ok(true);
    }
    // New file
    fs::write(path, content)?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_non_empty_path() {
        let home = home_dir().expect("home_dir should succeed");
        assert!(!home.as_os_str().is_empty(), "home_dir must not be empty");
    }

    #[test]
    fn diegops_dir_ends_with_diegops() {
        let dir = diegops_dir().expect("diegops_dir should succeed");
        assert!(
            dir.ends_with(".diegops"),
            "diegops_dir should end with .diegops, got: {}",
            dir.display()
        );
    }

    #[test]
    fn expand_home_with_home_prefix() {
        let home = home_dir().expect("home_dir should succeed");
        let expanded = expand_home("$HOME/test");
        assert_eq!(expanded, home.join("test"));
    }

    #[test]
    fn expand_home_with_leading_slash() {
        let home = home_dir().expect("home_dir should succeed");
        let expanded = expand_home("/$HOME/test");
        assert_eq!(expanded, home.join("test"));
    }

    #[test]
    fn expand_home_absolute_path_unchanged() {
        let expanded = expand_home("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn expand_home_bare_home() {
        let home = home_dir().expect("home_dir should succeed");
        let expanded = expand_home("$HOME");
        assert_eq!(expanded, home);
    }

    #[test]
    fn resolve_config_path_cli_flag_takes_precedence() {
        let cli = PathBuf::from("/custom/config.yaml");
        let result = resolve_config_path(
            Some(cli.as_path()),
            "NONEXISTENT_ENV_VAR_12345",
            "default.yaml",
        )
        .expect("should succeed");
        assert_eq!(result, cli);
    }

    #[test]
    fn resolve_config_path_falls_back_to_default() {
        // Use an env var name that definitely does not exist
        let result =
            resolve_config_path(None, "DIEGOPS_TEST_NONEXISTENT_ENV_VAR_98765", "repos.yaml")
                .expect("should succeed");
        let expected = diegops_dir()
            .expect("diegops_dir should succeed")
            .join("repos.yaml");
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_config_path_env_var_overrides_default() {
        let key = "DIEGOPS_TEST_RESOLVE_CONFIG_12345";
        std::env::set_var(key, "/from/env.yaml");
        let result = resolve_config_path(None, key, "default.yaml").expect("should succeed");
        std::env::remove_var(key);
        assert_eq!(result, PathBuf::from("/from/env.yaml"));
    }

    // -----------------------------------------------------------------------
    // parse_vault_kv_response
    // -----------------------------------------------------------------------

    #[test]
    fn parse_vault_kv_response_extracts_data() {
        let json = br#"{
            "data": {
                "data": {
                    "database.env": "aGVsbG8=",
                    "api.env": "d29ybGQ="
                },
                "metadata": { "version": 1 }
            }
        }"#;
        let data = parse_vault_kv_response(json, "secret/test").unwrap();
        assert_eq!(
            data.get("database.env").unwrap().as_str().unwrap(),
            "aGVsbG8="
        );
        assert_eq!(data.get("api.env").unwrap().as_str().unwrap(), "d29ybGQ=");
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn parse_vault_kv_response_rejects_missing_data_data() {
        let json = br#"{"data": {"wrong": "shape"}}"#;
        assert!(parse_vault_kv_response(json, "secret/test").is_err());
    }

    #[test]
    fn parse_vault_kv_response_rejects_bad_json() {
        assert!(parse_vault_kv_response(b"not json", "secret/test").is_err());
    }

    // -----------------------------------------------------------------------
    // write_file_with_backup
    // -----------------------------------------------------------------------

    #[test]
    fn write_file_with_backup_new_file_no_backup() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-common-backup-new-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let path = dir.join("secret.env");
        let backed_up = write_file_with_backup(&path, b"content").unwrap();

        assert!(!backed_up);
        assert_eq!(fs::read(&path).unwrap(), b"content");
        assert!(!dir.join("secret.env.bak").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_with_backup_same_content_no_backup() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-common-backup-same-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let path = dir.join("secret.env");
        fs::write(&path, b"same content").unwrap();

        let backed_up = write_file_with_backup(&path, b"same content").unwrap();

        assert!(!backed_up);
        assert_eq!(fs::read(&path).unwrap(), b"same content");
        assert!(!dir.join("secret.env.bak").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_with_backup_different_content_creates_backup() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-common-backup-diff-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let path = dir.join("secret.env");
        fs::write(&path, b"old content").unwrap();

        let backed_up = write_file_with_backup(&path, b"new content").unwrap();

        assert!(backed_up);
        assert_eq!(fs::read(&path).unwrap(), b"new content");
        let bak_path = dir.join("secret.env.bak");
        assert!(
            bak_path.exists(),
            "backup file should exist at {}",
            bak_path.display()
        );
        assert_eq!(fs::read(&bak_path).unwrap(), b"old content");

        let _ = fs::remove_dir_all(&dir);
    }
}
