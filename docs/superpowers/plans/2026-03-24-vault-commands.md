# Vault Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `diegops vault init|apply|list|list-diff` commands that pull secrets from HashiCorp Vault KV v2 and write `.env` files into local repo directories.

**Architecture:** New `src/commands/vault.rs` module mirroring the `repo.rs` pattern. Shared helpers (`expand_home`, `home_dir`, `diegops_dir`) extracted to `src/commands/common.rs`. Vault interaction via `std::process::Command` shelling out to the `vault` CLI. Config in `~/.diegops/repo-vault.yaml`.

**Tech Stack:** Rust stable, clap v4 derive, serde + serde_yaml + serde_json, std::process::Command

**Spec:** `docs/superpowers/specs/2026-03-24-vault-commands-design.md`

---

### Task 1: Extract shared helpers to `common.rs`

**Files:**
- Create: `src/commands/common.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/commands/repo.rs`

- [ ] **Step 1: Create `src/commands/common.rs` with helpers extracted from `repo.rs`**

```rust
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
```

- [ ] **Step 2: Update `src/commands/mod.rs` to add `common` module**

```rust
//! Command implementations.
//!
//! Each non-trivial command lives in its own submodule and exposes a single
//! `pub fn run(...) -> Result<(), Box<dyn std::error::Error>>` entry point.

pub mod common;
pub mod repo;
pub mod update;
```

- [ ] **Step 3: Refactor `repo.rs` to use `common::` helpers**

Replace the private `home_dir`, `expand_home`, `diegops_dir`, `default_config_path`, and `load_config` functions. The `load_config` function becomes:

```rust
fn load_config(path: Option<&Path>) -> Result<ReposConfig, Box<dyn std::error::Error>> {
    let config_path = super::common::resolve_config_path(
        path,
        "DIEGOPS_REPOS_CONFIG",
        "repos.yaml",
    )?;
    let content = super::common::read_config_file(&config_path)?;
    serde_yaml::from_str::<ReposConfig>(&content)
        .map_err(|e| format!("invalid repos config {}: {e}", config_path.display()).into())
}
```

Replace all calls to `expand_home(...)` with `super::common::expand_home(...)`.
Replace call to `diegops_dir()` in `init()` with `super::common::diegops_dir()`.
Remove the now-unused private functions: `home_dir`, `expand_home`, `diegops_dir`, `default_config_path`, `load_config` (the old version).

- [ ] **Step 4: Run tests to verify no regressions**

Run: `cargo test`
Expected: all existing tests pass

- [ ] **Step 5: Run lints**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: clean

- [ ] **Step 6: Commit**

```bash
git add src/commands/common.rs src/commands/mod.rs src/commands/repo.rs
git commit -m "refactor: extract shared helpers to commands/common.rs"
```

---

### Task 2: Add `VaultCommand` enum and wire into `main.rs`

**Files:**
- Create: `src/commands/vault.rs` (skeleton)
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write integration tests for vault help flags**

Add to `tests/integration_test.rs`:

```rust
#[test]
fn vault_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "--help"])
        .output()
        .expect("failed to run diegops vault --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("apply"), "got: {stdout}");
    assert!(stdout.contains("list"), "got: {stdout}");
    assert!(stdout.contains("list-diff"), "got: {stdout}");
    assert!(stdout.contains("init"), "got: {stdout}");
}

#[test]
fn vault_init_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "init", "--help"])
        .output()
        .expect("failed to run diegops vault init --help");

    assert!(output.status.success());
}

#[test]
fn vault_apply_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "apply", "--help"])
        .output()
        .expect("failed to run diegops vault apply --help");

    assert!(output.status.success());
}

#[test]
fn vault_list_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "list", "--help"])
        .output()
        .expect("failed to run diegops vault list --help");

    assert!(output.status.success());
}

#[test]
fn vault_list_diff_help_exits_successfully() {
    let output = diegops()
        .args(["vault", "list-diff", "--help"])
        .output()
        .expect("failed to run diegops vault list-diff --help");

    assert!(output.status.success());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: compilation error — `vault` module does not exist yet

- [ ] **Step 3: Create `src/commands/vault.rs` skeleton with clap enum and stub functions**

```rust
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
pub fn list_diff(
    _config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("vault list-diff")
}

/// Creates `~/.diegops/repo-vault.yaml` with a sample config.
///
/// If `base_dir` is `Some`, writes config under that directory instead of `~/.diegops/`.
/// This parameter exists for testability — production code passes `None`.
pub fn init(base_dir: Option<&std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    todo!("vault init")
}
```

- [ ] **Step 4: Add `vault` module to `mod.rs`**

```rust
pub mod common;
pub mod repo;
pub mod update;
pub mod vault;
```

- [ ] **Step 5: Wire `Vault` command into `main.rs`**

Add to the `Commands` enum:

```rust
/// Manage Vault secrets — pull secrets and write .env files from a config file
Vault {
    #[command(subcommand)]
    cmd: VaultCommand,
},
```

Add the match arm in `main()`:

```rust
Some(Commands::Vault { cmd }) => {
    let config_path_str;
    match cmd {
        VaultCommand::Apply { path, config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::vault::apply(cfg, path.as_deref())?;
        }
        VaultCommand::List { config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::vault::list(cfg)?;
        }
        VaultCommand::ListDiff { config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::vault::list_diff(cfg)?;
        }
        VaultCommand::Init => {
            commands::vault::init(None)?;
        }
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: all tests pass (help tests work, stubs are not called)

- [ ] **Step 7: Commit**

```bash
git add src/commands/vault.rs src/commands/mod.rs src/main.rs tests/integration_test.rs
git commit -m "feat: add vault command skeleton with clap wiring"
```

---

### Task 3: Implement `vault init`

**Files:**
- Modify: `src/commands/vault.rs`

- [ ] **Step 1: Write unit test for init**

Add at the bottom of `vault.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn init_creates_sample_config() {
        let dir = std::env::temp_dir()
            .join(format!("diegops-test-vault-init-{}", std::process::id()));
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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test vault::tests::init_creates_sample_config`
Expected: FAIL — `todo!()` panic

- [ ] **Step 3: Implement `init`**

Replace the `init` stub and add `SAMPLE_CONFIG`:

```rust
use std::path::{Path, PathBuf};
use std::fs;

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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test vault::tests::init_creates_sample_config`
Expected: PASS

- [ ] **Step 5: Lint**

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/commands/vault.rs
git commit -m "feat: implement vault init command"
```

---

### Task 4: Implement config loading and `vault list` / `vault list-diff`

**Files:**
- Modify: `src/commands/vault.rs`

- [ ] **Step 1: Write unit tests for config deserialization**

Add to the `tests` module in `vault.rs`:

```rust
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
```

- [ ] **Step 2: Run tests — should pass (serde + structs already defined)**

Run: `cargo test vault::tests::parse_config`
Expected: PASS

- [ ] **Step 3: Implement `load_config` helper and `list`**

```rust
fn load_config(path: Option<&Path>) -> Result<VaultConfig, Box<dyn std::error::Error>> {
    let config_path = super::common::resolve_config_path(
        path,
        "DIEGOPS_VAULT_CONFIG",
        "repo-vault.yaml",
    )?;
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
                    ).into());
                }
            }
        }
    }

    Ok(config)
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
```

- [ ] **Step 4: Implement `list_diff`**

```rust
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
        eprintln!("{total} targets missing .env files. Run `diegops vault apply` to generate them.");
    }

    Ok(())
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 6: Lint**

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 7: Commit**

```bash
git add src/commands/vault.rs
git commit -m "feat: implement vault list and list-diff commands"
```

---

### Task 5: Implement Vault CLI helpers (pre-flight checks and secret fetching)

**Files:**
- Modify: `src/commands/vault.rs`

- [ ] **Step 1: Write unit tests for `.env` line formatting and vault JSON parsing**

```rust
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
    // Non-string values are preserved as JSON values
    assert_eq!(data.get("port").unwrap().as_u64().unwrap(), 5432);
    assert_eq!(data.len(), 3);
}

#[test]
fn parse_vault_response_rejects_bad_json() {
    let json = br#"{"data": {"wrong": "shape"}}"#;
    assert!(parse_vault_response(json, "secret/test").is_err());
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
    assert_eq!(
        format_env_line("key", "has spaces"),
        r#"key="has spaces""#
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test vault::tests::format_env_line`
Expected: FAIL — function does not exist

- [ ] **Step 3: Implement helper functions**

```rust
use std::process::Command;

/// Formats a single `.env` line with double-quoted, escaped value.
fn format_env_line(key: &str, value: &str) -> String {
    let escaped = value.replace('\\', r"\\").replace('"', r#"\""#);
    format!("{key}=\"{escaped}\"")
}

/// Checks that the `vault` CLI is available on PATH.
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
fn check_vault_addr() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("VAULT_ADDR").is_err() {
        return Err("VAULT_ADDR is not set. Export it or configure your Vault client".into());
    }
    Ok(())
}

/// Checks that the current Vault token is valid.
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
        if stderr.contains("no secrets") || stderr.contains("Not Found") || stderr.contains("404")
        {
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
fn parse_vault_response(
    json_bytes: &[u8],
    vault_path: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let json: serde_json::Value = serde_json::from_slice(json_bytes)?;
    let data = json
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(|d| d.as_object())
        .ok_or_else(|| {
            format!("unexpected JSON structure from vault kv get for '{vault_path}'")
        })?;

    Ok(data.clone())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all pass (new unit test passes, vault CLI helpers are not called from tests)

- [ ] **Step 5: Lint**

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add src/commands/vault.rs
git commit -m "feat: add vault CLI helpers and env line formatting"
```

---

### Task 6: Implement `vault apply`

**Files:**
- Modify: `src/commands/vault.rs`

- [ ] **Step 1: Write unit tests for `.gitignore` update and `.env` content generation**

```rust
#[test]
fn ensure_gitignore_adds_env_entry() {
    let dir = std::env::temp_dir()
        .join(format!("diegops-test-gitignore-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // No .gitignore exists — should create one
    ensure_gitignore(&dir).unwrap();
    let content = fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert!(content.contains(".env"));

    // Already present — should not duplicate
    ensure_gitignore(&dir).unwrap();
    let content = fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert_eq!(content.matches(".env").count(), 1);

    // .gitignore without trailing newline
    fs::write(dir.join(".gitignore"), "node_modules").unwrap();
    ensure_gitignore(&dir).unwrap();
    let content = fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert!(content.contains("node_modules\n.env"));

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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test vault::tests::ensure_gitignore vault::tests::build_env_content`
Expected: FAIL — functions do not exist

- [ ] **Step 3: Implement `.gitignore` helper and `.env` content builder**

```rust
/// Ensures `.env` is listed in the target directory's `.gitignore`.
fn ensure_gitignore(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let gitignore = dir.join(".gitignore");

    if gitignore.exists() {
        let content = fs::read_to_string(&gitignore)?;
        // Exact line match
        if content.lines().any(|line| line.trim() == ".env") {
            return Ok(());
        }
        // Ensure preceding newline
        let prefix = if content.ends_with('\n') || content.is_empty() {
            ""
        } else {
            "\n"
        };
        fs::write(&gitignore, format!("{content}{prefix}.env\n"))?;
    } else {
        fs::write(&gitignore, ".env\n")?;
    }

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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test vault::tests::ensure_gitignore vault::tests::build_env_content`
Expected: PASS

- [ ] **Step 5: Implement `apply`**

```rust
/// Pulls secrets from Vault and writes `.env` files for each target.
pub fn apply(
    config_path: Option<&Path>,
    path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Pre-flight checks — fatal, exit immediately on failure
    check_vault_binary()?;
    check_vault_addr()?;
    check_vault_auth()?;

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
            match vault_kv_get(&entry.vault_path) {
                Ok(data) => {
                    match &entry.keys {
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
                                            v.as_str()
                                                .unwrap_or(&v.to_string())
                                                .to_string(),
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
                    }
                }
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

        // Build .env content and check if it changed
        let env_content = build_env_content(&all_secrets);
        let env_path = dest.join(".env");

        if env_path.exists() {
            if let Ok(existing) = fs::read_to_string(&env_path) {
                if existing == env_content {
                    eprintln!("  SKIP  .env unchanged");
                    n_skipped += 1;
                    continue;
                }
            }
        }

        // Write .env
        fs::write(&env_path, &env_content)?;

        // Set file permissions to 0600 on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&env_path, fs::Permissions::from_mode(0o600))?;
        }

        // Update .gitignore
        ensure_gitignore(&dest)?;

        eprintln!("  WRITE .env ({} secrets)", all_secrets.len());
        n_written += 1;
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
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 7: Lint**

Run: `cargo fmt && cargo clippy -- -D warnings`

- [ ] **Step 8: Commit**

```bash
git add src/commands/vault.rs
git commit -m "feat: implement vault apply command"
```

---

### Task 7: Update README.md and CLAUDE.md

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add vault commands to README.md**

Add to the Commands section (after the repo commands block):

```markdown
diegops vault init                                 Create sample config at ~/.diegops/repo-vault.yaml
diegops vault apply                                Pull secrets and write .env files
diegops vault apply --path <prefix>                Pull only for targets under this path prefix
diegops vault apply --config <file>                Use a custom repo-vault.yaml
diegops vault list                                 Show targets that have a .env file
diegops vault list-diff                            Show targets missing a .env file
```

Add a `### diegops vault — secret management` section with usage examples, config format, and prerequisites (vault CLI, authentication, VAULT_ADDR).

- [ ] **Step 2: Add vault command docs to CLAUDE.md**

Add a `### diegops vault — Vault secret management` section in the Commands table and detailed description, following the pattern of the existing `### diegops repo` and `### diegops update` sections.

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add vault commands to README and CLAUDE.md"
```

---

### Task 8: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Run full lint suite**

Run: `cargo fmt --check && cargo clippy -- -D warnings`
Expected: clean

- [ ] **Step 3: Verify build**

Run: `cargo build --release`
Expected: clean build

- [ ] **Step 4: Smoke test CLI**

Run: `cargo run -- vault --help`
Expected: shows all vault subcommands

Run: `cargo run -- vault init --help`
Expected: shows init help
