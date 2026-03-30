# `diegops secrets` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `diegops secrets` command that syncs sensitive workstation files (SSH keys, kubeconfigs) through HashiCorp Vault KV v2 with push/pull/status/init subcommands.

**Architecture:** New `src/commands/secrets.rs` module following the same pattern as `vault.rs`. Config-driven (YAML), uses `vault` CLI for Vault interaction, base64 encodes/decodes file content. Vault pre-flight checks are extracted from `vault.rs` into `common.rs` to share between both commands.

**Tech Stack:** Rust, clap v4, serde/serde_yaml, base64 crate (already in Cargo.toml), std::process::Command for vault CLI.

**Spec:** `docs/superpowers/specs/2026-03-28-secrets-command-design.md`

---

### Task 1: Extract vault pre-flight checks into common.rs

**Files:**
- Modify: `src/commands/common.rs`
- Modify: `src/commands/vault.rs`

The three vault pre-flight functions (`check_vault_binary`, `check_vault_addr`, `check_vault_auth`) currently live in `vault.rs`. Move them to `common.rs` so `secrets.rs` can reuse them.

- [ ] **Step 1: Add vault pre-flight functions to common.rs**

Add these three functions at the bottom of `src/commands/common.rs`, above the `#[cfg(test)]` block:

```rust
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
```

- [ ] **Step 2: Update vault.rs to use common functions**

In `src/commands/vault.rs`, replace the three private functions (`check_vault_binary`, `check_vault_addr`, `check_vault_auth`) and update the `apply` function to call the common versions.

Remove lines 404-436 (the three `fn check_*` functions) and update `apply` (lines 133-135):

```rust
    // Pre-flight checks — fatal, exit immediately on failure
    super::common::check_vault_binary()?;
    super::common::check_vault_addr()?;
    super::common::check_vault_auth()?;
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All existing tests pass (no behavior change).

- [ ] **Step 4: Commit**

```bash
git add src/commands/common.rs src/commands/vault.rs
git commit -m "refactor: extract vault pre-flight checks into common.rs"
```

---

### Task 2: Config structs and parsing with tests

**Files:**
- Create: `src/commands/secrets.rs`
- Modify: `src/commands/mod.rs`

- [ ] **Step 1: Write config parsing tests**

Create `src/commands/secrets.rs` with the config structs and tests:

```rust
//! Vault-backed workstation file sync — push/pull sensitive files via Vault KV v2.
//!
//! Config file (`secrets.yaml`) format:
//! ```yaml
//! folders:
//!   - dest: $HOME/.ssh
//!     vault_path: secret/workstation/ssh
//!     keys:
//!       - id_ed25519
//!       - id_ed25519.pub
//!     dir_mode: "0700"
//!
//!   - dest: $HOME/.kube
//!     vault_path: secret/workstation/kube
//!     keys: "*"
//! ```
//!
//! Config resolution order:
//! 1. `--config <path>` CLI flag
//! 2. `DIEGOPS_SECRETS_CONFIG` environment variable
//! 3. `~/.diegops/secrets.yaml` (default)

use serde::Deserialize;
use std::path::Path;

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

#[derive(Deserialize)]
#[serde(untagged)]
enum KeyEntry {
    WithMode { name: String, mode: Option<String> },
    Simple(String),
}

impl KeyEntry {
    fn name(&self) -> &str {
        match self {
            KeyEntry::Simple(s) => s,
            KeyEntry::WithMode { name, .. } => name,
        }
    }

    fn mode(&self) -> Option<&str> {
        match self {
            KeyEntry::Simple(_) => None,
            KeyEntry::WithMode { mode, .. } => mode.as_deref(),
        }
    }
}

// ---------------------------------------------------------------------------
// Permission helpers
// ---------------------------------------------------------------------------

/// Returns the default Unix file mode for a given filename.
///
/// Public keys (`*.pub`) get `0644`, everything else gets `0600`.
fn default_mode(filename: &str) -> u32 {
    if filename.ends_with(".pub") {
        0o644
    } else {
        0o600
    }
}

/// Parses a mode string like `"0600"` into a `u32`.
fn parse_mode(mode_str: &str) -> Result<u32, Box<dyn std::error::Error>> {
    u32::from_str_radix(mode_str.trim_start_matches('0'), 8)
        .map_err(|e| format!("invalid mode '{mode_str}': {e}").into())
}

/// Returns the effective file mode for a key entry.
fn effective_mode(entry: &KeyEntry) -> Result<u32, Box<dyn std::error::Error>> {
    match entry.mode() {
        Some(m) => parse_mode(m),
        None => Ok(default_mode(entry.name())),
    }
}

/// Returns the directory mode from config, or the default `0700`.
fn dir_mode(folder: &Folder) -> Result<u32, Box<dyn std::error::Error>> {
    match &folder.dir_mode {
        Some(m) => parse_mode(m),
        None => Ok(0o700),
    }
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Loads and validates the secrets config from the resolved path.
fn load_config(path: Option<&Path>) -> Result<SecretsConfig, Box<dyn std::error::Error>> {
    let config_path =
        super::common::resolve_config_path(path, "DIEGOPS_SECRETS_CONFIG", "secrets.yaml")?;
    let content = super::common::read_config_file(&config_path)?;
    let config: SecretsConfig = serde_yaml::from_str(&content)
        .map_err(|e| format!("invalid secrets config {}: {e}", config_path.display()))?;

    // Validate KeySelector::All values — must be exactly "*"
    for folder in &config.folders {
        if let KeySelector::All(ref s) = folder.keys {
            if s != "*" {
                return Err(format!(
                    "invalid key selector '{}' for vault_path '{}' — use \"*\" or a list of filenames",
                    s, folder.vault_path
                )
                .into());
            }
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_explicit_keys() {
        let yaml = r#"
folders:
  - dest: $HOME/.ssh
    vault_path: secret/workstation/ssh
    keys:
      - id_ed25519
      - id_ed25519.pub
    dir_mode: "0700"
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.folders.len(), 1);
        assert_eq!(config.folders[0].dest, "$HOME/.ssh");
        assert_eq!(config.folders[0].vault_path, "secret/workstation/ssh");
        assert_eq!(config.folders[0].dir_mode.as_deref(), Some("0700"));
        match &config.folders[0].keys {
            KeySelector::List(keys) => {
                assert_eq!(keys.len(), 2);
                assert_eq!(keys[0].name(), "id_ed25519");
                assert_eq!(keys[1].name(), "id_ed25519.pub");
            }
            KeySelector::All(_) => panic!("expected List"),
        }
    }

    #[test]
    fn parse_config_wildcard_keys() {
        let yaml = r#"
folders:
  - dest: $HOME/.kube
    vault_path: secret/workstation/kube
    keys: "*"
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.folders[0].keys {
            KeySelector::All(s) => assert_eq!(s, "*"),
            KeySelector::List(_) => panic!("expected All"),
        }
    }

    #[test]
    fn parse_config_mixed_keys_with_mode() {
        let yaml = r#"
folders:
  - dest: $HOME/.ssh
    vault_path: secret/workstation/ssh
    keys:
      - name: id_ed25519
        mode: "0600"
      - id_ed25519.pub
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.folders[0].keys {
            KeySelector::List(keys) => {
                assert_eq!(keys[0].name(), "id_ed25519");
                assert_eq!(keys[0].mode(), Some("0600"));
                assert_eq!(keys[1].name(), "id_ed25519.pub");
                assert_eq!(keys[1].mode(), None);
            }
            KeySelector::All(_) => panic!("expected List"),
        }
    }

    #[test]
    fn parse_config_no_dir_mode_defaults_none() {
        let yaml = r#"
folders:
  - dest: $HOME/.kube
    vault_path: secret/workstation/kube
    keys: "*"
"#;
        let config: SecretsConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.folders[0].dir_mode.is_none());
    }

    #[test]
    fn default_mode_private_key() {
        assert_eq!(default_mode("id_ed25519"), 0o600);
        assert_eq!(default_mode("id_rsa"), 0o600);
    }

    #[test]
    fn default_mode_public_key() {
        assert_eq!(default_mode("id_ed25519.pub"), 0o644);
        assert_eq!(default_mode("id_rsa.pub"), 0o644);
    }

    #[test]
    fn default_mode_other_files() {
        assert_eq!(default_mode("config"), 0o600);
        assert_eq!(default_mode("known_hosts"), 0o600);
    }

    #[test]
    fn parse_mode_octal() {
        assert_eq!(parse_mode("0600").unwrap(), 0o600);
        assert_eq!(parse_mode("0644").unwrap(), 0o644);
        assert_eq!(parse_mode("0700").unwrap(), 0o700);
        assert_eq!(parse_mode("0755").unwrap(), 0o755);
    }

    #[test]
    fn parse_mode_invalid() {
        assert!(parse_mode("abc").is_err());
    }

    #[test]
    fn effective_mode_uses_override() {
        let entry = KeyEntry::WithMode {
            name: "id_ed25519.pub".to_string(),
            mode: Some("0600".to_string()),
        };
        // Override should win over smart default (which would be 0644)
        assert_eq!(effective_mode(&entry).unwrap(), 0o600);
    }

    #[test]
    fn effective_mode_uses_default() {
        let entry = KeyEntry::Simple("id_ed25519.pub".to_string());
        assert_eq!(effective_mode(&entry).unwrap(), 0o644);
    }

    #[test]
    fn dir_mode_default() {
        let folder = Folder {
            dest: "$HOME/.ssh".to_string(),
            vault_path: "secret/ssh".to_string(),
            keys: KeySelector::All("*".to_string()),
            dir_mode: None,
        };
        assert_eq!(dir_mode(&folder).unwrap(), 0o700);
    }

    #[test]
    fn dir_mode_override() {
        let folder = Folder {
            dest: "$HOME/.ssh".to_string(),
            vault_path: "secret/ssh".to_string(),
            keys: KeySelector::All("*".to_string()),
            dir_mode: Some("0755".to_string()),
        };
        assert_eq!(dir_mode(&folder).unwrap(), 0o755);
    }
}
```

- [ ] **Step 2: Register the module in mod.rs**

Add to `src/commands/mod.rs`:

```rust
pub mod secrets;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test secrets`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/commands/secrets.rs src/commands/mod.rs
git commit -m "feat(secrets): add config structs, parsing, and permission helpers"
```

---

### Task 3: Init command and clap wiring

**Files:**
- Modify: `src/commands/secrets.rs`
- Modify: `src/main.rs`
- Modify: `tests/integration_test.rs`

- [ ] **Step 1: Write integration test for init help**

Add to `tests/integration_test.rs`:

```rust
#[test]
fn secrets_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "--help"])
        .output()
        .expect("failed to run diegops secrets --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("init"), "got: {stdout}");
    assert!(stdout.contains("push"), "got: {stdout}");
    assert!(stdout.contains("pull"), "got: {stdout}");
    assert!(stdout.contains("status"), "got: {stdout}");
}

#[test]
fn secrets_init_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "init", "--help"])
        .output()
        .expect("failed to run diegops secrets init --help");

    assert!(output.status.success());
}
```

- [ ] **Step 2: Add clap subcommand enum to secrets.rs**

Add at the top of `src/commands/secrets.rs`, after the module doc comment and before the config structs:

```rust
use std::fs;

// ---------------------------------------------------------------------------
// Clap sub-command definitions
// ---------------------------------------------------------------------------

/// Secrets management sub-commands.
#[derive(clap::Subcommand)]
pub enum SecretsCommand {
    /// Upload local files to Vault (base64-encoded)
    Push {
        /// Only process folders whose expanded dest starts with this prefix.
        #[arg(long)]
        path: Option<String>,

        /// Path to the secrets.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Download files from Vault and write to local paths
    Pull {
        /// Only process folders whose expanded dest starts with this prefix.
        #[arg(long)]
        path: Option<String>,

        /// Path to the secrets.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Compare local files against Vault contents
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
```

- [ ] **Step 3: Add init function and sample config**

Add to `src/commands/secrets.rs`, after `load_config`:

```rust
// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Creates `~/.diegops/secrets.yaml` with a sample config.
///
/// Idempotent: if the file already exists it prints its location and exits 0.
pub fn init(base_dir: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = match base_dir {
        Some(d) => d.to_owned(),
        None => super::common::diegops_dir()?,
    };
    let config_path = dir.join("secrets.yaml");

    if config_path.exists() {
        println!("Config already exists: {}", config_path.display());
        println!("Edit it directly or run `diegops secrets status` to check sync state.");
        return Ok(());
    }

    fs::create_dir_all(&dir)?;
    fs::write(&config_path, SAMPLE_SECRETS_CONFIG)?;

    println!("Created: {}", config_path.display());
    println!("Edit the file to add your folders, then run `diegops secrets push`.");
    Ok(())
}

const SAMPLE_SECRETS_CONFIG: &str = "\
# diegops secrets configuration
#
# Each folder maps a local directory to a Vault KV v2 path.
# Files are stored as base64-encoded values in Vault.
#
# Commands:
#   diegops secrets push    — upload local files to Vault
#   diegops secrets pull    — download files from Vault
#   diegops secrets status  — compare local vs Vault
#
# Path rules:
#   - Use $HOME as a portable prefix (works on Linux, macOS, and WSL).
#   - vault_path uses the logical Vault path (no /data/ segment).
#   - keys: list specific filenames, or use \"*\" to sync all files.
#   - The vault CLI must be installed and authenticated.

folders:

  - dest: $HOME/.ssh
    vault_path: secret/workstation/ssh
    keys:
      - id_ed25519
      - id_ed25519.pub
    dir_mode: \"0700\"

  - dest: $HOME/.kube
    vault_path: secret/workstation/kube
    keys: \"*\"
";
```

- [ ] **Step 4: Add init unit test**

Add to the `#[cfg(test)] mod tests` block in `secrets.rs`:

```rust
    #[test]
    fn init_creates_sample_config() {
        let dir = std::env::temp_dir()
            .join(format!("diegops-test-secrets-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        init(Some(&dir)).unwrap();

        let config_path = dir.join("secrets.yaml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("folders:"));
        assert!(content.contains("vault_path:"));
        assert!(content.contains("keys:"));

        // Idempotent: second call succeeds
        init(Some(&dir)).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 5: Wire into main.rs**

Add the import at the top of `src/main.rs` alongside the others:

```rust
use commands::secrets::SecretsCommand;
```

Add the `Secrets` variant to the `Commands` enum (after the `Vault` variant):

```rust
    /// Sync workstation secrets via Vault (SSH keys, kubeconfigs, etc.)
    #[command(
        long_about = "Sync workstation secrets via Vault — push/pull sensitive files.\n\n\
        Config: ~/.diegops/secrets.yaml (override with --config or $DIEGOPS_SECRETS_CONFIG)\n\
        Requires: vault CLI on PATH, VAULT_ADDR set, authenticated session.\n\
        Files are stored as base64-encoded values in Vault KV v2.\n\n\
        Quick start:\n  \
        diegops secrets init    # create sample config\n  \
        diegops secrets push    # upload local files to Vault\n  \
        diegops secrets pull    # download files from Vault"
    )]
    Secrets {
        #[command(subcommand)]
        cmd: SecretsCommand,
    },
```

Add the match arm in `main()` (after the `Vault` block):

```rust
        Some(Commands::Secrets { cmd }) => {
            let config_path_str;
            match cmd {
                SecretsCommand::Push { path, config } => {
                    config_path_str = config;
                    let cfg = config_path_str.as_deref().map(std::path::Path::new);
                    commands::secrets::push(cfg, path.as_deref())?;
                }
                SecretsCommand::Pull { path, config } => {
                    config_path_str = config;
                    let cfg = config_path_str.as_deref().map(std::path::Path::new);
                    commands::secrets::pull(cfg, path.as_deref())?;
                }
                SecretsCommand::Status { config } => {
                    config_path_str = config;
                    let cfg = config_path_str.as_deref().map(std::path::Path::new);
                    commands::secrets::status(cfg)?;
                }
                SecretsCommand::Init => {
                    commands::secrets::init(None)?;
                }
            }
        }
```

- [ ] **Step 6: Add stub public functions so it compiles**

Add to `src/commands/secrets.rs` after the `init` function:

```rust
/// Pushes local files to Vault.
pub fn push(
    _config_path: Option<&Path>,
    _path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("diegops secrets push: not yet implemented".into())
}

/// Pulls files from Vault to local paths.
pub fn pull(
    _config_path: Option<&Path>,
    _path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("diegops secrets pull: not yet implemented".into())
}

/// Shows sync status between local files and Vault.
pub fn status(_config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    Err("diegops secrets status: not yet implemented".into())
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: All tests pass including new integration and unit tests.

- [ ] **Step 8: Commit**

```bash
git add src/commands/secrets.rs src/main.rs tests/integration_test.rs
git commit -m "feat(secrets): add init command and clap wiring with stubs"
```

---

### Task 4: Vault interaction helpers

**Files:**
- Modify: `src/commands/secrets.rs`

Add the vault read/write helpers and base64 encode/decode that both push and pull will use.

- [ ] **Step 1: Write tests for base64 round-trip and vault response parsing**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn base64_round_trip() {
        let original = b"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI test@host\n";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn base64_round_trip_binary() {
        let original: Vec<u8> = (0..=255).collect();
        let encoded = base64_encode(&original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn parse_vault_kv_response_extracts_data() {
        let json = br#"{
            "data": {
                "data": {
                    "id_ed25519": "c3NoLWtleQ==",
                    "id_ed25519.pub": "cHVibGljLWtleQ=="
                },
                "metadata": { "version": 1 }
            }
        }"#;
        let data = parse_vault_kv_response(json, "secret/ssh").unwrap();
        assert_eq!(data.get("id_ed25519").unwrap().as_str().unwrap(), "c3NoLWtleQ==");
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn parse_vault_kv_response_rejects_bad_structure() {
        let json = br#"{"data": {"wrong": "shape"}}"#;
        assert!(parse_vault_kv_response(json, "secret/ssh").is_err());
    }
```

- [ ] **Step 2: Implement the helpers**

Add to `src/commands/secrets.rs`, after the `load_config` function and before the public entry points:

```rust
// ---------------------------------------------------------------------------
// Base64 helpers
// ---------------------------------------------------------------------------

/// Base64-encodes bytes (no line wrapping).
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Base64-decodes a string back to bytes.
fn base64_decode(encoded: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("base64 decode failed: {e}").into())
}

// ---------------------------------------------------------------------------
// Vault helpers
// ---------------------------------------------------------------------------

/// Fetches all key-value pairs from a Vault KV v2 path.
fn vault_kv_get(
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
        if stderr.contains("no secrets") || stderr.contains("Not Found") || stderr.contains("404")
        {
            return Err(
                format!("secret not found at '{vault_path}'. Verify the path exists in Vault")
                    .into(),
            );
        }
        return Err(format!("vault kv get failed for '{vault_path}': {stderr}").into());
    }

    parse_vault_kv_response(&output.stdout, vault_path)
}

/// Parses the JSON response from `vault kv get -format=json`.
///
/// The JSON structure is `{ "data": { "data": { ... } } }`.
fn parse_vault_kv_response(
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

/// Writes key-value pairs to a Vault KV v2 path.
///
/// Each pair is `key=value` where value is already base64-encoded.
fn vault_kv_put(
    vault_path: &str,
    pairs: &[(&str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    let mut args: Vec<String> = vec![
        "kv".to_string(),
        "put".to_string(),
        vault_path.to_string(),
    ];
    for (key, value) in pairs {
        args.push(format!("{key}={value}"));
    }

    let output = Command::new("vault")
        .args(&args)
        .output()
        .map_err(|e| format!("could not run vault: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("vault kv put failed for '{vault_path}': {}", stderr.trim()).into());
    }

    Ok(())
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test secrets`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/commands/secrets.rs
git commit -m "feat(secrets): add base64 and vault interaction helpers"
```

---

### Task 5: Push command

**Files:**
- Modify: `src/commands/secrets.rs`

- [ ] **Step 1: Write tests for resolving file list from directory**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn resolve_files_in_dir_skips_subdirs_and_dotfiles() {
        let dir = std::env::temp_dir()
            .join(format!("diegops-test-resolve-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Create regular files
        fs::write(dir.join("id_ed25519"), "key").unwrap();
        fs::write(dir.join("id_ed25519.pub"), "pub").unwrap();
        // Create dotfile (should be skipped)
        fs::write(dir.join(".DS_Store"), "junk").unwrap();
        // Create subdir (should be skipped)
        fs::create_dir_all(dir.join("subdir")).unwrap();
        fs::write(dir.join("subdir").join("nested"), "x").unwrap();

        let files = resolve_files_in_dir(&dir).unwrap();
        assert!(files.contains(&"id_ed25519".to_string()));
        assert!(files.contains(&"id_ed25519.pub".to_string()));
        assert!(!files.iter().any(|f| f == ".DS_Store"));
        assert!(!files.iter().any(|f| f == "subdir"));
        assert_eq!(files.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Add resolve_files_in_dir helper**

Add after the vault helpers section:

```rust
// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

/// Lists regular files in a directory (non-recursive, skips dotfiles and subdirectories).
fn resolve_files_in_dir(dir: &std::path::Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with('.') {
                continue;
            }
            // Skip .bak files (our own backups)
            if name.ends_with(".bak") {
                continue;
            }
            files.push(name.to_string());
        }
    }
    files.sort();
    Ok(files)
}
```

- [ ] **Step 3: Implement push**

Replace the stub `push` function:

```rust
/// Pushes local files to Vault (base64-encoded).
pub fn push(
    config_path: Option<&Path>,
    path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    super::common::check_vault_binary()?;
    super::common::check_vault_addr()?;
    super::common::check_vault_auth()?;

    let config = load_config(config_path)?;
    let filter = path_filter.map(super::common::expand_home);

    let mut n_pushed: usize = 0;
    let mut n_unchanged: usize = 0;
    let mut failures: Vec<String> = Vec::new();

    for folder in &config.folders {
        let dest = super::common::expand_home(&folder.dest);

        if let Some(ref f) = filter {
            if !dest.starts_with(f) {
                continue;
            }
        }

        eprintln!("\n{}", folder.dest);

        if !dest.exists() {
            eprintln!("  FAIL  directory '{}' does not exist", dest.display());
            failures.push(folder.dest.clone());
            continue;
        }

        // Resolve file list
        let file_names: Vec<String> = match &folder.keys {
            KeySelector::All(s) if s == "*" => match resolve_files_in_dir(&dest) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("  FAIL  could not list directory: {e}");
                    failures.push(folder.dest.clone());
                    continue;
                }
            },
            KeySelector::All(s) => {
                eprintln!("  FAIL  invalid key selector '{s}' (expected \"*\")");
                failures.push(folder.dest.clone());
                continue;
            }
            KeySelector::List(entries) => entries.iter().map(|e| e.name().to_string()).collect(),
        };

        if file_names.is_empty() {
            eprintln!("  SKIP  no files to push");
            continue;
        }

        // Read and encode files
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut folder_failed = false;

        for name in &file_names {
            let file_path = dest.join(name);
            match fs::read(&file_path) {
                Ok(content) => {
                    pairs.push((name.clone(), base64_encode(&content)));
                }
                Err(e) => {
                    eprintln!("  FAIL  could not read '{}': {e}", file_path.display());
                    folder_failed = true;
                    break;
                }
            }
        }

        if folder_failed {
            failures.push(folder.dest.clone());
            continue;
        }

        // Compare with Vault (if it already exists)
        let needs_push = match vault_kv_get(&folder.vault_path) {
            Ok(existing) => {
                pairs.iter().any(|(key, value)| {
                    existing
                        .get(key)
                        .and_then(|v| v.as_str())
                        .map_or(true, |v| v != value)
                }) || pairs.len() != existing.len()
            }
            Err(_) => true, // Path doesn't exist yet — push everything
        };

        if !needs_push {
            eprintln!("  SKIP  all files unchanged");
            n_unchanged += 1;
            continue;
        }

        // Push to Vault
        let kv_pairs: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        match vault_kv_put(&folder.vault_path, &kv_pairs) {
            Ok(()) => {
                eprintln!("  PUSH  {} file(s) to {}", pairs.len(), folder.vault_path);
                n_pushed += 1;
            }
            Err(e) => {
                eprintln!("  FAIL  {e}");
                failures.push(folder.dest.clone());
            }
        }
    }

    eprintln!();
    println!(
        "Done: {n_pushed} pushed, {n_unchanged} unchanged, {} failed",
        failures.len()
    );

    if !failures.is_empty() {
        eprintln!("\nFailed folders:");
        for f in &failures {
            eprintln!("  {f}");
        }
        return Err(format!("{} folder(s) failed", failures.len()).into());
    }

    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test secrets`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/commands/secrets.rs
git commit -m "feat(secrets): implement push command"
```

---

### Task 6: Pull command

**Files:**
- Modify: `src/commands/secrets.rs`

- [ ] **Step 1: Write test for backup behavior**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn write_file_with_backup_creates_bak() {
        let dir = std::env::temp_dir()
            .join(format!("diegops-test-backup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let file_path = dir.join("test_key");

        // Write original
        fs::write(&file_path, "original content").unwrap();

        // Write new content with backup
        let backed_up = write_file_with_backup(&file_path, b"new content").unwrap();
        assert!(backed_up);
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "new content");
        assert_eq!(
            fs::read_to_string(dir.join("test_key.bak")).unwrap(),
            "original content"
        );

        // Write same content again — no backup needed
        let backed_up = write_file_with_backup(&file_path, b"new content").unwrap();
        assert!(!backed_up);

        // Write to non-existing file — no backup
        let new_path = dir.join("new_key");
        let backed_up = write_file_with_backup(&new_path, b"fresh").unwrap();
        assert!(!backed_up);
        assert_eq!(fs::read_to_string(&new_path).unwrap(), "fresh");

        let _ = fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Implement write_file_with_backup helper**

Add to the file helpers section:

```rust
/// Writes content to a file, backing up the existing file if it differs.
///
/// Returns `true` if a backup was created, `false` if the file was new or unchanged.
fn write_file_with_backup(
    path: &std::path::Path,
    content: &[u8],
) -> Result<bool, Box<dyn std::error::Error>> {
    if path.exists() {
        let existing = fs::read(path)?;
        if existing == content {
            return Ok(false); // unchanged
        }
        // Back up existing file
        let bak = path.with_extension("bak");
        fs::copy(path, &bak)?;
        #[cfg(unix)]
        {
            // Preserve permissions on backup
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(path) {
                let _ = fs::set_permissions(&bak, fs::Permissions::from_mode(meta.permissions().mode()));
            }
        }
        fs::write(path, content)?;
        Ok(true) // backed up
    } else {
        fs::write(path, content)?;
        Ok(false) // new file
    }
}

/// Sets Unix file permissions. No-op on Windows.
#[cfg(unix)]
fn set_permissions(path: &std::path::Path, mode: u32) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_permissions(_path: &std::path::Path, _mode: u32) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
```

- [ ] **Step 3: Implement pull**

Replace the stub `pull` function:

```rust
/// Pulls files from Vault and writes them to local paths.
pub fn pull(
    config_path: Option<&Path>,
    path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    super::common::check_vault_binary()?;
    super::common::check_vault_addr()?;
    super::common::check_vault_auth()?;

    let config = load_config(config_path)?;
    let filter = path_filter.map(super::common::expand_home);

    let mut n_written: usize = 0;
    let mut n_unchanged: usize = 0;
    let mut n_backed_up: usize = 0;
    let mut failures: Vec<String> = Vec::new();

    for folder in &config.folders {
        let dest = super::common::expand_home(&folder.dest);

        if let Some(ref f) = filter {
            if !dest.starts_with(f) {
                continue;
            }
        }

        eprintln!("\n{}", folder.dest);

        // Create dest directory if missing
        if !dest.exists() {
            fs::create_dir_all(&dest)?;
            let mode = dir_mode(folder)?;
            set_permissions(&dest, mode)?;
            eprintln!("  MKDIR {}", dest.display());
        }

        // Fetch from Vault
        let vault_data = match vault_kv_get(&folder.vault_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("  FAIL  {e}");
                failures.push(folder.dest.clone());
                continue;
            }
        };

        // Resolve which keys to pull
        let keys_to_pull: Vec<KeyEntry> = match &folder.keys {
            KeySelector::All(s) if s == "*" => vault_data
                .keys()
                .map(|k| KeyEntry::Simple(k.clone()))
                .collect(),
            KeySelector::All(s) => {
                eprintln!("  FAIL  invalid key selector '{s}' (expected \"*\")");
                failures.push(folder.dest.clone());
                continue;
            }
            KeySelector::List(entries) => entries.clone(),
        };

        let mut folder_failed = false;

        for entry in &keys_to_pull {
            let name = entry.name();
            let file_path = dest.join(name);

            let encoded = match vault_data.get(name).and_then(|v| v.as_str()) {
                Some(v) => v,
                None => {
                    eprintln!("  FAIL  key '{name}' not found in Vault at '{}'", folder.vault_path);
                    folder_failed = true;
                    continue;
                }
            };

            let content = match base64_decode(encoded) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("  FAIL  could not decode '{name}': {e}");
                    folder_failed = true;
                    continue;
                }
            };

            match write_file_with_backup(&file_path, &content) {
                Ok(true) => {
                    eprintln!("  WRITE {name} (backed up to {name}.bak)");
                    n_backed_up += 1;
                    n_written += 1;
                }
                Ok(false) if file_path.exists() && fs::read(&file_path).map_or(false, |c| c == content) => {
                    eprintln!("  SKIP  {name} unchanged");
                    n_unchanged += 1;
                    // Still set permissions in case they drifted
                    let mode = effective_mode(entry)?;
                    set_permissions(&file_path, mode)?;
                    continue;
                }
                Ok(false) => {
                    eprintln!("  WRITE {name}");
                    n_written += 1;
                }
                Err(e) => {
                    eprintln!("  FAIL  could not write '{name}': {e}");
                    folder_failed = true;
                    continue;
                }
            }

            // Set permissions
            let mode = effective_mode(entry)?;
            set_permissions(&file_path, mode)?;
        }

        if folder_failed {
            failures.push(folder.dest.clone());
        }
    }

    eprintln!();
    println!(
        "Done: {n_written} written ({n_backed_up} backed up), {n_unchanged} unchanged, {} failed",
        failures.len()
    );

    if !failures.is_empty() {
        eprintln!("\nFailed folders:");
        for f in &failures {
            eprintln!("  {f}");
        }
        return Err(format!("{} folder(s) failed", failures.len()).into());
    }

    Ok(())
}
```

- [ ] **Step 4: Add Clone derive to KeyEntry**

The `pull` function needs to clone `KeyEntry` from the config list. Update the derive:

```rust
#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum KeyEntry {
    WithMode { name: String, mode: Option<String> },
    Simple(String),
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test secrets`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/commands/secrets.rs
git commit -m "feat(secrets): implement pull command with backup-on-overwrite"
```

---

### Task 7: Status command

**Files:**
- Modify: `src/commands/secrets.rs`

- [ ] **Step 1: Implement status**

Replace the stub `status` function:

```rust
/// Shows sync status between local files and Vault.
pub fn status(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    super::common::check_vault_binary()?;
    super::common::check_vault_addr()?;
    super::common::check_vault_auth()?;

    let config = load_config(config_path)?;

    for folder in &config.folders {
        let dest = super::common::expand_home(&folder.dest);
        println!("\n{}", folder.dest);

        if !dest.exists() {
            println!("  MISSING  directory does not exist");
            continue;
        }

        // Fetch from Vault
        let vault_data = match vault_kv_get(&folder.vault_path) {
            Ok(data) => data,
            Err(e) => {
                println!("  ERROR  could not read Vault: {e}");
                continue;
            }
        };

        // Build list of all relevant filenames
        let local_files: Vec<String> = match &folder.keys {
            KeySelector::All(s) if s == "*" => {
                let mut all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                // Add all Vault keys
                for key in vault_data.keys() {
                    all.insert(key.clone());
                }
                // Add all local files
                if let Ok(files) = resolve_files_in_dir(&dest) {
                    for f in files {
                        all.insert(f);
                    }
                }
                all.into_iter().collect()
            }
            KeySelector::All(s) => {
                println!("  ERROR  invalid key selector '{s}' (expected \"*\")");
                continue;
            }
            KeySelector::List(entries) => entries.iter().map(|e| e.name().to_string()).collect(),
        };

        for name in &local_files {
            let file_path = dest.join(name);
            let in_vault = vault_data.get(name).and_then(|v| v.as_str());
            let local_exists = file_path.exists();

            match (local_exists, in_vault) {
                (true, Some(vault_b64)) => {
                    let local_content = fs::read(&file_path).unwrap_or_default();
                    let local_b64 = base64_encode(&local_content);
                    if local_b64 == vault_b64 {
                        println!("  SYNCED      {name}");
                    } else {
                        println!("  DIFFERS     {name}");
                    }
                }
                (true, None) => {
                    println!("  LOCAL_ONLY  {name}");
                }
                (false, Some(_)) => {
                    println!("  VAULT_ONLY  {name}");
                }
                (false, None) => {
                    println!("  MISSING     {name}");
                }
            }
        }
    }

    println!();
    Ok(())
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/commands/secrets.rs
git commit -m "feat(secrets): implement status command"
```

---

### Task 8: Update CLAUDE.md and README.md

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

- [ ] **Step 1: Add secrets command to the command table in CLAUDE.md**

Add to the commands table:

```markdown
| `diegops secrets init` | Create sample secrets config |
| `diegops secrets push [--path P] [--config F]` | Upload local files to Vault |
| `diegops secrets pull [--path P] [--config F]` | Download files from Vault |
| `diegops secrets status [--config F]` | Compare local vs Vault |
```

- [ ] **Step 2: Add secrets section to CLAUDE.md**

Add a new section after the `diegops vault` section:

```markdown
### `diegops secrets` — workstation file sync via Vault

Config: `~/.diegops/secrets.yaml` (override with `--config` or `$DIEGOPS_SECRETS_CONFIG`)

Syncs sensitive workstation files (SSH keys, kubeconfigs, etc.) through Vault KV v2.
Files are stored as base64-encoded values. Push reads local files and writes to Vault.
Pull reads from Vault and writes locally with correct permissions.

- Push: `diegops secrets push` — base64-encodes local files and writes to Vault
- Pull: `diegops secrets pull` — decodes from Vault and writes with permissions (backs up existing files to `.bak`)
- Status: `diegops secrets status` — shows SYNCED / DIFFERS / LOCAL_ONLY / VAULT_ONLY per file
- Smart permissions: `*.pub` files get `0644`, everything else `0600`, directories `0700`
- Per-file mode overrides supported in config
```

- [ ] **Step 3: Update README.md with secrets usage**

Add a secrets section to the README, following the style of the existing vault section.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass (no code changes, just docs).

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: add secrets command to CLAUDE.md and README.md"
```

---

### Task 9: Final integration tests

**Files:**
- Modify: `tests/integration_test.rs`

- [ ] **Step 1: Add remaining integration tests**

Add to `tests/integration_test.rs`:

```rust
#[test]
fn secrets_push_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "push", "--help"])
        .output()
        .expect("failed to run diegops secrets push --help");

    assert!(output.status.success());
}

#[test]
fn secrets_pull_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "pull", "--help"])
        .output()
        .expect("failed to run diegops secrets pull --help");

    assert!(output.status.success());
}

#[test]
fn secrets_status_help_exits_successfully() {
    let output = diegops()
        .args(["secrets", "status", "--help"])
        .output()
        .expect("failed to run diegops secrets status --help");

    assert!(output.status.success());
}
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Run clippy and fmt**

Run: `cargo fmt --check && cargo clippy -- -D warnings`
Expected: No warnings, no formatting issues.

- [ ] **Step 4: Commit**

```bash
git add tests/integration_test.rs
git commit -m "test(secrets): add integration tests for all subcommands"
```

---

### Task 10: Version bump and tag

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Bump version**

In `Cargo.toml`, bump version from current to next patch (e.g. `1.0.13` → `1.0.14`).

- [ ] **Step 2: Commit, push, and tag**

```bash
git add Cargo.toml
git commit -m "chore: bump version to 1.0.14"
git push
git tag v1.0.14
git push origin v1.0.14
```
