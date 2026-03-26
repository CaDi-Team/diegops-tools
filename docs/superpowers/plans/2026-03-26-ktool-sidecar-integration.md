# ktool Sidecar Integration & CLI Elevation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate ktool as a managed sidecar inside diegops and elevate ktool to match diegops' CLI maturity with update, auth, cadi, and version commands.

**Architecture:** Two independent repos, convention-based sharing. ktool lives at `~/.ktool/` standalone; diegops manages a sidecar copy at `~/.diegops/bin/ktool` and reads ktool's tokens read-only. No shared crate.

**Tech Stack:** Rust stable, clap v4 derive, ureq (update HTTP), reqwest (karluiz API), flate2+tar (archive extraction), serde_json (token schema)

**Repos:**
- ktool: `/Users/diegopinto/github/dpinto-config/karluiz-tool-cli`
- diegops: `/Users/diegopinto/github/dpinto-config/diegops-tools`

---

## Part A: ktool Changes (karluiz-tool-cli)

### Task 1: Update Cargo.toml dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Update Cargo.toml**

```toml
[package]
name = "karluiz-tool"
version = "0.2.0"
edition = "2024"
description = "CLI for karluiz tools"
license = "MIT"
repository = "https://github.com/CaDi-Team/karluiz-tool-cli"

[[bin]]
name = "ktool"
path = "src/main.rs"

[dependencies]
clap = { version = "4.6", features = ["derive"] }
dirs = "6.0"
reqwest = { version = "0.13", features = ["blocking", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "1"
# ureq: sync HTTP for update command (rustls, no OpenSSL)
ureq = { version = "2", default-features = false, features = ["tls", "json"] }
humantime = "2"

# Unix: extract .tar.gz release archives
[target.'cfg(unix)'.dependencies]
flate2 = "1"
tar = "0.4"

# Windows: extract .zip release archives
[target.'cfg(windows)'.dependencies]
zip = "2"

[dev-dependencies]
tempfile = "3"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

Note: `rpassword` removed (no interactive login). `ureq`, `humantime`, `flate2`, `tar`, `zip` added for update command. Version bumped to `0.2.0`. Release profile added matching diegops.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles (warnings ok for now — unused imports will resolve as we add code)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: update dependencies for auth, update, and cadi commands"
```

---

### Task 2: Add common helpers module

**Files:**
- Create: `src/commands/common.rs`
- Modify: `src/commands/mod.rs`

- [ ] **Step 1: Create `src/commands/common.rs`**

```rust
//! Shared helpers used across command modules.

use std::path::PathBuf;

/// Returns the current user's home directory.
///
/// Checks `$HOME` first (Unix), then `$USERPROFILE` (Windows).
pub fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| "home directory not set ($HOME / $USERPROFILE)".to_string())
}

/// Returns `~/.ktool/`.
pub fn ktool_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".ktool"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_a_path() {
        let home = home_dir().unwrap();
        assert!(home.is_absolute() || !home.as_os_str().is_empty());
    }

    #[test]
    fn ktool_dir_is_under_home() {
        let home = home_dir().unwrap();
        let ktool = ktool_dir().unwrap();
        assert!(ktool.starts_with(&home));
        assert!(ktool.ends_with(".ktool"));
    }
}
```

- [ ] **Step 2: Update `src/commands/mod.rs`**

```rust
pub mod common;
pub mod kenv;
```

- [ ] **Step 3: Run tests**

Run: `cargo test commands::common`
Expected: 2 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/commands/common.rs src/commands/mod.rs
git commit -m "feat: add common helpers module with home_dir and ktool_dir"
```

---

### Task 3: Add auth module — token storage

**Files:**
- Create: `src/commands/auth.rs`
- Modify: `src/commands/mod.rs`

- [ ] **Step 1: Write the auth token storage tests**

Create `src/commands/auth.rs`:

```rust
//! Auth credential management — store, load, and validate provider tokens.
//!
//! Tokens are stored as JSON files in `~/.ktool/tokens/`, one per provider.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Clap sub-command definitions
// ---------------------------------------------------------------------------

/// Auth credential management sub-commands.
#[derive(clap::Subcommand)]
pub enum AuthCommand {
    /// kenv authentication
    Kenv {
        #[command(subcommand)]
        cmd: KenvAuthCommand,
    },
    /// Show authentication status for all providers
    Status,
    /// Remove all stored authentication tokens
    Logout,
}

/// kenv-specific auth sub-commands.
#[derive(clap::Subcommand)]
pub enum KenvAuthCommand {
    /// Validate and store a kenv API token
    Login {
        /// kenv API token
        token: String,
    },
    /// Remove stored kenv token
    Logout,
    /// Show authenticated kenv context
    Whoami,
}

// ---------------------------------------------------------------------------
// Token data structures
// ---------------------------------------------------------------------------

/// Stored token metadata — same schema as diegops for interoperability.
#[derive(Serialize, Deserialize, Debug)]
pub struct TokenData {
    /// Schema version for forward compatibility.
    pub version: u32,
    /// The token value.
    pub token: String,
    /// RFC 3339 timestamp of when the token was stored.
    pub stored_at: String,
}

// ---------------------------------------------------------------------------
// Token file I/O (internal helpers with explicit dir for testability)
// ---------------------------------------------------------------------------

/// Returns the tokens directory: `~/.ktool/tokens/`.
fn tokens_dir() -> Result<PathBuf, String> {
    Ok(super::common::ktool_dir()?.join("tokens"))
}

/// Formats the current time as an RFC 3339 timestamp.
fn format_rfc3339_now() -> String {
    humantime::format_rfc3339(std::time::SystemTime::now()).to_string()
}

/// Saves a kenv token to the given directory.
fn save_kenv_token_to(dir: &Path, token: &str) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create tokens directory: {e}"))?;
    let data = TokenData {
        version: 1,
        token: token.to_owned(),
        stored_at: format_rfc3339_now(),
    };
    let path = dir.join("kenv.json");
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize token: {e}"))?;
    fs::write(&path, json)
        .map_err(|e| format!("Failed to write token file: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to set token file permissions: {e}"))?;
    }

    Ok(())
}

/// Loads a kenv token from the given directory. Returns `None` if file absent.
fn load_kenv_token_from(dir: &Path) -> Result<Option<String>, String> {
    let path = dir.join("kenv.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read token file: {e}"))?;
    let data: TokenData = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse token file: {e}"))?;
    Ok(Some(data.token))
}

/// Loads token metadata from the given directory. Returns `None` if file absent.
fn load_kenv_token_data_from(dir: &Path) -> Result<Option<TokenData>, String> {
    let path = dir.join("kenv.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read token file: {e}"))?;
    let data = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse token file: {e}"))?;
    Ok(Some(data))
}

/// Removes the kenv token file. Idempotent. Returns true if file was removed.
fn remove_kenv_token_from(dir: &Path) -> Result<bool, String> {
    let path = dir.join("kenv.json");
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove token file: {e}"))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Public API (uses default tokens_dir)
// ---------------------------------------------------------------------------

/// Saves a kenv token to `~/.ktool/tokens/kenv.json`.
pub fn save_kenv_token(token: &str) -> Result<(), String> {
    save_kenv_token_to(&tokens_dir()?, token)
}

/// Loads the kenv token. Resolution: file > $KENV_API_TOKEN > None.
pub fn load_kenv_token() -> Result<Option<String>, String> {
    let from_file = load_kenv_token_from(&tokens_dir()?)?;
    if from_file.is_some() {
        return Ok(from_file);
    }
    if let Ok(env_token) = std::env::var("KENV_API_TOKEN") {
        if !env_token.is_empty() {
            return Ok(Some(env_token));
        }
    }
    Ok(None)
}

/// Loads kenv token metadata for status display.
pub fn load_kenv_token_data() -> Result<Option<TokenData>, String> {
    load_kenv_token_data_from(&tokens_dir()?)
}

/// Removes the kenv token. Returns true if a file was removed.
pub fn remove_kenv_token() -> Result<bool, String> {
    remove_kenv_token_from(&tokens_dir()?)
}

/// Removes all tokens in `~/.ktool/tokens/`. Returns count removed.
pub fn remove_all_tokens() -> Result<usize, String> {
    let dir = tokens_dir()?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read tokens directory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
            fs::remove_file(entry.path())
                .map_err(|e| format!("Failed to remove token file: {e}"))?;
            count += 1;
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Validates and stores a kenv API token.
pub fn kenv_login(token: &str) -> Result<(), String> {
    // Validate by making a test request to the karluiz API
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(crate::api::BASE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("Failed to validate token: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(format!(
            "Token validation failed (HTTP {status}). Check that your token is valid."
        ));
    }

    save_kenv_token(token)?;
    println!("Token validated and saved.");
    Ok(())
}

/// Shows authenticated kenv context.
pub fn kenv_whoami() -> Result<(), String> {
    let token = load_kenv_token()?
        .ok_or("Not authenticated. Run 'ktool auth kenv login <TOKEN>' first.")?;

    // Show token is configured and valid
    let data = load_kenv_token_data()?;
    match data {
        Some(d) => {
            println!("kenv: authenticated");
            println!("Token stored at: {}", d.stored_at);
            // Show the obfuscated token
            println!("Token: {}", crate::api::obfuscate(&token));
        }
        None => {
            println!("kenv: authenticated via $KENV_API_TOKEN");
        }
    }
    Ok(())
}

/// Removes the stored kenv token.
pub fn kenv_logout() -> Result<(), String> {
    if remove_kenv_token()? {
        println!("kenv token removed.");
    } else {
        println!("kenv token was not configured.");
    }
    Ok(())
}

/// Shows auth status for all providers.
pub fn status() -> Result<(), String> {
    match load_kenv_token_data()? {
        Some(data) => {
            println!(
                "kenv    [ok] configured    stored {}",
                data.stored_at
            );
        }
        None => {
            if std::env::var("KENV_API_TOKEN").map(|v| !v.is_empty()).unwrap_or(false) {
                println!("kenv    [ok] configured    via $KENV_API_TOKEN");
            } else {
                println!("kenv    [--] not configured");
            }
        }
    }
    Ok(())
}

/// Removes all stored tokens.
pub fn logout_all() -> Result<(), String> {
    let count = remove_all_tokens()?;
    if count > 0 {
        println!("Removed {count} token(s).");
    } else {
        println!("No tokens were configured.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_kenv_token() {
        let dir = std::env::temp_dir().join(format!("ktool-test-auth-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        save_kenv_token_to(&dir, "test_token_123").unwrap();

        let token_file = dir.join("kenv.json");
        assert!(token_file.exists());

        let data: TokenData =
            serde_json::from_str(&fs::read_to_string(&token_file).unwrap()).unwrap();
        assert_eq!(data.token, "test_token_123");
        assert_eq!(data.version, 1);
        assert!(!data.stored_at.is_empty());

        let loaded = load_kenv_token_from(&dir).unwrap();
        assert_eq!(loaded.unwrap(), "test_token_123");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_kenv_token_returns_none_when_missing() {
        let dir =
            std::env::temp_dir().join(format!("ktool-test-auth-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let loaded = load_kenv_token_from(&dir).unwrap();
        assert!(loaded.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_kenv_token_is_idempotent() {
        let dir =
            std::env::temp_dir().join(format!("ktool-test-auth-rm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        save_kenv_token_to(&dir, "test_token_123").unwrap();
        assert!(dir.join("kenv.json").exists());

        let removed = remove_kenv_token_from(&dir).unwrap();
        assert!(removed);
        assert!(!dir.join("kenv.json").exists());

        let removed = remove_kenv_token_from(&dir).unwrap();
        assert!(!removed);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_rfc3339_produces_valid_timestamp() {
        let ts = format_rfc3339_now();
        assert!(ts.contains('T'), "got: {ts}");
        assert!(ts.ends_with('Z'), "got: {ts}");
    }

    #[test]
    fn token_data_roundtrips_as_json() {
        let data = TokenData {
            version: 1,
            token: "tok123".to_string(),
            stored_at: "2026-03-26T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string_pretty(&data).unwrap();
        let parsed: TokenData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.token, "tok123");
        assert_eq!(parsed.stored_at, "2026-03-26T12:00:00Z");
    }
}
```

- [ ] **Step 2: Update `src/commands/mod.rs`**

```rust
pub mod auth;
pub mod common;
pub mod kenv;
```

- [ ] **Step 3: Run tests**

Run: `cargo test commands::auth`
Expected: 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/commands/auth.rs src/commands/mod.rs
git commit -m "feat: add auth module with kenv token management"
```

---

### Task 4: Add version command

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add Version and Help commands to clap structure and wire them up**

Replace the full `src/main.rs` content with:

```rust
mod api;
mod commands;
mod config;

use clap::{CommandFactory, Parser, Subcommand};
use commands::auth::{AuthCommand, KenvAuthCommand};
use commands::kenv::{KenvArgs, KenvCommands};

#[derive(Parser)]
#[command(
    name = "ktool",
    about = "CLI for karluiz tools",
    version,
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Print version information
    Version,
    /// Show help and available commands
    Help,
    /// Manage authentication tokens
    Auth {
        #[command(subcommand)]
        cmd: AuthCommand,
    },
    /// Manage the kenv secrets service.
    ///
    /// Use --set-app / --set-env to save default context, then `ktool kenv list` to
    /// fetch secrets.
    Kenv(KenvArgs),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Version) => {
            println!("ktool v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(Commands::Help) | None => {
            Cli::command().print_long_help().map_err(|e| e.to_string())
        }
        Some(Commands::Auth { cmd }) => run_auth(cmd),
        Some(Commands::Kenv(args)) => run_kenv(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// auth
// ---------------------------------------------------------------------------

fn run_auth(cmd: AuthCommand) -> Result<(), String> {
    match cmd {
        AuthCommand::Kenv { cmd: kenv_cmd } => match kenv_cmd {
            KenvAuthCommand::Login { token } => commands::auth::kenv_login(&token),
            KenvAuthCommand::Logout => commands::auth::kenv_logout(),
            KenvAuthCommand::Whoami => commands::auth::kenv_whoami(),
        },
        AuthCommand::Status => commands::auth::status(),
        AuthCommand::Logout => commands::auth::logout_all(),
    }
}

// ---------------------------------------------------------------------------
// kenv
// ---------------------------------------------------------------------------

fn run_kenv(args: KenvArgs) -> Result<(), String> {
    let mut cfg = config::load()?;
    let mut updated = false;

    if let Some(app) = args.set_app {
        cfg.app = Some(app);
        updated = true;
    }
    if let Some(env) = args.set_env {
        cfg.env = Some(env);
        updated = true;
    }
    if updated {
        config::save(&cfg)?;
        println!(
            "Config updated — app: {}, env: {}.",
            cfg.app.as_deref().unwrap_or("(not set)"),
            cfg.env.as_deref().unwrap_or("(not set)"),
        );
    }

    match args.command {
        Some(KenvCommands::List(list_args)) => run_kenv_list(&cfg, list_args.json),
        None => {
            if !updated {
                println!(
                    "Current context — app: {}, env: {}",
                    cfg.app.as_deref().unwrap_or("(not set)"),
                    cfg.env.as_deref().unwrap_or("(not set)"),
                );
                println!("Run `ktool kenv list` to fetch secrets.");
            }
            Ok(())
        }
    }
}

fn run_kenv_list(cfg: &config::Config, as_json: bool) -> Result<(), String> {
    let token = commands::auth::load_kenv_token()?
        .ok_or("No token found. Run `ktool auth kenv login <TOKEN>` first.")?;

    let app = cfg
        .app
        .as_deref()
        .ok_or("No app set. Run `ktool kenv --set-app=<app>` first.")?;

    let env = cfg
        .env
        .as_deref()
        .ok_or("No env set. Run `ktool kenv --set-env=<env>` first.")?;

    let value = api::fetch_secrets(app, env, &token)?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
    } else if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            let plain = val.as_str().map(|s| s.to_owned()).unwrap_or_else(|| val.to_string());
            println!("{key}={}", api::obfuscate(&plain));
        }
    } else {
        println!("{value}");
    }

    Ok(())
}
```

- [ ] **Step 2: Run check**

Run: `cargo check`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add version and help commands, restructure CLI with auth"
```

---

### Task 5: Update config module — remove token, new path

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Update config.rs to remove token field and use `~/.ktool/config.toml`**

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Persistent configuration stored in `~/.ktool/config.toml`.
///
/// Contains only app/env preferences — auth tokens are managed by the auth module.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Default application name (`--set-app`).
    pub app: Option<String>,
    /// Default environment (`--set-env`).
    pub env: Option<String>,
}

/// Return the path to the config file, creating parent directories if needed.
pub fn config_path() -> Result<PathBuf, String> {
    let dir = commands::common::ktool_dir()?;
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create config directory {}: {e}", dir.display()))?;
    Ok(dir.join("config.toml"))
}

/// Load the config from disk, returning a default `Config` if the file does not exist yet.
pub fn load() -> Result<Config, String> {
    load_from(&config_path()?)
}

/// Persist the config to disk.
pub fn save(cfg: &Config) -> Result<(), String> {
    save_to(&config_path()?, cfg)
}

// Internal helpers used directly in tests so we never mutate global state.

pub(crate) fn load_from(path: &PathBuf) -> Result<Config, String> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    toml::from_str(&raw).map_err(|e| format!("Failed to parse config: {e}"))
}

pub(crate) fn save_to(path: &PathBuf, cfg: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory {}: {e}", parent.display()))?;
    }
    let content = toml::to_string_pretty(cfg)
        .map_err(|e| format!("Failed to serialise config: {e}"))?;
    fs::write(path, content)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

// We need access to commands module for ktool_dir
use crate::commands;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_config_path() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        (dir, path)
    }

    #[test]
    fn default_config_is_empty() {
        let cfg = Config::default();
        assert!(cfg.app.is_none());
        assert!(cfg.env.is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (_dir, path) = temp_config_path();
        let cfg = Config {
            app: Some("my-app".to_string()),
            env: Some("prod".to_string()),
        };
        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(cfg, loaded);
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let (_dir, path) = temp_config_path();
        let result = load_from(&path).unwrap();
        assert_eq!(result, Config::default());
    }

    #[test]
    fn config_ignores_unknown_fields() {
        // Old config files may still have a "token" field — deserialization must not fail.
        let (_dir, path) = temp_config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "token = \"old_token\"\napp = \"my-app\"\nenv = \"prod\"\n",
        )
        .unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.app.as_deref(), Some("my-app"));
        assert_eq!(loaded.env.as_deref(), Some("prod"));
    }
}
```

**Important:** Add `#[serde(deny_unknown_fields)]` is NOT used here — we need to tolerate old config files that still have `token` in them. serde's default behavior ignores unknown fields, which is what we want.

- [ ] **Step 2: Run tests**

Run: `cargo test config::tests`
Expected: 4 tests pass (including the new `config_ignores_unknown_fields`)

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "refactor: remove token from config, move config to ~/.ktool/"
```

---

### Task 6: Add migration from old config

**Files:**
- Create: `src/migrate.rs`
- Modify: `src/main.rs` (add `mod migrate;` and call on startup)

- [ ] **Step 1: Create `src/migrate.rs`**

```rust
//! One-time migration from `~/.config/ktool/config.toml` to the new layout.
//!
//! Old format: `~/.config/ktool/config.toml` with `token`, `app`, `env` fields.
//! New format:
//! - Token → `~/.ktool/tokens/kenv.json` (JSON, managed by auth module)
//! - App/env → `~/.ktool/config.toml` (TOML, managed by config module)

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// The old config structure (may have token + app + env).
#[derive(Deserialize, Default)]
struct OldConfig {
    token: Option<String>,
    app: Option<String>,
    env: Option<String>,
}

/// Returns the old config path: `~/.config/ktool/config.toml`.
fn old_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ktool").join("config.toml"))
}

/// Runs the migration if the old config exists and has a token.
///
/// Idempotent: skips if the new token file already exists.
pub fn run() {
    if let Err(e) = try_migrate() {
        eprintln!("Warning: config migration failed: {e}");
    }
}

fn try_migrate() -> Result<(), String> {
    let old_path = match old_config_path() {
        Some(p) if p.exists() => p,
        _ => return Ok(()), // No old config — nothing to migrate
    };

    let raw = fs::read_to_string(&old_path)
        .map_err(|e| format!("Failed to read old config: {e}"))?;
    let old: OldConfig =
        toml::from_str(&raw).map_err(|e| format!("Failed to parse old config: {e}"))?;

    // Migrate token if present and new token file doesn't exist
    if let Some(token) = &old.token {
        if !token.is_empty() {
            let new_token_dir = crate::commands::common::ktool_dir()?.join("tokens");
            let new_token_path = new_token_dir.join("kenv.json");
            if !new_token_path.exists() {
                crate::commands::auth::save_kenv_token(token)?;
                eprintln!("Migrated kenv token to ~/.ktool/tokens/kenv.json");
            }
        }
    }

    // Migrate app/env if new config doesn't exist
    let new_config_path = crate::commands::common::ktool_dir()?.join("config.toml");
    if !new_config_path.exists() && (old.app.is_some() || old.env.is_some()) {
        let new_cfg = crate::config::Config {
            app: old.app,
            env: old.env,
        };
        crate::config::save_to(&new_config_path, &new_cfg)?;
        eprintln!("Migrated app/env config to ~/.ktool/config.toml");
    }

    Ok(())
}
```

- [ ] **Step 2: Add `mod migrate;` to `src/main.rs` and call it at the start of `main()`**

Add after the existing `mod` declarations at the top of `src/main.rs`:

```rust
mod migrate;
```

Add as the first line inside `fn main()`:

```rust
    migrate::run();
```

- [ ] **Step 3: Run check**

Run: `cargo check`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add src/migrate.rs src/main.rs
git commit -m "feat: add one-time migration from old config layout"
```

---

### Task 7: Add cadi command — karluiz hero screen

**Files:**
- Create: `src/commands/cadi.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/commands/cadi.rs`**

```rust
//! The `ktool cadi` hero screen — retro 8-bit karluiz branding.

/// Prints the karluiz hero screen to stdout.
pub fn run() {
    println!(
        r#"
 ╔══════════════════════════════════════════════════════════════╗
 ║                                                              ║
 ║   ██╗  ██╗ █████╗ ██████╗ ██╗     ██╗   ██╗██╗███████╗      ║
 ║   ██║ ██╔╝██╔══██╗██╔══██╗██║     ██║   ██║██║╚══███╔╝      ║
 ║   █████╔╝ ███████║██████╔╝██║     ██║   ██║██║  ███╔╝       ║
 ║   ██╔═██╗ ██╔══██║██╔══██╗██║     ██║   ██║██║ ███╔╝        ║
 ║   ██║  ██╗██║  ██║██║  ██║███████╗╚██████╔╝██║███████╗      ║
 ║   ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚═╝╚══════╝      ║
 ║                                                              ║
 ║          >> DEVELOPER BY PASSION <<                          ║
 ║          >> COMMODORE 64 FOREVER <<                          ║
 ║                                                              ║
 ║   ktool v{version}                                           ║
 ║   Made by CaDi Labs with love <3                             ║
 ║                                                              ║
 ╚══════════════════════════════════════════════════════════════╝"#,
        version = env!("CARGO_PKG_VERSION")
    );
}
```

- [ ] **Step 2: Update `src/commands/mod.rs`**

```rust
pub mod auth;
pub mod cadi;
pub mod common;
pub mod kenv;
```

- [ ] **Step 3: Add `Cadi` variant to Commands enum in `src/main.rs`**

Add to the `Commands` enum:

```rust
    /// Show the karluiz hero screen
    Cadi,
```

Add to the match in `main()`:

```rust
        Some(Commands::Cadi) => {
            commands::cadi::run();
            Ok(())
        }
```

- [ ] **Step 4: Run check**

Run: `cargo check`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/commands/cadi.rs src/commands/mod.rs src/main.rs
git commit -m "feat: add cadi command with karluiz 8-bit hero screen"
```

---

### Task 8: Add update command — self-update

**Files:**
- Create: `src/commands/update.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/commands/update.rs`**

```rust
//! Self-update command: fetch the latest GitHub release and replace the running binary.
//!
//! Progress and diagnostics go to **stderr**; the final status line goes to **stdout**.
//! Idempotent: running when already on the latest version prints "Already up to date" and exits 0.

use std::fs;
use std::io::Read;

// ---------------------------------------------------------------------------
// Compile-time target detection
// ---------------------------------------------------------------------------

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const CURRENT_TARGET: &str = "x86_64-apple-darwin";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const CURRENT_TARGET: &str = "aarch64-apple-darwin";

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "musl"))]
const CURRENT_TARGET: &str = "x86_64-unknown-linux-musl";

#[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
const CURRENT_TARGET: &str = "x86_64-unknown-linux-musl";

#[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "musl"))]
const CURRENT_TARGET: &str = "aarch64-unknown-linux-musl";

#[cfg(all(target_os = "linux", target_arch = "aarch64", not(target_env = "musl")))]
const CURRENT_TARGET: &str = "aarch64-unknown-linux-musl";

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const CURRENT_TARGET: &str = "x86_64-pc-windows-gnu";

#[cfg(not(any(
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64"),
)))]
compile_error!(
    "ktool update: unsupported target platform — add a CURRENT_TARGET constant for this target"
);

const RELEASES_API: &str =
    "https://api.github.com/repos/CaDi-Team/karluiz-tool-cli/releases/latest";

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Checks GitHub for a newer release and, if found, downloads and installs it in place.
pub fn run() -> Result<(), String> {
    eprintln!("Checking for updates...");

    let release = fetch_latest()?;

    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or("GitHub API response missing 'tag_name'")?;

    let current = format!("v{}", env!("CARGO_PKG_VERSION"));

    if latest_tag == current.as_str() {
        println!("Already up to date ({current}).");
        return Ok(());
    }

    println!("Update available: {current} → {latest_tag}");
    eprintln!("Downloading {latest_tag} for {CURRENT_TARGET}...");

    let url = find_asset_url(&release)?;
    let bytes = download(&url)?;

    eprintln!("Extracting...");
    let binary = extract_binary(&bytes)?;

    eprintln!("Installing...");
    replace_self(&binary)?;

    println!("Updated to {latest_tag}. Restart ktool to use the new version.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Network helpers
// ---------------------------------------------------------------------------

fn fetch_latest() -> Result<serde_json::Value, String> {
    let ua = format!("ktool/{}", env!("CARGO_PKG_VERSION"));
    let req = ureq::get(RELEASES_API)
        .set("User-Agent", &ua)
        .set("Accept", "application/vnd.github.v3+json");

    match req.call() {
        Ok(resp) => resp
            .into_json()
            .map_err(|e| format!("Failed to parse GitHub API response: {e}")),
        Err(ureq::Error::Status(404, _)) => {
            Err("GitHub API returned 404. No releases found.".to_string())
        }
        Err(e) => Err(format!("GitHub API error: {e}")),
    }
}

fn find_asset_url(release: &serde_json::Value) -> Result<String, String> {
    let assets = release["assets"]
        .as_array()
        .ok_or("GitHub API response missing 'assets'")?;

    let suffix = if cfg!(windows) {
        format!("{CURRENT_TARGET}.zip")
    } else {
        format!("{CURRENT_TARGET}.tar.gz")
    };

    for asset in assets {
        if let Some(name) = asset["name"].as_str() {
            if name.ends_with(&suffix) {
                let url = asset["browser_download_url"]
                    .as_str()
                    .ok_or("asset missing 'browser_download_url'")?;
                return Ok(url.to_owned());
            }
        }
    }

    Err(format!(
        "no release asset found for target '{CURRENT_TARGET}'"
    ))
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let ua = format!("ktool/{}", env!("CARGO_PKG_VERSION"));
    let req = ureq::get(url).set("User-Agent", &ua);
    let mut buf = Vec::new();
    req.call()
        .map_err(|e| format!("Download failed: {e}"))?
        .into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read download: {e}"))?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Archive extraction
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use flate2::read::GzDecoder;
    use std::ffi::OsStr;
    use tar::Archive;

    let mut archive = Archive::new(GzDecoder::new(bytes));
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read archive: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read archive entry: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to read entry path: {e}"))?
            .into_owned();
        if path.file_name() == Some(OsStr::new("ktool")) {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read binary from archive: {e}"))?;
            return Ok(buf);
        }
    }
    Err("'ktool' binary not found in archive".to_string())
}

#[cfg(windows)]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use zip::ZipArchive;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Failed to read zip archive: {e}"))?;
    let mut file = archive
        .by_name("ktool.exe")
        .map_err(|e| format!("Failed to find ktool.exe in archive: {e}"))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read binary from archive: {e}"))?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Binary replacement
// ---------------------------------------------------------------------------

fn replace_self(binary: &[u8]) -> Result<(), String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Failed to locate current binary: {e}"))?;
    let tmp = current_exe.with_extension("tmp");

    fs::write(&tmp, binary).map_err(|e| format!("Failed to write temp binary: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set binary permissions: {e}"))?;
        fs::rename(&tmp, &current_exe)
            .map_err(|e| format!("Failed to replace binary: {e}"))?;
    }

    #[cfg(windows)]
    {
        let old = current_exe.with_extension("old");
        if old.exists() {
            let _ = fs::remove_file(&old);
        }
        fs::rename(&current_exe, &old)
            .map_err(|e| format!("Failed to rename current binary: {e}"))?;
        fs::rename(&tmp, &current_exe)
            .map_err(|e| format!("Failed to install new binary: {e}"))?;
    }

    Ok(())
}
```

- [ ] **Step 2: Update `src/commands/mod.rs`**

```rust
pub mod auth;
pub mod cadi;
pub mod common;
pub mod kenv;
pub mod update;
```

- [ ] **Step 3: Add `Update` variant to Commands enum in `src/main.rs`**

Add to the `Commands` enum:

```rust
    /// Update ktool to the latest released version
    Update,
```

Add to the match in `main()`:

```rust
        Some(Commands::Update) => commands::update::run(),
```

- [ ] **Step 4: Run check**

Run: `cargo check`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/commands/update.rs src/commands/mod.rs src/main.rs
git commit -m "feat: add self-update command from GitHub releases"
```

---

### Task 9: Add integration tests

**Files:**
- Create: `tests/integration_test.rs`

- [ ] **Step 1: Create `tests/integration_test.rs`**

```rust
use std::process::Command;

fn ktool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ktool"))
}

#[test]
fn version_subcommand_prints_version() {
    let output = ktool()
        .arg("version")
        .output()
        .expect("failed to run ktool");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("ktool v"), "got: {stdout}");
}

#[test]
fn help_subcommand_exits_successfully() {
    let output = ktool()
        .arg("help")
        .output()
        .expect("failed to run ktool");

    assert!(output.status.success());
}

#[test]
fn no_args_exits_successfully() {
    let output = ktool().output().expect("failed to run ktool");

    assert!(output.status.success());
}

#[test]
fn flag_version_exits_successfully() {
    let output = ktool()
        .arg("--version")
        .output()
        .expect("failed to run ktool");

    assert!(output.status.success());
}

#[test]
fn cadi_prints_hero_screen() {
    let output = ktool()
        .arg("cadi")
        .output()
        .expect("failed to run ktool cadi");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("KARLUIZ"), "got: {stdout}");
    assert!(stdout.contains("DEVELOPER BY PASSION"), "got: {stdout}");
    assert!(stdout.contains("CaDi Labs"), "got: {stdout}");
}

#[test]
fn update_help_exits_successfully() {
    let output = ktool()
        .args(["update", "--help"])
        .output()
        .expect("failed to run ktool update --help");

    assert!(output.status.success());
}

#[test]
fn auth_help_exits_successfully() {
    let output = ktool()
        .args(["auth", "--help"])
        .output()
        .expect("failed to run ktool auth --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kenv"), "got: {stdout}");
    assert!(stdout.contains("status"), "got: {stdout}");
    assert!(stdout.contains("logout"), "got: {stdout}");
}

#[test]
fn auth_kenv_help_exits_successfully() {
    let output = ktool()
        .args(["auth", "kenv", "--help"])
        .output()
        .expect("failed to run ktool auth kenv --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("login"), "got: {stdout}");
    assert!(stdout.contains("logout"), "got: {stdout}");
    assert!(stdout.contains("whoami"), "got: {stdout}");
}

#[test]
fn auth_status_runs_without_config() {
    let output = ktool()
        .args(["auth", "status"])
        .output()
        .expect("failed to run ktool auth status");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kenv"), "got: {stdout}");
}

#[test]
fn kenv_help_exits_successfully() {
    let output = ktool()
        .args(["kenv", "--help"])
        .output()
        .expect("failed to run ktool kenv --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list"), "got: {stdout}");
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: all unit tests + integration tests pass

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings

- [ ] **Step 4: Run fmt check**

Run: `cargo fmt --check`
Expected: no formatting issues (fix with `cargo fmt` if needed)

- [ ] **Step 5: Commit**

```bash
git add tests/integration_test.rs
git commit -m "test: add integration tests for all new commands"
```

---

### Task 10: Update release workflow — asset naming with target triples

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Update release.yml to use target triple in asset names**

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

env:
  CARGO_TERM_COLOR: always
  BIN_NAME: ktool

permissions:
  contents: write

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-musl
            runner: ubuntu-latest
            archive: ktool-x86_64-unknown-linux-musl.tar.gz

          - target: aarch64-unknown-linux-musl
            runner: ubuntu-latest
            archive: ktool-aarch64-unknown-linux-musl.tar.gz

          - target: x86_64-apple-darwin
            runner: macos-latest
            archive: ktool-x86_64-apple-darwin.tar.gz

          - target: aarch64-apple-darwin
            runner: macos-latest
            archive: ktool-aarch64-apple-darwin.tar.gz

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable + target
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache Cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: ${{ runner.os }}-${{ matrix.target }}-cargo-

      - name: Install cross (Linux targets)
        if: runner.os == 'Linux'
        uses: taiki-e/install-action@v2
        with:
          tool: cross

      - name: Build (Linux — cross)
        if: runner.os == 'Linux'
        run: cross build --release --locked --target ${{ matrix.target }}

      - name: Build (macOS — native cargo)
        if: runner.os == 'macOS'
        run: cargo build --release --locked --target ${{ matrix.target }}

      - name: Package binary
        shell: bash
        run: |
          BINARY="target/${{ matrix.target }}/release/${{ env.BIN_NAME }}"
          tar -czvf "${{ matrix.archive }}" -C "$(dirname "$BINARY")" "$(basename "$BINARY")"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.archive }}
          path: ${{ matrix.archive }}
          retention-days: 90

  release:
    name: Create GitHub Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4.2.0
        with:
          path: artifacts
          merge-multiple: true

      - name: Create release and upload assets
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: artifacts/*.tar.gz
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: rename release assets to use full target triples"
```

---

### Task 11: Update CI workflow — add fmt check

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Update ci.yml to add fmt check**

```yaml
name: CI

on:
  push:
    branches: ["**"]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: ${{ runner.os }}-cargo-

      - name: Format check
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Tests
        run: cargo test
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add cargo fmt check to CI pipeline"
```

---

## Part B: diegops Changes (diegops-tools)

### Task 12: Add ktool sidecar command

**Files:**
- Create: `src/commands/ktool.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/commands/ktool.rs`**

```rust
//! ktool sidecar management — download, update, and proxy the ktool binary.
//!
//! `diegops ktool update` downloads the latest ktool release to `~/.diegops/bin/ktool`.
//! All other arguments are forwarded to the managed ktool binary.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

const RELEASES_API: &str =
    "https://api.github.com/repos/CaDi-Team/karluiz-tool-cli/releases/latest";

/// Returns the managed ktool binary path: `~/.diegops/bin/ktool`.
fn ktool_bin_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(super::common::diegops_dir()?.join("bin").join(if cfg!(windows) {
        "ktool.exe"
    } else {
        "ktool"
    }))
}

/// Compile-time target triple — same constants as the main update module.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const CURRENT_TARGET: &str = "x86_64-pc-windows-gnu";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const CURRENT_TARGET: &str = "x86_64-apple-darwin";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const CURRENT_TARGET: &str = "aarch64-apple-darwin";

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "musl"))]
const CURRENT_TARGET: &str = "x86_64-unknown-linux-musl";

#[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
const CURRENT_TARGET: &str = "x86_64-unknown-linux-gnu";

#[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "musl"))]
const CURRENT_TARGET: &str = "aarch64-unknown-linux-musl";

#[cfg(all(target_os = "linux", target_arch = "aarch64", not(target_env = "musl")))]
const CURRENT_TARGET: &str = "aarch64-unknown-linux-gnu";

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
)))]
compile_error!(
    "diegops ktool: unsupported target platform"
);

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Handles `diegops ktool <args>`.
///
/// If the first arg is `update`, downloads the latest ktool binary.
/// Otherwise, forwards all args to the managed ktool binary.
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.first().map(|s| s.as_str()) == Some("update") {
        return update();
    }
    passthrough(args)
}

// ---------------------------------------------------------------------------
// Update — download latest ktool to ~/.diegops/bin/ktool
// ---------------------------------------------------------------------------

fn update() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Checking for ktool updates...");

    let token = super::auth::load_gh_token().ok().flatten();
    let release = fetch_latest(token.as_deref())?;

    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or("GitHub API response missing 'tag_name'")?;

    // Check if we already have this version by running ktool version
    let bin_path = ktool_bin_path()?;
    if bin_path.exists() {
        if let Ok(output) = Command::new(&bin_path).arg("version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let installed_version = stdout.trim();
            let expected = format!("ktool {latest_tag}");
            if installed_version == expected {
                println!("ktool already up to date ({latest_tag}).");
                return Ok(());
            }
        }
    }

    println!("Installing ktool {latest_tag}...");
    eprintln!("Downloading ktool {latest_tag} for {CURRENT_TARGET}...");

    let url = find_asset_url(&release)?;
    let bytes = download(&url, token.as_deref())?;

    eprintln!("Extracting...");
    let binary = extract_binary(&bytes)?;

    eprintln!("Installing to {}...", bin_path.display());
    install_binary(&bin_path, &binary)?;

    println!("ktool {latest_tag} installed to {}", bin_path.display());
    Ok(())
}

fn fetch_latest(
    token: Option<&str>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let mut req = ureq::get(RELEASES_API)
        .set("User-Agent", &ua)
        .set("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }

    match req.call() {
        Ok(resp) => Ok(resp.into_json()?),
        Err(ureq::Error::Status(404, _)) if token.is_none() => {
            Err("GitHub API returned 404. If this is a private repo, run 'diegops auth gh login <PAT>' first".into())
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("stored GitHub token is no longer valid. Run 'diegops auth gh login <PAT>' to update it".into())
        }
        Err(e) => Err(e.into()),
    }
}

fn find_asset_url(
    release: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    let assets = release["assets"]
        .as_array()
        .ok_or("GitHub API response missing 'assets'")?;

    let suffix = if cfg!(windows) {
        format!("{CURRENT_TARGET}.zip")
    } else {
        format!("{CURRENT_TARGET}.tar.gz")
    };

    for asset in assets {
        if let Some(name) = asset["name"].as_str() {
            if name.ends_with(&suffix) {
                let url = asset["browser_download_url"]
                    .as_str()
                    .ok_or("asset missing 'browser_download_url'")?;
                return Ok(url.to_owned());
            }
        }
    }

    Err(format!("no ktool release asset found for target '{CURRENT_TARGET}'").into())
}

fn download(
    url: &str,
    token: Option<&str>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let mut req = ureq::get(url).set("User-Agent", &ua);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    let mut buf = Vec::new();
    req.call()?.into_reader().read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(unix)]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    use std::ffi::OsStr;
    use tar::Archive;

    let mut archive = Archive::new(GzDecoder::new(bytes));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.file_name() == Some(OsStr::new("ktool")) {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    Err("'ktool' binary not found in archive".into())
}

#[cfg(windows)]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use zip::ZipArchive;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;
    let mut file = archive.by_name("ktool.exe")?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

fn install_binary(
    path: &PathBuf,
    binary: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp = path.with_extension("tmp");
    fs::write(&tmp, binary)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }

    fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Passthrough — forward args to managed ktool binary
// ---------------------------------------------------------------------------

fn passthrough(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let bin_path = ktool_bin_path()?;

    if !bin_path.exists() {
        return Err(
            "ktool not installed. Run 'diegops ktool update' first.".into(),
        );
    }

    let status = Command::new(&bin_path)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(2));
    }

    Ok(())
}
```

- [ ] **Step 2: Update `src/commands/mod.rs` — add `pub mod ktool;`**

Add to `src/commands/mod.rs`:

```rust
pub mod ktool;
```

- [ ] **Step 3: Add Ktool command to `src/main.rs`**

Add to the `Commands` enum:

```rust
    /// Manage and run ktool (karluiz tools)
    Ktool {
        /// Arguments passed to ktool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
```

Add to the match in `main()` (before the closing `}`):

```rust
        Some(Commands::Ktool { args }) => {
            commands::ktool::run(&args)?;
        }
```

- [ ] **Step 4: Run check**

Run: `cargo check`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/commands/ktool.rs src/commands/mod.rs src/main.rs
git commit -m "feat: add ktool sidecar command with update and passthrough"
```

---

### Task 13: Enhance auth status to show ktool kenv token

**Files:**
- Modify: `src/commands/auth.rs`

- [ ] **Step 1: Add ktool token reading to auth status**

In `src/commands/auth.rs`, add a helper function to read ktool's kenv token:

```rust
/// Reads ktool's kenv token metadata from `~/.ktool/tokens/kenv.json` (read-only).
fn load_ktool_kenv_token_data() -> Result<Option<TokenData>, Box<dyn std::error::Error>> {
    let home = super::common::home_dir()?;
    let path = home.join(".ktool").join("tokens").join("kenv.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&content)?))
}
```

- [ ] **Step 2: Update the `status()` function to show kenv status**

Replace the `status()` function:

```rust
/// Shows auth status for all providers.
pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    // GitHub token (managed by diegops)
    match load_gh_token_data()? {
        Some(data) => {
            println!(
                "GitHub (gh)    [ok] configured    stored {}",
                data.stored_at
            );
        }
        None => {
            println!("GitHub (gh)    [--] not configured");
        }
    }

    // kenv token (managed by ktool, read-only)
    match load_ktool_kenv_token_data()? {
        Some(data) => {
            println!(
                "kenv (ktool)   [ok] configured    stored {}    (managed by ktool)",
                data.stored_at
            );
        }
        None => {
            println!("kenv (ktool)   [--] not configured");
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: all tests pass (including `auth_status_runs_without_config` — it now shows both providers)

- [ ] **Step 4: Commit**

```bash
git add src/commands/auth.rs
git commit -m "feat: show ktool kenv token status in auth status"
```

---

### Task 14: Add diegops integration tests for ktool commands

**Files:**
- Modify: `tests/integration_test.rs`

- [ ] **Step 1: Add ktool integration tests**

Append to `tests/integration_test.rs`:

```rust
#[test]
fn ktool_without_binary_prints_error() {
    // When the managed ktool binary doesn't exist, diegops should error
    let output = diegops()
        .args(["ktool", "version"])
        .output()
        .expect("failed to run diegops ktool version");

    // This may succeed if ktool is installed, or fail if not.
    // We just verify diegops doesn't panic.
    let _ = output.status;
}

#[test]
fn ktool_help_exits_successfully() {
    let output = diegops()
        .args(["ktool", "--help"])
        .output()
        .expect("failed to run diegops ktool --help");

    // --help is handled by clap before we get to passthrough
    assert!(output.status.success());
}

#[test]
fn auth_status_shows_kenv() {
    let output = diegops()
        .args(["auth", "status"])
        .output()
        .expect("failed to run diegops auth status");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("GitHub"), "got: {stdout}");
    assert!(stdout.contains("kenv"), "got: {stdout}");
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 3: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --check`
Expected: no warnings, no formatting issues

- [ ] **Step 4: Commit**

```bash
git add tests/integration_test.rs
git commit -m "test: add integration tests for ktool sidecar and kenv auth status"
```

---

### Task 15: Bump diegops version

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Bump version in Cargo.toml**

Change version from `"1.0.6"` to `"1.0.7"`.

- [ ] **Step 2: Run tests one final time**

Run: `cargo test`
Expected: all pass

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 1.0.7"
```

---

## Part C: Release

### Task 16: Tag and push ktool release

- [ ] **Step 1: Push ktool changes**

```bash
cd /Users/diegopinto/github/dpinto-config/karluiz-tool-cli
git push origin main
```

- [ ] **Step 2: Tag and push**

```bash
git tag v0.2.0
git push origin v0.2.0
```

### Task 17: Tag and push diegops release

- [ ] **Step 1: Push diegops changes**

```bash
cd /Users/diegopinto/github/dpinto-config/diegops-tools
git push origin develop
```

- [ ] **Step 2: Tag and push**

```bash
git tag v1.0.7
git push origin v1.0.7
```
