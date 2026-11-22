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
    #[command(
        long_about = "Validate and store a GitHub personal access token (PAT).\n\n\
        diegops uses it for 'sync' (your private diegops-<user> repo) and to clone\n\
        private repos. With no flags you are asked for it with hidden input.\n\n\
        Create one (fine-grained, Contents: read and write on the repos you need):\n  \
        https://github.com/settings/personal-access-tokens/new\n\
        More: diegops explain pat"
    )]
    Login {
        /// Read the token from stdin (e.g. from a password manager)
        #[arg(long, conflicts_with = "from_gh")]
        with_token: bool,
        /// Reuse the token of the GitHub CLI ('gh auth token')
        #[arg(long)]
        from_gh: bool,
        /// Deprecated: the token ends up in shell history
        #[arg(hide = true, conflicts_with_all = ["with_token", "from_gh"])]
        token: Option<String>,
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
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// Formats Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC), using Howard Hinnant's
/// civil-from-days algorithm (no date crate needed).
fn rfc3339_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
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

/// Where the GitHub token in use comes from.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GhTokenSource {
    Env(&'static str),
    Stored,
    GhCli,
}

/// ADR 0001 D2: `$GITHUB_TOKEN` > `$GH_TOKEN` > stored token > `gh auth token`.
/// `gh` is only asked when nothing else is set (it is a subprocess).
pub fn resolve_gh_token(
    env: &dyn Fn(&str) -> Option<String>,
    stored: Option<String>,
    gh: &dyn Fn() -> Option<String>,
) -> Option<(String, GhTokenSource)> {
    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Some(t) = env(var).filter(|t| !t.trim().is_empty()) {
            return Some((t, GhTokenSource::Env(var)));
        }
    }
    if let Some(t) = stored {
        return Some((t, GhTokenSource::Stored));
    }
    gh().filter(|t| !t.trim().is_empty())
        .map(|t| (t, GhTokenSource::GhCli))
}

fn gh_cli_token() -> Option<String> {
    if std::env::var("DIEGOPS_NO_GH_CLI").is_ok_and(|v| !v.is_empty() && v != "0") {
        return None;
    }
    let out = std::process::Command::new("gh")
        .args(["auth", "token"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The GitHub token in use and its source.
pub fn load_gh_token_with_source(
) -> Result<Option<(String, GhTokenSource)>, Box<dyn std::error::Error>> {
    let stored = load_gh_token_from(&tokens_dir()?)?;
    let r = resolve_gh_token(&|k| std::env::var(k).ok(), stored, &gh_cli_token);
    crate::debug!(
        "github token: {}",
        match &r {
            Some((_, GhTokenSource::Env(v))) => format!("${v}"),
            Some((_, GhTokenSource::Stored)) => "stored (~/.diegops/tokens/gh.json)".into(),
            Some((_, GhTokenSource::GhCli)) => "gh auth token".into(),
            None => "none".into(),
        }
    );
    Ok(r)
}

/// Loads the GitHub token (see [`resolve_gh_token`] for the order).
pub fn load_gh_token() -> Result<Option<String>, Box<dyn std::error::Error>> {
    Ok(load_gh_token_with_source()?.map(|(t, _)| t))
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
        .header("User-Agent", &ua)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Accept", "application/vnd.github.v3+json")
        .call();

    match response {
        Ok(mut resp) => {
            let json: serde_json::Value = resp.body_mut().read_json()?;
            let login = json["login"]
                .as_str()
                .ok_or("GitHub API response missing 'login'")?;
            save_gh_token(token)?;
            println!("Authenticated as {login}");
            Ok(())
        }
        Err(ureq::Error::StatusCode(401 | 403)) => {
            Err("token validation failed — check that your PAT is valid and not expired".into())
        }
        Err(ureq::Error::StatusCode(code)) => {
            Err(format!("GitHub API returned unexpected status {code}").into())
        }
        Err(_) => Err("could not reach GitHub API. Check your connection".into()),
    }
}

/// Where `auth gh login` takes the token from.
#[derive(Debug, PartialEq)]
pub enum TokenSource {
    Argument(String),
    Stdin,
    Prompt,
    GhCli,
}

/// Picks the token source from the flags and whether stdin is a terminal.
pub fn token_source(
    arg: Option<String>,
    with_token: bool,
    from_gh: bool,
    stdin_is_tty: bool,
) -> TokenSource {
    match (arg, with_token, from_gh) {
        (Some(t), _, _) => TokenSource::Argument(t),
        (_, _, true) => TokenSource::GhCli,
        (_, true, _) => TokenSource::Stdin,
        _ if stdin_is_tty => TokenSource::Prompt,
        _ => TokenSource::Stdin,
    }
}

/// `diegops auth gh login`: gets the token without putting it on the command line.
pub fn gh_login_cmd(
    arg: Option<String>,
    with_token: bool,
    from_gh: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{IsTerminal, Read};
    let src = token_source(arg, with_token, from_gh, std::io::stdin().is_terminal());
    let token = match src {
        TokenSource::Argument(t) => {
            eprintln!(
                "warning: passing the token as an argument leaves it in your shell history; \
                 run 'diegops auth gh login' (hidden prompt) or use --with-token"
            );
            t
        }
        TokenSource::Stdin => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            if s.trim().is_empty() {
                return Err("no token received on stdin\n  hint: echo \"$TOKEN\" | diegops auth gh login --with-token".into());
            }
            s
        }
        TokenSource::GhCli => {
            let out = std::process::Command::new("gh")
                .args(["auth", "token"])
                .output()
                .map_err(|_| "GitHub CLI 'gh' not found\n  hint: install it with 'diegops tool install gh', then run 'gh auth login'")?;
            if !out.status.success() {
                return Err("'gh auth token' failed: gh is not logged in\n  hint: run 'gh auth login' first".into());
            }
            String::from_utf8(out.stdout)?
        }
        TokenSource::Prompt => {
            eprintln!("Paste a GitHub personal access token (input is hidden).");
            eprintln!("Create one: https://github.com/settings/personal-access-tokens/new");
            rpassword::prompt_password("Token: ")?
        }
    };
    let token = token.trim();
    if token.is_empty() {
        return Err("empty token\n  hint: see 'diegops explain pat'".into());
    }
    gh_login(token)
}

/// Shows the authenticated GitHub user.
pub fn gh_whoami() -> Result<(), Box<dyn std::error::Error>> {
    let token = load_gh_token()?.ok_or("not authenticated. Run 'diegops auth gh login' first")?;

    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let response = ureq::get("https://api.github.com/user")
        .header("User-Agent", &ua)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Accept", "application/vnd.github.v3+json")
        .call();

    match response {
        Ok(mut resp) => {
            let scopes = resp
                .headers()
                .get("x-oauth-scopes")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(none)")
                .to_string();
            let json: serde_json::Value = resp.body_mut().read_json()?;
            let login = json["login"]
                .as_str()
                .ok_or("GitHub API response missing 'login'")?;
            println!("Logged in as: {login}");
            println!("Token scopes: {scopes}");
            Ok(())
        }
        Err(ureq::Error::StatusCode(401 | 403)) => Err(
            "stored GitHub token is no longer valid. Run 'diegops auth gh login' to update it"
                .into(),
        ),
        Err(e) if !matches!(e, ureq::Error::StatusCode(_)) => {
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
    let stored = load_gh_token_data()?;
    let from_env = ["GITHUB_TOKEN", "GH_TOKEN"]
        .into_iter()
        .find(|v| std::env::var(v).is_ok_and(|t| !t.trim().is_empty()));
    match (from_env, stored) {
        (Some(var), stored) => {
            let note = if stored.is_some() {
                " (overrides the stored token)"
            } else {
                ""
            };
            println!("GitHub (gh)    [ok] from ${var}{note}");
        }
        (None, Some(data)) => {
            println!(
                "GitHub (gh)    [ok] configured    stored {}",
                data.stored_at
            );
        }
        (None, None) => {
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
    fn env_token_beats_stored_and_gh_is_last() {
        let env = |k: &str| (k == "GH_TOKEN").then(|| "gh-env".to_string());
        let none = |_: &str| None;
        let gh = || Some("from-gh".to_string());
        let no_gh = || None;
        assert_eq!(
            resolve_gh_token(&env, Some("stored".into()), &gh),
            Some(("gh-env".into(), GhTokenSource::Env("GH_TOKEN")))
        );
        let both = |k: &str| Some(format!("{k}-v"));
        assert_eq!(
            resolve_gh_token(&both, None, &gh).unwrap().1,
            GhTokenSource::Env("GITHUB_TOKEN")
        );
        assert_eq!(
            resolve_gh_token(&none, Some("stored".into()), &gh),
            Some(("stored".into(), GhTokenSource::Stored))
        );
        assert_eq!(
            resolve_gh_token(&none, None, &gh),
            Some(("from-gh".into(), GhTokenSource::GhCli))
        );
        assert_eq!(resolve_gh_token(&none, None, &no_gh), None);
    }

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
    fn rfc3339_from_unix_known_dates() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_from_unix(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_from_unix(1_795_305_599), "2026-11-21T23:59:59Z");
        assert_eq!(rfc3339_from_unix(4_102_444_799), "2099-12-31T23:59:59Z");
    }

    #[test]
    fn format_rfc3339_produces_valid_timestamp() {
        let ts = format_rfc3339_now();
        assert!(ts.contains('T'), "got: {ts}");
        assert!(ts.ends_with('Z'), "got: {ts}");
    }
}

#[cfg(test)]
mod token_source_tests {
    use super::{token_source, TokenSource::*};

    #[test]
    fn picks_source() {
        assert_eq!(
            token_source(Some("t".into()), false, false, true),
            Argument("t".into())
        );
        assert_eq!(token_source(None, false, true, true), GhCli);
        assert_eq!(token_source(None, true, false, true), Stdin);
        assert_eq!(token_source(None, false, false, true), Prompt);
        assert_eq!(token_source(None, false, false, false), Stdin);
    }
}
