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
    let (path, from) = match cli_path {
        Some(p) => (p.to_owned(), "--config".to_string()),
        None => match std::env::var(env_var) {
            Ok(env_path) => (PathBuf::from(env_path), format!("${env_var}")),
            Err(_) => (diegops_dir()?.join(default_filename), "default".into()),
        },
    };
    crate::debug!("config: {} ({from})", path.display());
    Ok(path)
}

/// The `init` command that writes a sample of the given config file.
pub fn init_command_for(path: &Path) -> Option<&'static str> {
    match path.file_name()?.to_str()? {
        "repos.yaml" => Some("diegops repo init"),
        "secrets.yaml" => Some("diegops files init"),
        "repo-vault.yaml" => Some("diegops env init"),
        "agents.yaml" => Some("diegops agents init"),
        "gpg-config.yaml" => Some("diegops devtools gpg init"),
        _ => None,
    }
}

/// Error for a batch where some items failed: names them and says what to do.
pub fn failed_items_error(
    what: &str,
    total: usize,
    failed: &[String],
    hint: &str,
) -> Box<dyn std::error::Error> {
    format!(
        "{} of {total} {what}(s) failed: {}\n  hint: {hint}",
        failed.len(),
        failed.join(", ")
    )
    .into()
}

/// Reads a config file and returns its content as a string.
///
/// Provides user-friendly errors for missing files.
pub fn read_config_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    fs::read_to_string(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            let hint = match init_command_for(path) {
                Some(cmd) => format!("run '{cmd}' to create a sample, or pass --config <path>"),
                None => "pass --config <path> or create the file".to_string(),
            };
            format!("config not found: {}\n  hint: {hint}", path.display()).into()
        } else {
            format!("could not read {}: {e}", path.display()).into()
        }
    })
}

// ---------------------------------------------------------------------------
// Vault KV helpers (shared by secrets.rs and vault.rs)
// ---------------------------------------------------------------------------

/// Fetches all key-value pairs at `path` from the configured backend.
///
/// Same shape as [`vault_kv_get`] so callers are backend-agnostic.
pub fn kv_get(
    backend: &dyn crate::secrets_backend::SecretsBackend,
    path: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let data = backend.get(path)?.ok_or_else(|| {
        format!(
            "secret not found at '{path}'. Verify the path exists in {}",
            backend.name()
        )
    })?;
    Ok(data
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect())
}

/// Writes `content` to `path`, creating a `.bak` backup if the file already
/// exists and its content differs.
///
/// What [`write_file_with_backup`] would do, without touching the disk
/// (`--dry-run`).
#[derive(Debug, PartialEq, Eq)]
pub enum WritePlan {
    Create,
    Unchanged,
    /// Differs: would be overwritten and a `.bak` kept.
    Replace,
}

pub fn write_plan(path: &Path, content: &[u8]) -> WritePlan {
    match fs::read(path) {
        Ok(old) if old == content => WritePlan::Unchanged,
        Ok(_) => WritePlan::Replace,
        Err(_) if path.exists() => WritePlan::Replace,
        Err(_) => WritePlan::Create,
    }
}

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
    #[test]
    fn write_plan_reports_without_writing() {
        let dir = std::env::temp_dir().join(format!("diegops-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join(".env");
        assert_eq!(super::write_plan(&f, b"A=1\n"), super::WritePlan::Create);
        std::fs::write(&f, b"A=1\n").unwrap();
        assert_eq!(super::write_plan(&f, b"A=1\n"), super::WritePlan::Unchanged);
        assert_eq!(super::write_plan(&f, b"A=2\n"), super::WritePlan::Replace);
        assert_eq!(std::fs::read(&f).unwrap(), b"A=1\n");
        assert!(!dir.join(".env.bak").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_is_supported_by_every_command_that_writes_user_files() {
        for c in [
            "env apply",
            "files pull",
            "files push",
            "agents hermes restore",
        ] {
            assert!(crate::ui::check_dry_run(true, c).is_ok(), "{c}");
        }
    }

    #[test]
    fn missing_config_hint_names_the_init_command() {
        let cases = [
            ("repos.yaml", "diegops repo init"),
            ("secrets.yaml", "diegops files init"),
            ("repo-vault.yaml", "diegops env init"),
            ("agents.yaml", "diegops agents init"),
            ("gpg-config.yaml", "diegops devtools gpg init"),
        ];
        for (file, cmd) in cases {
            let err = super::read_config_file(&std::path::Path::new("/nonexistent/x").join(file))
                .unwrap_err()
                .to_string();
            assert!(
                err.starts_with("config not found: /nonexistent/x/"),
                "{err}"
            );
            assert!(
                err.contains(&format!("hint: run '{cmd}' to create a sample")),
                "{err}"
            );
        }
    }

    #[test]
    fn missing_unknown_config_keeps_generic_hint() {
        let err = super::read_config_file(std::path::Path::new("/nonexistent/other.yaml"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("hint: pass --config <path>"), "{err}");
    }

    #[test]
    fn failed_items_error_names_items_and_hint() {
        let err = super::failed_items_error(
            "clone",
            3,
            &["a".to_string(), "b".to_string()],
            "check access with 'diegops auth status'",
        )
        .to_string();
        assert_eq!(
            err,
            "2 of 3 clone(s) failed: a, b\n  hint: check access with 'diegops auth status'"
        );
    }

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
    // kv_get (backend-agnostic)
    struct OneSecret;
    impl crate::secrets_backend::SecretsBackend for OneSecret {
        fn name(&self) -> &'static str {
            "mem"
        }
        fn get(
            &self,
            path: &str,
        ) -> Result<Option<crate::secrets_backend::SecretMap>, Box<dyn std::error::Error>> {
            Ok((path == "m/a").then(|| [("k".to_string(), "v".to_string())].into()))
        }
        fn list(&self, _: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
            Ok(vec![])
        }
        fn put(
            &self,
            _: &str,
            _: &crate::secrets_backend::SecretMap,
        ) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }

    #[test]
    fn kv_get_returns_string_values() {
        let m = kv_get(&OneSecret, "m/a").unwrap();
        assert_eq!(m["k"], serde_json::json!("v"));
    }

    #[test]
    fn kv_get_missing_path_is_not_found_error() {
        let e = kv_get(&OneSecret, "m/b").unwrap_err().to_string();
        assert!(e.contains("secret not found at 'm/b'"), "{e}");
    }

    // -----------------------------------------------------------------------

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
