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
}
