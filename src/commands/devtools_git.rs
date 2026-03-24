//! Git identity setup.

use std::process::Command;

/// Sets git global user.name and user.email.
///
/// Resolution: --name/--email flags > gh API.
/// Idempotent: skips if already set to the same value.
pub fn set(name: Option<&str>, email: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    super::devtools::require_cmd("git", "Install it from https://git-scm.com/")?;

    let identity = super::devtools::resolve_identity(name, email)?;

    let current_name = git_config_get("user.name");
    let current_email = git_config_get("user.email");

    if current_name.as_deref() == Some(&identity.name) {
        eprintln!("  user.name already set to '{}'", identity.name);
    } else {
        git_config_set("user.name", &identity.name)?;
        println!("  user.name = {}", identity.name);
    }

    if current_email.as_deref() == Some(&identity.email) {
        eprintln!("  user.email already set to '{}'", identity.email);
    } else {
        git_config_set("user.email", &identity.email)?;
        println!("  user.email = {}", identity.email);
    }

    println!("Done.");
    Ok(())
}

/// Reads a git global config value.
fn git_config_get(key: &str) -> Option<String> {
    Command::new("git")
        .args(["config", "--global", key])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Sets a git global config value.
fn git_config_set(key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["config", "--global", key, value])
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("failed to set git config {key}: {stderr}").into());
    }
    Ok(())
}
