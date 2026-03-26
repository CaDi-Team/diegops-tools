//! Auth credential management — store, load, and validate provider tokens.
//!
//! Tokens are stored as JSON files in `~/.diegops/tokens/`, one per provider.
//! Currently supports GitHub (`gh`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Clap sub-command definitions
// ---------------------------------------------------------------------------

/// Auth credential management sub-commands.
#[derive(clap::Subcommand)]
pub enum AuthCommand {
    /// GitHub authentication
    Gh {
        #[command(subcommand)]
        cmd: GhCommand,
    },
    /// Show authentication status for all providers
    Status,
    /// Remove all stored authentication tokens
    Logout,
}

/// GitHub-specific auth sub-commands.
#[derive(clap::Subcommand)]
pub enum GhCommand {
    /// Validate and store a GitHub personal access token
    Login {
        /// GitHub personal access token (PAT)
        token: String,
    },
    /// Remove stored GitHub token
    Logout,
    /// Show authenticated GitHub user and token scopes
    Whoami,
}

// ---------------------------------------------------------------------------
// Token data structures
// ---------------------------------------------------------------------------

/// Stored token metadata.
#[derive(Serialize, Deserialize)]
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

/// Returns the tokens directory: `~/.diegops/tokens/`.
fn tokens_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(super::common::diegops_dir()?.join("tokens"))
}

/// Formats the current time as an RFC 3339 timestamp.
fn format_rfc3339_now() -> String {
    humantime::format_rfc3339(std::time::SystemTime::now()).to_string()
}

/// Saves a GitHub token to the given directory.
fn save_gh_token_to(dir: &Path, token: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dir)?;
    let data = TokenData {
        version: 1,
        token: token.to_owned(),
        stored_at: format_rfc3339_now(),
    };
    let path = dir.join("gh.json");
    fs::write(&path, serde_json::to_string_pretty(&data)?)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Loads a GitHub token from the given directory. Returns `None` if file absent.
fn load_gh_token_from(dir: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let path = dir.join("gh.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    let data: TokenData = serde_json::from_str(&content)?;
    Ok(Some(data.token))
}

/// Loads token metadata from the given directory. Returns `None` if file absent.
fn load_gh_token_data_from(dir: &Path) -> Result<Option<TokenData>, Box<dyn std::error::Error>> {
    let path = dir.join("gh.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

/// Removes the GitHub token file. Idempotent. Returns true if file was removed.
fn remove_gh_token_from(dir: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let path = dir.join("gh.json");
    if path.exists() {
        fs::remove_file(&path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Public API (uses default tokens_dir)
// ---------------------------------------------------------------------------

/// Saves a GitHub token to `~/.diegops/tokens/gh.json`.
pub fn save_gh_token(token: &str) -> Result<(), Box<dyn std::error::Error>> {
    save_gh_token_to(&tokens_dir()?, token)
}

/// Loads the GitHub token. Resolution: file > $GITHUB_TOKEN > None.
pub fn load_gh_token() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let from_file = load_gh_token_from(&tokens_dir()?)?;
    if from_file.is_some() {
        return Ok(from_file);
    }
    if let Ok(env_token) = std::env::var("GITHUB_TOKEN") {
        if !env_token.is_empty() {
            return Ok(Some(env_token));
        }
    }
    Ok(None)
}

/// Loads GitHub token metadata for status display.
pub fn load_gh_token_data() -> Result<Option<TokenData>, Box<dyn std::error::Error>> {
    load_gh_token_data_from(&tokens_dir()?)
}

/// Removes the GitHub token. Returns true if a file was removed.
pub fn remove_gh_token() -> Result<bool, Box<dyn std::error::Error>> {
    remove_gh_token_from(&tokens_dir()?)
}

/// Removes all tokens in `~/.diegops/tokens/`. Returns count removed.
pub fn remove_all_tokens() -> Result<usize, Box<dyn std::error::Error>> {
    let dir = tokens_dir()?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
            fs::remove_file(entry.path())?;
            count += 1;
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Validates and stores a GitHub PAT.
pub fn gh_login(token: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let response = ureq::get("https://api.github.com/user")
        .set("User-Agent", &ua)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match response {
        Ok(resp) => {
            let json: serde_json::Value = resp.into_json()?;
            let login = json["login"]
                .as_str()
                .ok_or("GitHub API response missing 'login'")?;
            save_gh_token(token)?;
            println!("Authenticated as {login}");
            Ok(())
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("token validation failed — check that your PAT is valid and not expired".into())
        }
        Err(ureq::Error::Status(code, _)) => {
            Err(format!("GitHub API returned unexpected status {code}").into())
        }
        Err(ureq::Error::Transport(_)) => {
            Err("could not reach GitHub API. Check your connection".into())
        }
    }
}

/// Shows the authenticated GitHub user.
pub fn gh_whoami() -> Result<(), Box<dyn std::error::Error>> {
    let token =
        load_gh_token()?.ok_or("not authenticated. Run 'diegops auth gh login <PAT>' first")?;

    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let response = ureq::get("https://api.github.com/user")
        .set("User-Agent", &ua)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match response {
        Ok(resp) => {
            let scopes = resp
                .header("x-oauth-scopes")
                .unwrap_or("(none)")
                .to_string();
            let json: serde_json::Value = resp.into_json()?;
            let login = json["login"]
                .as_str()
                .ok_or("GitHub API response missing 'login'")?;
            println!("Logged in as: {login}");
            println!("Token scopes: {scopes}");
            Ok(())
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            Err("stored GitHub token is no longer valid. Run 'diegops auth gh login <PAT>' to update it".into())
        }
        Err(ureq::Error::Transport(_)) => {
            Err("could not reach GitHub API. Check your connection".into())
        }
        Err(e) => Err(format!("GitHub API error: {e}").into()),
    }
}

/// Removes the stored GitHub token.
pub fn gh_logout() -> Result<(), Box<dyn std::error::Error>> {
    if remove_gh_token()? {
        println!("GitHub token removed.");
    } else {
        println!("GitHub token was not configured.");
    }
    Ok(())
}

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

/// Shows auth status for all providers.
pub fn status() -> Result<(), Box<dyn std::error::Error>> {
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

/// Removes all stored tokens.
pub fn logout_all() -> Result<(), Box<dyn std::error::Error>> {
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
    fn save_and_load_gh_token() {
        let dir = std::env::temp_dir().join(format!("diegops-test-auth-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        save_gh_token_to(&dir, "ghp_test123").unwrap();

        let token_file = dir.join("gh.json");
        assert!(token_file.exists());

        let data: TokenData =
            serde_json::from_str(&fs::read_to_string(&token_file).unwrap()).unwrap();
        assert_eq!(data.token, "ghp_test123");
        assert_eq!(data.version, 1);
        assert!(!data.stored_at.is_empty());

        let loaded = load_gh_token_from(&dir).unwrap();
        assert_eq!(loaded.unwrap(), "ghp_test123");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_gh_token_returns_none_when_missing() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-auth-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let loaded = load_gh_token_from(&dir).unwrap();
        assert!(loaded.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_gh_token_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("diegops-test-auth-rm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        save_gh_token_to(&dir, "ghp_test123").unwrap();
        assert!(dir.join("gh.json").exists());

        let removed = remove_gh_token_from(&dir).unwrap();
        assert!(removed);
        assert!(!dir.join("gh.json").exists());

        // Idempotent: second call succeeds
        let removed = remove_gh_token_from(&dir).unwrap();
        assert!(!removed);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_rfc3339_produces_valid_timestamp() {
        let ts = format_rfc3339_now();
        assert!(ts.contains('T'), "got: {ts}");
        assert!(ts.ends_with('Z'), "got: {ts}");
    }
}
