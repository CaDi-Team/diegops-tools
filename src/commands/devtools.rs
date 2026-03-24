//! Developer tools — GPG, SSH, and git identity setup.
//!
//! This module contains the clap command enums and shared identity resolution
//! helpers used by the `devtools git`, `devtools gpg`, and `devtools ssh`
//! subcommand families.

use clap::Subcommand;
use std::process::Command;

/// Top-level devtools subcommands.
#[derive(Subcommand)]
pub enum DevtoolsCommand {
    /// Git identity configuration
    Git {
        #[command(subcommand)]
        cmd: GitCommand,
    },
    /// GPG key management and commit signing
    Gpg {
        #[command(subcommand)]
        cmd: GpgCommand,
    },
    /// SSH key management
    Ssh {
        #[command(subcommand)]
        cmd: SshCommand,
    },
}

/// Git identity subcommands.
#[derive(Subcommand)]
pub enum GitCommand {
    /// Set git global user.name and user.email
    Set {
        /// Full name for git commits
        #[arg(long)]
        name: Option<String>,
        /// Email for git commits
        #[arg(long)]
        email: Option<String>,
    },
}

/// GPG subcommands.
#[derive(Subcommand)]
pub enum GpgCommand {
    /// Generate GPG identity config
    Init {
        /// Full name for the GPG key
        #[arg(long)]
        name: Option<String>,
        /// Email for the GPG key
        #[arg(long)]
        email: Option<String>,
    },
    /// Generate GPG key, upload to GitHub, configure git signing
    Set,
    /// Restart the gpg-agent
    Restart,
}

/// SSH subcommands.
#[derive(Subcommand)]
pub enum SshCommand {
    /// List local SSH keys and GitHub registered keys
    List,
    /// Print ~/.ssh/config contents
    Config,
    /// Create a new SSH key
    Create {
        /// Key name / comment
        #[arg(long)]
        name: Option<String>,
        /// Key type (e.g. ed25519, rsa)
        #[arg(long, default_value = "ed25519")]
        r#type: String,
        /// Email for the key
        #[arg(long)]
        email: Option<String>,
    },
}

/// Resolved identity (name + email).
#[allow(dead_code)]
pub struct Identity {
    /// Full name.
    pub name: String,
    /// Email address.
    pub email: String,
}

/// Resolves an identity from: CLI flags > git global config > GitHub API.
///
/// Returns an error if neither source provides the required values.
#[allow(dead_code)]
pub fn resolve_identity(
    name_flag: Option<&str>,
    email_flag: Option<&str>,
) -> Result<Identity, Box<dyn std::error::Error>> {
    let name = match name_flag {
        Some(n) => n.to_string(),
        None => match git_config_get("user.name")? {
            Some(n) => n,
            None => gh_api_login()?,
        },
    };

    let email = match email_flag {
        Some(e) => e.to_string(),
        None => match git_config_get("user.email")? {
            Some(e) => e,
            None => gh_api_primary_email()?,
        },
    };

    Ok(Identity { name, email })
}

/// Reads a git global config value, returning `None` if the key is unset.
#[allow(dead_code)]
pub fn git_config_get(key: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["config", "--global", key])
        .output()?;

    if output.status.success() {
        let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if val.is_empty() {
            Ok(None)
        } else {
            Ok(Some(val))
        }
    } else {
        Ok(None)
    }
}

/// Fetches the GitHub username via `gh api /user`.
#[allow(dead_code)]
pub fn gh_api_login() -> Result<String, Box<dyn std::error::Error>> {
    require_cmd("gh", "Install GitHub CLI: https://cli.github.com/")?;
    let output = Command::new("gh")
        .args(["api", "/user", "--jq", ".login"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh api /user failed: {stderr}").into());
    }

    let login = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if login.is_empty() {
        return Err("gh api /user returned empty login".into());
    }
    Ok(login)
}

/// Fetches the primary email from `gh api /user/emails`.
#[allow(dead_code)]
pub fn gh_api_primary_email() -> Result<String, Box<dyn std::error::Error>> {
    require_cmd("gh", "Install GitHub CLI: https://cli.github.com/")?;
    let output = Command::new("gh")
        .args([
            "api",
            "/user/emails",
            "--jq",
            ".[] | select(.primary) | .email",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh api /user/emails failed: {stderr}").into());
    }

    let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if email.is_empty() {
        return Err("gh api /user/emails returned no primary email".into());
    }
    Ok(email)
}

/// Checks that a command exists on `$PATH`, returning a helpful error if not.
#[allow(dead_code)]
pub fn require_cmd(name: &str, install_hint: &str) -> Result<(), Box<dyn std::error::Error>> {
    let check = if cfg!(target_os = "windows") {
        Command::new("where").arg(name).output()
    } else {
        Command::new("which").arg(name).output()
    };

    match check {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(format!("{name} not found on PATH. {install_hint}").into()),
    }
}
