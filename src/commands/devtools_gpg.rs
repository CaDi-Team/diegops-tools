//! GPG key management and git commit signing setup.

use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use super::common::diegops_dir;
use super::devtools::{require_cmd, resolve_identity};

/// GPG identity configuration loaded from `gpg-config.yaml`.
#[derive(Deserialize)]
struct GpgConfig {
    /// Full name for the GPG key.
    name: String,
    /// Email address for the GPG key.
    email: String,
}

/// Config file name inside `~/.diegops/`.
const GPG_CONFIG_FILENAME: &str = "gpg-config.yaml";

// ─── Public commands ────────────────────────────────────────────────

/// Generates GPG identity config at `~/.diegops/gpg-config.yaml`.
///
/// Resolves identity from CLI flags, git config, or GitHub API, then
/// writes the YAML file. Idempotent — skips if the file already exists.
pub fn init(name: Option<&str>, email: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = diegops_dir()?;
    let identity = resolve_identity(name, email)?;
    init_to(&dir, &identity.name, &identity.email)
}

/// Testable inner implementation of `init` that accepts an arbitrary directory.
pub fn init_to(dir: &Path, name: &str, email: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let config_path = dir.join(GPG_CONFIG_FILENAME);

    if config_path.exists() {
        eprintln!(
            "SKIP  gpg-config.yaml already exists: {}",
            config_path.display()
        );
        return Ok(());
    }

    let content = format!("name: \"{name}\"\nemail: \"{email}\"\n");
    std::fs::write(&config_path, &content)?;
    eprintln!("WROTE {}", config_path.display());
    Ok(())
}

/// Generates a GPG key, uploads it to GitHub, and configures git signing.
///
/// Flow:
/// 1. Load identity from `gpg-config.yaml`.
/// 2. Find existing GPG key for the email, or generate a new one.
/// 3. Upload the key to GitHub if not already present.
/// 4. Configure `git` to use the key for commit signing.
pub fn set() -> Result<(), Box<dyn std::error::Error>> {
    require_cmd("gpg", "Install GnuPG: https://gnupg.org/")?;
    require_cmd("gh", "Install GitHub CLI: https://cli.github.com/")?;
    require_cmd("git", "Install git: https://git-scm.com/")?;

    let config = load_gpg_config()?;

    // Find or generate key
    let key_id = match find_gpg_key(&config.email)? {
        Some(id) => {
            eprintln!("FOUND existing GPG key {id} for {}", config.email);
            id
        }
        None => {
            eprintln!(
                "Generating new GPG key for {} <{}>…",
                config.name, config.email
            );
            generate_gpg_key(&config.name, &config.email)?
        }
    };

    // Upload to GitHub if needed
    if is_key_on_github(&key_id)? {
        eprintln!("SKIP  key {key_id} already registered on GitHub");
    } else {
        eprintln!("Uploading key {key_id} to GitHub…");
        upload_gpg_key_to_github(&key_id)?;
        eprintln!("DONE  key uploaded to GitHub");
    }

    // Configure git signing
    let gpg_path = which_gpg()?;

    let git_configs: &[(&str, &str)] = &[
        ("user.signingkey", &key_id),
        ("commit.gpgsign", "true"),
        ("tag.gpgsign", "true"),
        ("gpg.program", &gpg_path),
    ];

    for (key, value) in git_configs {
        let status = Command::new("git")
            .args(["config", "--global", key, value])
            .status()?;
        if !status.success() {
            return Err(format!("git config --global {key} {value} failed").into());
        }
    }

    println!("GPG signing configured: key={key_id} gpg={gpg_path}");
    Ok(())
}

/// Restarts the gpg-agent.
///
/// Kills the running agent, relaunches it, and verifies connectivity.
pub fn restart() -> Result<(), Box<dyn std::error::Error>> {
    require_cmd("gpgconf", "Install GnuPG: https://gnupg.org/")?;

    eprintln!("Killing gpg-agent…");
    let kill = Command::new("gpgconf")
        .args(["--kill", "gpg-agent"])
        .status()?;
    if !kill.success() {
        return Err("gpgconf --kill gpg-agent failed".into());
    }

    eprintln!("Launching gpg-agent…");
    let launch = Command::new("gpgconf")
        .args(["--launch", "gpg-agent"])
        .status()?;
    if !launch.success() {
        return Err("gpgconf --launch gpg-agent failed".into());
    }

    // Verify agent is responsive
    let verify = Command::new("gpg-connect-agent").arg("/bye").output()?;
    if !verify.status.success() {
        return Err("gpg-agent restarted but gpg-connect-agent /bye failed".into());
    }

    println!("gpg-agent restarted successfully");
    Ok(())
}

// ─── Helpers ────────────────────────────────────────────────────────

/// Loads GPG config from `~/.diegops/gpg-config.yaml`.
fn load_gpg_config() -> Result<GpgConfig, Box<dyn std::error::Error>> {
    let path = diegops_dir()?.join(GPG_CONFIG_FILENAME);
    let content = super::common::read_config_file(&path)?;
    let config: GpgConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Finds an existing GPG secret key for the given email.
///
/// Parses `gpg --list-secret-keys --keyid-format=long <email>` output
/// and looks for lines starting with `sec`, extracting the key ID after `/`.
fn find_gpg_key(email: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let output = Command::new("gpg")
        .args(["--list-secret-keys", "--keyid-format=long", email])
        .output()?;

    if !output.status.success() {
        // gpg returns non-zero when no keys match — that is not an error here.
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("sec") {
            // Format: "sec   rsa4096/KEYID 2024-01-01 [SC]"
            if let Some(slash_pos) = trimmed.find('/') {
                let after_slash = &trimmed[slash_pos + 1..];
                if let Some(key_id) = after_slash.split_whitespace().next() {
                    return Ok(Some(key_id.to_string()));
                }
            }
        }
    }

    Ok(None)
}

/// Generates a new 4096-bit RSA GPG key with no passphrase and no expiry.
///
/// Uses `gpg --batch --gen-key` with batch content piped via stdin.
fn generate_gpg_key(name: &str, email: &str) -> Result<String, Box<dyn std::error::Error>> {
    let batch = format!(
        "%no-protection\n\
         Key-Type: RSA\n\
         Key-Length: 4096\n\
         Subkey-Type: RSA\n\
         Subkey-Length: 4096\n\
         Name-Real: {name}\n\
         Name-Email: {email}\n\
         Expire-Date: 0\n\
         %commit\n"
    );

    let mut child = Command::new("gpg")
        .args(["--batch", "--gen-key"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(batch.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gpg --batch --gen-key failed: {stderr}").into());
    }

    // After generation, look up the key we just created
    find_gpg_key(email)?
        .ok_or_else(|| "key generation succeeded but key not found afterwards".into())
}

/// Checks whether a GPG key is already registered on the authenticated
/// GitHub account.
fn is_key_on_github(key_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("gh").args(["api", "user/gpg_keys"]).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh api user/gpg_keys failed: {stderr}").into());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let keys: serde_json::Value = serde_json::from_str(&body)?;

    if let Some(arr) = keys.as_array() {
        for key in arr {
            if let Some(kid) = key.get("key_id").and_then(|v| v.as_str()) {
                if kid == key_id {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

/// Exports the public key and uploads it to GitHub via `gh api`.
fn upload_gpg_key_to_github(key_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Export the public key in ASCII-armored form
    let export = Command::new("gpg")
        .args(["--armor", "--export", key_id])
        .output()?;

    if !export.status.success() {
        let stderr = String::from_utf8_lossy(&export.stderr);
        return Err(format!("gpg --armor --export {key_id} failed: {stderr}").into());
    }

    let pubkey = String::from_utf8_lossy(&export.stdout).to_string();
    if pubkey.is_empty() {
        return Err(format!("gpg --armor --export {key_id} produced empty output").into());
    }

    // Build JSON body
    let body = serde_json::json!({ "armored_public_key": pubkey });
    let body_str = serde_json::to_string(&body)?;

    let mut upload = Command::new("gh")
        .args(["api", "user/gpg_keys", "--method", "POST", "--input", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(stdin) = upload.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(body_str.as_bytes())?;
    }
    // Drop stdin so the child process sees EOF
    upload.stdin.take();

    let result = upload.wait_with_output()?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("gh api user/gpg_keys POST failed: {stderr}").into());
    }

    Ok(())
}

/// Returns the absolute path to the `gpg` binary.
fn which_gpg() -> Result<String, Box<dyn std::error::Error>> {
    let (cmd, arg) = if cfg!(target_os = "windows") {
        ("where", "gpg")
    } else {
        ("which", "gpg")
    };

    let output = Command::new(cmd).arg(arg).output()?;
    if !output.status.success() {
        return Err("could not locate gpg on PATH".into());
    }

    // `where` on Windows may return multiple lines; take the first.
    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();

    if path.is_empty() {
        return Err("gpg path resolved to empty string".into());
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn init_creates_gpg_config() {
        let dir =
            std::env::temp_dir().join(format!("diegops-test-gpg-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        init_to(&dir, "Test User", "test@example.com").unwrap();

        let config = dir.join("gpg-config.yaml");
        assert!(config.exists());

        let content = fs::read_to_string(&config).unwrap();
        assert!(content.contains("Test User"));
        assert!(content.contains("test@example.com"));

        // Idempotent
        init_to(&dir, "Test User", "test@example.com").unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}
