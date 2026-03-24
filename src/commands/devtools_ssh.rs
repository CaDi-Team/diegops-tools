//! SSH key management.

use std::fs;
use std::io::{BufRead, IsTerminal};
use std::process::Command;

/// Lists local SSH keys and GitHub registered keys.
pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    let ssh_dir = super::common::home_dir()?.join(".ssh");

    // Local keys
    println!("Local keys (~/.ssh/):");
    if ssh_dir.exists() {
        let mut found = false;
        let header = "PUBLIC KEY";
        println!(
            "  {:<20} {:<10} {:<45} {}",
            "NAME", "TYPE", "FINGERPRINT", header
        );
        let mut entries: Vec<_> = fs::read_dir(&ssh_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("pub"))
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            if let Some(info) = ssh_key_info(&path) {
                let pub_key = fs::read_to_string(&path)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                println!(
                    "  {:<20} {:<10} {:<45} {}",
                    name, info.key_type, info.fingerprint, pub_key
                );
                found = true;
            }
        }
        if !found {
            println!("  (no keys found)");
        }
    } else {
        println!("  (~/.ssh/ directory not found)");
    }

    // GitHub keys
    println!();
    println!("GitHub registered keys:");
    match gh_ssh_keys() {
        Ok(keys) if !keys.is_empty() => {
            let header = "ADDED";
            println!(
                "  {:<20} {:<10} {:<45} {}",
                "TITLE", "TYPE", "FINGERPRINT", header
            );
            for key in keys {
                println!(
                    "  {:<20} {:<10} {:<45} {}",
                    key.title, key.key_type, key.fingerprint, key.added
                );
            }
        }
        Ok(_) => {
            println!("  (no keys registered)");
        }
        Err(_) => {
            println!("  (GitHub keys unavailable — gh CLI not found or not authenticated)");
        }
    }

    Ok(())
}

/// Prints ~/.ssh/config contents.
pub fn config() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = super::common::home_dir()?.join(".ssh").join("config");

    if !config_path.exists() {
        println!("No SSH config found at {}", config_path.display());
        return Ok(());
    }

    let content = fs::read_to_string(&config_path)?;
    println!("{content}");
    Ok(())
}

/// Creates a new SSH key.
pub fn create(
    name: Option<&str>,
    key_type: &str,
    email: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    super::devtools::require_cmd("ssh-keygen", "Install OpenSSH")?;

    // Resolve email for comment
    let comment = match email {
        Some(e) => e.to_owned(),
        None => super::devtools::resolve_identity(None, None)
            .map(|id| id.email)
            .unwrap_or_else(|_| "diegops".to_owned()),
    };

    // Resolve key path
    let ssh_dir = super::common::home_dir()?.join(".ssh");
    fs::create_dir_all(&ssh_dir)?;

    let base_name = match name {
        Some(n) => n.to_owned(),
        None => format!("id_{key_type}"),
    };
    let mut key_path = ssh_dir.join(&base_name);

    if key_path.exists() {
        let postfix = if std::io::stdin().is_terminal() {
            eprintln!(
                "Key '{}' already exists. Enter a postfix (e.g., 'work', 'personal'):",
                key_path.display()
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_owned();
            if input.is_empty() {
                generate_random_postfix()
            } else {
                input
            }
        } else {
            generate_random_postfix()
        };
        key_path = ssh_dir.join(format!("{base_name}_{postfix}"));
    }

    eprintln!("Generating {key_type} key at {}...", key_path.display());

    let output = Command::new("ssh-keygen")
        .args(["-t", key_type, "-C", &comment, "-f"])
        .arg(&key_path)
        .args(["-N", ""])
        .output()
        .map_err(|e| format!("could not run ssh-keygen: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ssh-keygen failed: {stderr}").into());
    }

    let pub_path = key_path.with_extension("pub");
    let public_key = fs::read_to_string(&pub_path)?;

    println!("Created: {}", key_path.display());
    println!();
    println!("Public key (copy this):");
    println!("{}", public_key.trim());
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct SshKeyInfo {
    key_type: String,
    fingerprint: String,
}

/// Extracts key info using ssh-keygen -l.
fn ssh_key_info(pub_path: &std::path::Path) -> Option<SshKeyInfo> {
    let output = Command::new("ssh-keygen")
        .args(["-l", "-f"])
        .arg(pub_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let line = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = line.split_whitespace().collect();
    // Format: 256 SHA256:abc123... user@host (ED25519)
    if parts.len() >= 4 {
        let fingerprint = parts[1].to_owned();
        let key_type = parts
            .last()?
            .trim_matches(|c| c == '(' || c == ')')
            .to_lowercase();
        Some(SshKeyInfo {
            key_type,
            fingerprint,
        })
    } else {
        None
    }
}

struct GhSshKey {
    title: String,
    key_type: String,
    fingerprint: String,
    added: String,
}

/// Fetches SSH keys from GitHub.
fn gh_ssh_keys() -> Result<Vec<GhSshKey>, Box<dyn std::error::Error>> {
    let output = Command::new("gh")
        .args(["ssh-key", "list"])
        .output()
        .map_err(|e| format!("could not run gh: {e}"))?;

    if !output.status.success() {
        return Err("gh ssh-key list failed".into());
    }

    let mut keys = Vec::new();
    for line in output.stdout.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            keys.push(GhSshKey {
                title: parts[0].to_owned(),
                key_type: parts[1].to_owned(),
                fingerprint: parts[2].to_owned(),
                added: parts[3].to_owned(),
            });
        }
    }
    Ok(keys)
}

/// Generates a random 6-character hex postfix using system time.
fn generate_random_postfix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:06x}", nanos & 0xFFFFFF)
}
