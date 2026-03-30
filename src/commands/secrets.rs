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
use std::process::Command;

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
/// 2. Default `0o700`
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
// Base64 helpers
// ---------------------------------------------------------------------------

/// Encodes bytes to a base64 string using the standard alphabet.
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Decodes a base64 string to bytes using the standard alphabet.
fn base64_decode(encoded: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("base64 decode error: {e}").into())
}

// ---------------------------------------------------------------------------
// Vault interaction helpers
// ---------------------------------------------------------------------------

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
        .ok_or_else(|| {
            format!("unexpected JSON structure from vault kv get for '{vault_path}'")
        })?;

    Ok(data.clone())
}

/// Writes key-value pairs to a Vault KV v2 path.
///
/// Each pair is passed as `key=value` on the command line.
fn vault_kv_put(
    vault_path: &str,
    pairs: &[(&str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new("vault");
    cmd.args(["kv", "put", vault_path]);
    for (key, value) in pairs {
        cmd.arg(format!("{key}={value}"));
    }
    let output = cmd
        .output()
        .map_err(|e| format!("could not run vault: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        return Err(format!("vault kv put failed for '{vault_path}': {stderr}").into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

/// Lists regular (non-dot, non-.bak) files in `dir`, non-recursive, sorted.
///
/// Returns file names (not full paths) as strings. Directories, dotfiles,
/// and `.bak` files are skipped.
fn resolve_files_in_dir(dir: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(dir)
        .map_err(|e| format!("could not read directory '{}': {e}", dir.display()))?
    {
        let entry = entry?;
        let ft = entry.file_type()?;
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        if name_str.ends_with(".bak") {
            continue;
        }
        names.push(name_str.into_owned());
    }
    names.sort();
    Ok(names)
}

/// Writes `content` to `path`, creating a `.bak` backup if the file already
/// exists and its content differs.
///
/// Returns `true` if a backup was created (i.e. the file existed and differed),
/// `false` otherwise (new file, or content identical).
fn write_file_with_backup(
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

/// Sets Unix file permissions on `path`.
///
/// On non-Unix platforms this is a no-op.
fn set_permissions(path: &Path, mode: u32) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode); // no-op on non-Unix
    }
    Ok(())
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

/// Pushes local secret files to Vault.
///
/// For each configured folder, reads local files, base64-encodes their content,
/// and writes to Vault only when the value has changed.
pub fn push(
    config_path: Option<&Path>,
    path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Pre-flight checks — fatal, exit immediately on failure
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
            eprintln!(
                "  FAIL  destination directory '{}' does not exist",
                dest.display()
            );
            failures.push(format!("{} (directory missing)", folder.dest));
            continue;
        }

        // Determine file list to push
        let file_names: Vec<String> = match &folder.keys {
            KeySelector::All(s) if s == "*" => match resolve_files_in_dir(&dest) {
                Ok(names) => names,
                Err(e) => {
                    eprintln!("  FAIL  could not list files: {e}");
                    failures.push(folder.dest.clone());
                    continue;
                }
            },
            KeySelector::All(_) => {
                eprintln!("  FAIL  invalid key selector (expected \"*\")");
                failures.push(folder.dest.clone());
                continue;
            }
            KeySelector::List(entries) => entries.iter().map(|e| e.name().to_string()).collect(),
        };

        if file_names.is_empty() {
            eprintln!("  SKIP  no files to push");
            continue;
        }

        // Fetch current vault state for comparison (treat missing path as empty)
        let vault_state = vault_kv_get(&folder.vault_path).unwrap_or_default();

        // Read and encode each file, collect pairs that differ from vault
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut folder_failed = false;

        for name in &file_names {
            let file_path = dest.join(name);
            let content = match fs::read(&file_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("  FAIL  could not read '{}': {e}", file_path.display());
                    folder_failed = true;
                    break;
                }
            };
            let encoded = base64_encode(&content);

            // Compare with current vault value
            let vault_value = vault_state
                .get(name)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if vault_value == encoded {
                eprintln!("  SKIP  {name} (unchanged)");
                n_unchanged += 1;
            } else {
                pairs.push((name.clone(), encoded));
            }
        }

        if folder_failed {
            failures.push(folder.dest.clone());
            continue;
        }

        if pairs.is_empty() {
            continue;
        }

        // Push all changed pairs in one vault kv put call
        let kv_pairs: Vec<(&str, &str)> =
            pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        match vault_kv_put(&folder.vault_path, &kv_pairs) {
            Ok(()) => {
                for (name, _) in &pairs {
                    eprintln!("  PUSH  {name}");
                    n_pushed += 1;
                }
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

/// Pulls secret files from Vault into local destination directories.
///
/// For each configured folder, fetches Vault KV data, base64-decodes each
/// value, and writes the result to the destination file. Existing files that
/// differ are backed up with a `.bak` extension before being overwritten.
pub fn pull(
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
    let mut n_backed_up: usize = 0;
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

        // Ensure destination directory exists
        if !dest.exists() {
            let mode = match dir_mode(folder) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("  FAIL  invalid dir_mode: {e}");
                    failures.push(folder.dest.clone());
                    continue;
                }
            };
            if let Err(e) = fs::create_dir_all(&dest) {
                eprintln!("  FAIL  could not create directory '{}': {e}", dest.display());
                failures.push(folder.dest.clone());
                continue;
            }
            if let Err(e) = set_permissions(&dest, mode) {
                eprintln!("  WARN  could not set permissions on '{}': {e}", dest.display());
            }
        }

        // Fetch vault data
        let vault_data = match vault_kv_get(&folder.vault_path) {
            Ok(map) => map,
            Err(e) => {
                eprintln!("  FAIL  {e}");
                failures.push(folder.dest.clone());
                continue;
            }
        };

        // Resolve which keys to pull
        let keys_to_pull: Vec<(String, u32)> = match &folder.keys {
            KeySelector::All(s) if s == "*" => vault_data
                .keys()
                .map(|k| (k.clone(), default_mode(k)))
                .collect(),
            KeySelector::All(_) => {
                eprintln!("  FAIL  invalid key selector (expected \"*\")");
                failures.push(folder.dest.clone());
                continue;
            }
            KeySelector::List(entries) => {
                let mut result = Vec::new();
                let mut entry_failed = false;
                for entry in entries {
                    let mode = match effective_mode(entry) {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("  FAIL  invalid mode for '{}': {e}", entry.name());
                            entry_failed = true;
                            break;
                        }
                    };
                    result.push((entry.name().to_string(), mode));
                }
                if entry_failed {
                    failures.push(folder.dest.clone());
                    continue;
                }
                result
            }
        };

        let mut folder_failed = false;

        for (key, mode) in &keys_to_pull {
            let vault_value = match vault_data.get(key) {
                Some(v) => v.as_str().unwrap_or("").to_string(),
                None => {
                    eprintln!("  FAIL  key '{key}' not found at '{}'", folder.vault_path);
                    folder_failed = true;
                    break;
                }
            };

            let content = match base64_decode(&vault_value) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("  FAIL  could not decode '{key}': {e}");
                    folder_failed = true;
                    break;
                }
            };

            let file_path = dest.join(key);
            let file_existed = file_path.exists();
            match write_file_with_backup(&file_path, &content) {
                Ok(backed_up) => {
                    if backed_up {
                        eprintln!("  WRITE {key} (backup created)");
                        n_backed_up += 1;
                        n_written += 1;
                    } else if file_existed {
                        // File existed and content was identical — nothing written
                        eprintln!("  SKIP  {key} (unchanged)");
                        n_unchanged += 1;
                    } else {
                        // New file
                        eprintln!("  WRITE {key}");
                        n_written += 1;
                    }
                    if let Err(e) = set_permissions(&file_path, *mode) {
                        eprintln!("  WARN  could not set permissions on '{key}': {e}");
                    }
                }
                Err(e) => {
                    eprintln!("  FAIL  could not write '{key}': {e}");
                    folder_failed = true;
                    break;
                }
            }
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

/// Shows the sync status of secret files for each configured folder.
///
/// Compares local files with Vault keys and reports SYNCED, DIFFERS,
/// LOCAL_ONLY, or VAULT_ONLY for each file. Uses a BTreeSet for a stable,
/// sorted union of local filenames and Vault keys.
pub fn status(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeSet;

    // Pre-flight checks — fatal, exit immediately on failure
    super::common::check_vault_binary()?;
    super::common::check_vault_addr()?;
    super::common::check_vault_auth()?;

    let config = load_config(config_path)?;

    for folder in &config.folders {
        let dest = super::common::expand_home(&folder.dest);
        println!("\n{}", folder.dest);

        if !dest.exists() {
            println!("  MISSING  (directory does not exist)");
            continue;
        }

        // Fetch vault state
        let vault_data = match vault_kv_get(&folder.vault_path) {
            Ok(map) => map,
            Err(e) => {
                eprintln!("  FAIL  {e}");
                continue;
            }
        };

        // Build sorted union of names to compare
        let mut all_names: BTreeSet<String> = BTreeSet::new();

        match &folder.keys {
            KeySelector::All(s) if s == "*" => {
                // Wildcard: union of local files + vault keys
                match resolve_files_in_dir(&dest) {
                    Ok(local_names) => {
                        for n in local_names {
                            all_names.insert(n);
                        }
                    }
                    Err(e) => {
                        eprintln!("  FAIL  could not list local files: {e}");
                        continue;
                    }
                }
                for k in vault_data.keys() {
                    all_names.insert(k.clone());
                }
            }
            KeySelector::All(_) => {
                eprintln!("  FAIL  invalid key selector (expected \"*\")");
                continue;
            }
            KeySelector::List(entries) => {
                for entry in entries {
                    all_names.insert(entry.name().to_string());
                }
            }
        }

        for name in &all_names {
            let local_path = dest.join(name);
            let local_exists = local_path.exists();
            let vault_value = vault_data.get(name).and_then(|v| v.as_str());

            match (local_exists, vault_value) {
                (false, None) => {
                    // Should not happen given union logic, but be safe
                    println!("  ?           {name}");
                }
                (true, None) => {
                    println!("  LOCAL_ONLY  {name}");
                }
                (false, Some(_)) => {
                    println!("  VAULT_ONLY  {name}");
                }
                (true, Some(encoded)) => {
                    let local_content = match fs::read(&local_path) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("  FAIL        could not read '{name}': {e}");
                            continue;
                        }
                    };
                    let decoded = match base64_decode(encoded) {
                        Ok(b) => b,
                        Err(_) => {
                            println!("  DIFFERS     {name} (vault value not base64)");
                            continue;
                        }
                    };
                    if local_content == decoded {
                        println!("  SYNCED      {name}");
                    } else {
                        println!("  DIFFERS     {name}");
                    }
                }
            }
        }
    }

    Ok(())
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
    fn default_mode_private_key() {
        assert_eq!(default_mode("id_ed25519"), 0o600);
        assert_eq!(default_mode("id_rsa"), 0o600);
        assert_eq!(default_mode("config"), 0o600);
    }

    #[test]
    fn default_mode_public_key() {
        assert_eq!(default_mode("id_ed25519.pub"), 0o644);
        assert_eq!(default_mode("id_rsa.pub"), 0o644);
    }

    #[test]
    fn default_mode_other_files() {
        assert_eq!(default_mode("known_hosts"), 0o600);
        assert_eq!(default_mode("kubeconfig"), 0o600);
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
    fn effective_mode_falls_back_to_default_private() {
        let entry = KeyEntry::Simple("id_ed25519".to_string());
        assert_eq!(effective_mode(&entry).unwrap(), 0o600);
    }

    #[test]
    fn effective_mode_falls_back_to_default_public() {
        let entry = KeyEntry::Simple("id_ed25519.pub".to_string());
        assert_eq!(effective_mode(&entry).unwrap(), 0o644);
    }

    #[test]
    fn effective_mode_override_wins_over_default() {
        let entry = KeyEntry::WithMode {
            name: "id_ed25519.pub".to_string(),
            mode: Some("0600".to_string()),
        };
        // Override should win over smart default (which would be 0644)
        assert_eq!(effective_mode(&entry).unwrap(), 0o600);
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
    fn dir_mode_defaults_to_0700() {
        let folder = Folder {
            dest: "$HOME/secrets".to_string(),
            vault_path: "secret/app".to_string(),
            keys: KeySelector::List(vec![]),
            dir_mode: None,
        };
        assert_eq!(dir_mode(&folder).unwrap(), 0o700);
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

    // -----------------------------------------------------------------------
    // base64 round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn base64_round_trip_text() {
        let original = b"hello, world!\nthis is a test";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn base64_round_trip_binary() {
        let original: Vec<u8> = (0u8..=255u8).collect();
        let encoded = base64_encode(&original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn base64_decode_invalid_returns_error() {
        assert!(base64_decode("not valid base64!!!").is_err());
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
        assert_eq!(data.get("database.env").unwrap().as_str().unwrap(), "aGVsbG8=");
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
    // resolve_files_in_dir
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_files_in_dir_returns_sorted_regular_files_only() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-secrets-resolve-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Regular files that should be returned
        fs::write(dir.join("api.env"), b"x").unwrap();
        fs::write(dir.join("database.env"), b"x").unwrap();
        fs::write(dir.join("readme.txt"), b"x").unwrap();

        // Dotfiles — should be skipped
        fs::write(dir.join(".gitignore"), b"x").unwrap();
        fs::write(dir.join(".hidden"), b"x").unwrap();

        // .bak files — should be skipped
        fs::write(dir.join("database.env.bak"), b"x").unwrap();

        // Subdirectory — should be skipped
        fs::create_dir_all(dir.join("subdir")).unwrap();

        let names = resolve_files_in_dir(&dir).unwrap();
        assert_eq!(names, vec!["api.env", "database.env", "readme.txt"]);

        let _ = fs::remove_dir_all(&dir);
    }

    // -----------------------------------------------------------------------
    // write_file_with_backup
    // -----------------------------------------------------------------------

    #[test]
    fn write_file_with_backup_new_file_no_backup() {
        let dir = std::env::temp_dir().join(format!(
            "diegops-test-secrets-backup-new-{}",
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
            "diegops-test-secrets-backup-same-{}",
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
            "diegops-test-secrets-backup-diff-{}",
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
        assert!(bak_path.exists(), "backup file should exist at {}", bak_path.display());
        assert_eq!(fs::read(&bak_path).unwrap(), b"old content");

        let _ = fs::remove_dir_all(&dir);
    }
}
