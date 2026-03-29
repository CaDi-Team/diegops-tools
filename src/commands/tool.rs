//! Managed CLI toolbox: download, update, remove, and proxy popular DevOps CLI tools.
//!
//! All managed binaries live in `~/.diegops/bin/`. The tool registry is a static
//! array compiled into the binary — no config file needed.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Clap sub-command definitions
// ---------------------------------------------------------------------------

/// Tool management sub-commands.
#[derive(clap::Subcommand)]
pub enum ToolCommand {
    /// List all available tools and their install status
    #[command(long_about = "List all available tools and their install status.\n\n\
        Shows: tool name, description, installed version, and status.\n\
        Available tools: gh, vault, terraform, helm, k9s, kubectl, jq, yq, trivy, trippy")]
    List,
    /// Install a tool (downloads latest version)
    #[command(long_about = "Install a tool by downloading the latest version.\n\n\
        Binaries are stored in ~/.diegops/bin/\n\
        Idempotent — skips if already at the latest version.\n\n\
        Available tools: gh, vault, terraform, helm, k9s, kubectl, jq, yq, trivy, trippy\n\n\
        Examples:\n  \
        diegops tool install gh         # install GitHub CLI\n  \
        diegops tool install jq         # install jq\n  \
        diegops tool install kubectl    # install kubectl\n\n\
        After installing, use directly: diegops gh pr list")]
    Install {
        /// Tool name (e.g., gh, vault, terraform, jq, kubectl)
        name: String,
    },
    /// Update installed tools (all or specific)
    #[command(long_about = "Update installed tools to their latest versions.\n\n\
        With no arguments, updates ALL installed tools.\n\
        With a tool name, updates only that tool.\n\n\
        Examples:\n  \
        diegops tool update        # update everything\n  \
        diegops tool update gh     # update only gh")]
    Update {
        /// Tool name (omit to update all installed tools)
        name: Option<String>,
    },
    /// Remove an installed tool
    #[command(long_about = "Remove an installed tool from ~/.diegops/bin/\n\n\
        Example: diegops tool remove gh")]
    Remove {
        /// Tool name (e.g., gh, vault, terraform)
        name: String,
    },
}

// ---------------------------------------------------------------------------
// Platform helpers
// ---------------------------------------------------------------------------

/// Returns the OS name used by most tool asset filenames.
fn platform_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

/// Returns the architecture name used by most tool asset filenames.
fn platform_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    }
}

/// Returns the full Rust target triple (used by trippy).
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const RUST_TARGET: &str = "aarch64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const RUST_TARGET: &str = "x86_64-apple-darwin";
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "musl"))]
const RUST_TARGET: &str = "x86_64-unknown-linux-musl";
#[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
const RUST_TARGET: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "musl"))]
const RUST_TARGET: &str = "aarch64-unknown-linux-musl";
#[cfg(all(target_os = "linux", target_arch = "aarch64", not(target_env = "musl")))]
const RUST_TARGET: &str = "aarch64-unknown-linux-gnu";
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const RUST_TARGET: &str = "x86_64-pc-windows-gnu";

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64"),
)))]
compile_error!("tool: unsupported target platform — add a RUST_TARGET constant for this target");

// ---------------------------------------------------------------------------
// Archive format
// ---------------------------------------------------------------------------

/// How the tool is distributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveFormat {
    /// `.tar.gz` archive
    TarGz,
    /// `.zip` archive
    Zip,
    /// Bare binary (no archive)
    Bare,
}

// ---------------------------------------------------------------------------
// Tool source
// ---------------------------------------------------------------------------

/// Where to fetch the tool from.
#[derive(Debug, Clone)]
enum ToolSource {
    /// GitHub releases API.
    GitHub { repo: &'static str },
    /// Custom URL (e.g. kubectl from dl.k8s.io).
    Url {
        latest_url: &'static str,
        download_template: &'static str,
    },
}

// ---------------------------------------------------------------------------
// Tool definition and registry
// ---------------------------------------------------------------------------

/// A managed tool definition.
#[derive(Debug, Clone)]
struct ToolDef {
    /// Short name (used as subcommand / lookup key).
    name: &'static str,
    /// Human-readable description.
    description: &'static str,
    /// Where to fetch releases from.
    source: ToolSource,
    /// Name of the binary on disk (e.g. "trip" for trippy).
    binary_name: &'static str,
    /// Archive format of the release asset.
    format: ArchiveFormat,
}

/// The static tool registry.
const REGISTRY: &[ToolDef] = &[
    ToolDef {
        name: "gh",
        description: "GitHub CLI",
        source: ToolSource::GitHub { repo: "cli/cli" },
        binary_name: "gh",
        format: ArchiveFormat::TarGz,
    },
    ToolDef {
        name: "vault",
        description: "HashiCorp Vault",
        source: ToolSource::GitHub {
            repo: "hashicorp/vault",
        },
        binary_name: "vault",
        format: ArchiveFormat::Zip,
    },
    ToolDef {
        name: "terraform",
        description: "HashiCorp Terraform",
        source: ToolSource::GitHub {
            repo: "hashicorp/terraform",
        },
        binary_name: "terraform",
        format: ArchiveFormat::Zip,
    },
    ToolDef {
        name: "helm",
        description: "Kubernetes package manager",
        source: ToolSource::GitHub { repo: "helm/helm" },
        binary_name: "helm",
        format: ArchiveFormat::TarGz,
    },
    ToolDef {
        name: "k9s",
        description: "Kubernetes TUI",
        source: ToolSource::GitHub {
            repo: "derailed/k9s",
        },
        binary_name: "k9s",
        format: ArchiveFormat::TarGz,
    },
    ToolDef {
        name: "jq",
        description: "JSON processor",
        source: ToolSource::GitHub { repo: "jqlang/jq" },
        binary_name: "jq",
        format: ArchiveFormat::Bare,
    },
    ToolDef {
        name: "yq",
        description: "YAML processor",
        source: ToolSource::GitHub {
            repo: "mikefarah/yq",
        },
        binary_name: "yq",
        format: ArchiveFormat::TarGz,
    },
    ToolDef {
        name: "trivy",
        description: "Security scanner",
        source: ToolSource::GitHub {
            repo: "aquasecurity/trivy",
        },
        binary_name: "trivy",
        format: ArchiveFormat::TarGz,
    },
    ToolDef {
        name: "trippy",
        description: "Network diagnostic tool",
        source: ToolSource::GitHub {
            repo: "fujiapple852/trippy",
        },
        binary_name: "trip",
        format: ArchiveFormat::TarGz,
    },
    ToolDef {
        name: "kubectl",
        description: "Kubernetes CLI",
        source: ToolSource::Url {
            latest_url: "https://dl.k8s.io/release/stable.txt",
            download_template: "https://dl.k8s.io/release/{version}/bin/{os}/{arch}/kubectl",
        },
        binary_name: "kubectl",
        format: ArchiveFormat::Bare,
    },
];

/// Returns the tool definition for the given name, or `None`.
fn find_tool(name: &str) -> Option<&'static ToolDef> {
    REGISTRY
        .iter()
        .find(|t| t.name == name || t.binary_name == name)
}

/// Returns `true` if `name` matches a known tool name or binary name.
pub fn is_known_tool(name: &str) -> bool {
    find_tool(name).is_some()
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Returns `~/.diegops/bin/`.
fn bin_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(super::common::diegops_dir()?.join("bin"))
}

/// Returns the full path to a managed tool binary.
fn tool_bin_path(tool: &ToolDef) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let name = if cfg!(windows) {
        format!("{}.exe", tool.binary_name)
    } else {
        tool.binary_name.to_string()
    };
    Ok(bin_dir()?.join(name))
}

// ---------------------------------------------------------------------------
// Asset name building (per-tool naming quirks)
// ---------------------------------------------------------------------------

/// Builds the release asset filename for the given tool, version, OS, and arch.
///
/// Each tool has unique naming conventions, so this is a match on tool name
/// rather than a generic template.
fn asset_name(tool: &ToolDef, version: &str) -> String {
    // Strip the "v" prefix if present — many tools use bare version in filenames.
    let v = version.trim_start_matches('v');
    let os = platform_os();
    let arch = platform_arch();

    match tool.name {
        // gh_2.50.0_darwin_arm64.tar.gz (no v prefix, underscores)
        "gh" => format!("gh_{v}_{os}_{arch}.tar.gz"),

        // vault_1.17.0_darwin_arm64.zip (no v prefix)
        "vault" => format!("vault_{v}_{os}_{arch}.zip"),

        // terraform_1.9.0_darwin_arm64.zip (no v prefix)
        "terraform" => format!("terraform_{v}_{os}_{arch}.zip"),

        // helm-v3.15.0-darwin-arm64.tar.gz (dashes, keeps v prefix)
        "helm" => format!("helm-v{v}-{os}-{arch}.tar.gz"),

        // k9s_darwin_arm64.tar.gz (no version in filename!)
        "k9s" => {
            let k9s_os = if cfg!(target_os = "macos") {
                "Darwin"
            } else if cfg!(target_os = "windows") {
                "Windows"
            } else {
                "Linux"
            };
            format!("k9s_{k9s_os}_{arch}.tar.gz")
        }

        // jq-macos-arm64 or jq-linux-amd64 (bare binary, "macos" not "darwin")
        "jq" => {
            let jq_os = if cfg!(target_os = "macos") {
                "macos"
            } else if cfg!(target_os = "windows") {
                "windows"
            } else {
                "linux"
            };
            format!("jq-{jq_os}-{arch}")
        }

        // yq_darwin_arm64.tar.gz
        "yq" => format!("yq_{os}_{arch}.tar.gz"),

        // trivy_0.52.0_macOS-ARM64.tar.gz / trivy_0.52.0_Linux-64bit.tar.gz
        "trivy" => {
            let trivy_os_arch = if cfg!(target_os = "macos") {
                if cfg!(target_arch = "aarch64") {
                    "macOS-ARM64"
                } else {
                    "macOS-64bit"
                }
            } else if cfg!(target_os = "windows") {
                if cfg!(target_arch = "aarch64") {
                    "Windows-ARM64"
                } else {
                    "Windows-64bit"
                }
            } else if cfg!(target_arch = "aarch64") {
                "Linux-ARM64"
            } else {
                "Linux-64bit"
            };
            format!("trivy_{v}_{trivy_os_arch}.tar.gz")
        }

        // trippy-0.11.0-aarch64-apple-darwin.tar.gz (uses Rust target triple)
        "trippy" => format!("trippy-{v}-{RUST_TARGET}.tar.gz"),

        // kubectl: bare binary from CDN (not from GitHub releases)
        "kubectl" => "kubectl".to_string(),

        _ => format!("{}-{v}-{os}-{arch}", tool.name),
    }
}

/// Returns the expected path of the binary inside the archive, or `None` for
/// bare binaries and root-level archives.
fn binary_path_in_archive(tool: &ToolDef, version: &str) -> Option<String> {
    let v = version.trim_start_matches('v');
    let os = platform_os();
    let arch = platform_arch();

    match tool.name {
        // gh binary is at gh_2.50.0_darwin_arm64/bin/gh
        "gh" => Some(format!("gh_{v}_{os}_{arch}/bin/gh")),
        // helm binary is at darwin-arm64/helm
        "helm" => Some(format!("{os}-{arch}/helm")),
        // All others: binary at root of archive (matched by filename)
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Version detection
// ---------------------------------------------------------------------------

/// Extracts the first semver-like pattern (X.Y.Z) from text.
fn extract_semver(text: &str) -> Option<String> {
    for word in text.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let parts: Vec<&str> = word.split('.').collect();
        if parts.len() >= 3
            && parts[0].chars().all(|c| c.is_ascii_digit())
            && !parts[0].is_empty()
            && parts[1].chars().all(|c| c.is_ascii_digit())
            && !parts[1].is_empty()
            && parts[2].chars().all(|c| c.is_ascii_digit())
            && !parts[2].is_empty()
        {
            // Rejoin the first 3 parts (ignore any extra dots)
            return Some(format!("{}.{}.{}", parts[0], parts[1], parts[2]));
        }
    }
    None
}

/// Detects the installed version of a tool by running it with `--version` or `version`.
fn detect_installed_version(binary_path: &Path) -> Option<String> {
    for flag in &["--version", "version"] {
        if let Ok(output) = Command::new(binary_path).arg(flag).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = extract_semver(&stdout) {
                    return Some(version);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Network helpers
// ---------------------------------------------------------------------------

/// Creates a ureq request builder with standard headers.
fn http_get(url: &str) -> ureq::Request {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    ureq::get(url).set("User-Agent", &ua)
}

/// Fetches the latest release metadata from the GitHub API.
fn fetch_github_latest(
    repo: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = http_get(&url)
        .set("Accept", "application/vnd.github.v3+json")
        .call()?;
    Ok(resp.into_json()?)
}

/// Fetches the latest version string for a tool.
fn fetch_latest_version(
    tool: &ToolDef,
) -> Result<String, Box<dyn std::error::Error>> {
    match &tool.source {
        ToolSource::GitHub { repo } => {
            let release = fetch_github_latest(repo)?;
            let tag = release["tag_name"]
                .as_str()
                .ok_or("GitHub API response missing 'tag_name'")?;
            Ok(tag.to_string())
        }
        ToolSource::Url {
            latest_url,
            download_template: _,
        } => {
            let resp = http_get(latest_url).call()?;
            let body = resp.into_string()?;
            Ok(body.trim().to_string())
        }
    }
}

/// Builds the download URL for a tool at a given version.
fn download_url(tool: &ToolDef, version: &str) -> String {
    match &tool.source {
        ToolSource::GitHub { repo } => {
            let asset = asset_name(tool, version);
            format!("https://github.com/{repo}/releases/download/{version}/{asset}")
        }
        ToolSource::Url {
            latest_url: _,
            download_template,
        } => {
            let os = platform_os();
            let arch = platform_arch();
            download_template
                .replace("{version}", version)
                .replace("{os}", os)
                .replace("{arch}", arch)
        }
    }
}

/// Downloads raw bytes from a URL.
fn download_bytes(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    http_get(url).call()?.into_reader().read_to_end(&mut buf)?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Extraction and installation
// ---------------------------------------------------------------------------

/// Installs a binary blob to the given path with executable permissions.
fn install_binary(path: &Path, binary: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
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

/// Extracts the tool binary from a `.tar.gz` archive.
#[cfg(unix)]
fn extract_from_tar_gz(
    bytes: &[u8],
    tool: &ToolDef,
    version: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let expected_path = binary_path_in_archive(tool, version);
    let binary_name = if cfg!(windows) {
        format!("{}.exe", tool.binary_name)
    } else {
        tool.binary_name.to_string()
    };

    let mut archive = Archive::new(GzDecoder::new(bytes));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let path_str = path.to_string_lossy().to_string();

        // Match by expected full path, or by filename at root
        let is_match = if let Some(ref expected) = expected_path {
            path_str == *expected || path_str == format!("./{expected}")
        } else {
            path.file_name().map(|f| f.to_string_lossy().to_string()) == Some(binary_name.clone())
        };

        if is_match {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }

    Err(format!(
        "'{}' binary not found in archive (looked for {:?} or filename '{}')",
        tool.binary_name, expected_path, binary_name
    )
    .into())
}

/// Extracts the tool binary from a `.zip` archive.
fn extract_from_zip(bytes: &[u8], tool: &ToolDef) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Use the zip crate (available on Windows, but we also need it on Unix for
    // HashiCorp tools which ship as .zip on all platforms).
    // Since zip is only a dependency on Windows via Cargo.toml, we use a manual
    // approach for Unix: shell out to `unzip` or use the `zip` feature.
    //
    // Actually, since vault and terraform ship .zip on all platforms, we need
    // zip support on Unix too. We'll use a minimal approach: try the system
    // `unzip` command via a temp directory.
    let tmp_dir = std::env::temp_dir().join(format!("diegops-tool-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)?;

    let zip_path = tmp_dir.join("archive.zip");
    fs::write(&zip_path, bytes)?;

    let binary_name = if cfg!(windows) {
        format!("{}.exe", tool.binary_name)
    } else {
        tool.binary_name.to_string()
    };

    // Use unzip command (available on all Unix systems and most CI environments)
    let output = Command::new("unzip")
        .args(["-o", "-q"])
        .arg(&zip_path)
        .arg(&binary_name)
        .arg("-d")
        .arg(&tmp_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(format!("failed to extract {binary_name} from zip: {stderr}").into());
    }

    let binary_path = tmp_dir.join(&binary_name);
    let buf = fs::read(&binary_path)?;
    let _ = fs::remove_dir_all(&tmp_dir);
    Ok(buf)
}

/// Downloads, extracts, and installs a tool at the given version.
fn download_and_install(
    tool: &ToolDef,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = download_url(tool, version);
    eprintln!("Downloading {} {}...", tool.name, version);
    let bytes = download_bytes(&url)?;

    let dest = tool_bin_path(tool)?;

    match tool.format {
        ArchiveFormat::TarGz => {
            #[cfg(unix)]
            {
                eprintln!("Extracting...");
                let binary = extract_from_tar_gz(&bytes, tool, version)?;
                install_binary(&dest, &binary)?;
            }
            #[cfg(windows)]
            {
                return Err(
                    format!("{}: .tar.gz extraction not supported on Windows", tool.name).into(),
                );
            }
        }
        ArchiveFormat::Zip => {
            eprintln!("Extracting...");
            let binary = extract_from_zip(&bytes, tool)?;
            install_binary(&dest, &binary)?;
        }
        ArchiveFormat::Bare => {
            install_binary(&dest, &bytes)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Handles `diegops tool list`.
pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    let bin = bin_dir()?;

    let status_header = "STATUS";
    println!(
        "{:<12} {:<30} {:<12} {}",
        "TOOL", "DESCRIPTION", "VERSION", status_header
    );
    println!("{}", "-".repeat(70));

    for tool in REGISTRY {
        let path = bin.join(if cfg!(windows) {
            format!("{}.exe", tool.binary_name)
        } else {
            tool.binary_name.to_string()
        });

        let (version, status) = if path.exists() {
            let ver = detect_installed_version(&path).unwrap_or_else(|| "?".to_string());
            (ver, "installed")
        } else {
            ("-".to_string(), "not installed")
        };

        println!(
            "{:<12} {:<30} {:<12} {}",
            tool.name, tool.description, version, status
        );
    }

    Ok(())
}

/// Handles `diegops tool install <name>`.
pub fn install(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tool = find_tool(name).ok_or_else(|| {
        format!("unknown tool: '{name}'. Run 'diegops tool list' to see available tools")
    })?;

    eprintln!("Fetching latest version of {}...", tool.name);
    let version = fetch_latest_version(tool)?;
    let version_bare = version.trim_start_matches('v');

    // Check if already installed at this version
    let dest = tool_bin_path(tool)?;
    if dest.exists() {
        if let Some(installed) = detect_installed_version(&dest) {
            if installed == version_bare {
                println!("{} is already at version {version_bare}.", tool.name);
                return Ok(());
            }
        }
    }

    download_and_install(tool, &version)?;

    // Verify
    if dest.exists() {
        if let Some(ver) = detect_installed_version(&dest) {
            println!("{} {} installed successfully.", tool.name, ver);
        } else {
            println!(
                "{} installed to {} (could not detect version).",
                tool.name,
                dest.display()
            );
        }
    } else {
        return Err(format!(
            "installation of {} failed — binary not found at {}",
            tool.name,
            dest.display()
        )
        .into());
    }

    Ok(())
}

/// Handles `diegops tool update [name]`.
pub fn update(name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    match name {
        Some(n) => install(n),
        None => {
            // Update all installed tools
            let bin = bin_dir()?;
            let mut updated = 0u32;
            let mut errors = 0u32;

            for tool in REGISTRY {
                let path = bin.join(if cfg!(windows) {
                    format!("{}.exe", tool.binary_name)
                } else {
                    tool.binary_name.to_string()
                });

                if path.exists() {
                    eprintln!("--- {} ---", tool.name);
                    match install(tool.name) {
                        Ok(()) => updated += 1,
                        Err(e) => {
                            eprintln!("FAIL {}: {e}", tool.name);
                            errors += 1;
                        }
                    }
                }
            }

            if updated == 0 && errors == 0 {
                println!("No tools installed. Run 'diegops tool install <name>' first.");
            } else {
                println!("{updated} tool(s) updated, {errors} error(s).");
            }

            if errors > 0 {
                return Err(format!("{errors} tool(s) failed to update").into());
            }

            Ok(())
        }
    }
}

/// Handles `diegops tool remove <name>`.
pub fn remove(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tool = find_tool(name).ok_or_else(|| {
        format!("unknown tool: '{name}'. Run 'diegops tool list' to see available tools")
    })?;

    let dest = tool_bin_path(tool)?;
    if dest.exists() {
        fs::remove_file(&dest)?;
        println!("{} removed.", tool.name);
    } else {
        println!("{} is not installed.", tool.name);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Passthrough — run a managed tool directly
// ---------------------------------------------------------------------------

/// Forwards arguments to a managed tool binary, propagating its exit code.
///
/// If the child process exits with a non-zero code, `std::process::exit` is
/// called directly because Rust's `Result` cannot propagate arbitrary exit codes.
pub fn passthrough(name: &str, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let tool = find_tool(name).ok_or_else(|| format!("unknown tool: '{name}'"))?;

    let bin_path = tool_bin_path(tool)?;
    if !bin_path.exists() {
        return Err(format!(
            "{} not installed. Run 'diegops tool install {}' first.",
            tool.name, tool.name
        )
        .into());
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_10_tools() {
        assert_eq!(REGISTRY.len(), 10);
    }

    #[test]
    fn all_tool_names_are_unique() {
        let mut names: Vec<&str> = REGISTRY.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), REGISTRY.len());
    }

    #[test]
    fn find_tool_by_name() {
        assert!(find_tool("gh").is_some());
        assert!(find_tool("kubectl").is_some());
        assert!(find_tool("nonexistent").is_none());
    }

    #[test]
    fn find_tool_by_binary_name() {
        // trippy's binary is "trip"
        let tool = find_tool("trip").expect("should find trippy by binary name");
        assert_eq!(tool.name, "trippy");
    }

    #[test]
    fn asset_name_gh() {
        let tool = find_tool("gh").unwrap();
        let name = asset_name(tool, "v2.50.0");
        assert!(
            name.starts_with("gh_2.50.0_"),
            "unexpected asset name: {name}"
        );
        assert!(name.ends_with(".tar.gz"), "unexpected asset name: {name}");
    }

    #[test]
    fn asset_name_vault() {
        let tool = find_tool("vault").unwrap();
        let name = asset_name(tool, "v1.17.0");
        assert!(
            name.starts_with("vault_1.17.0_"),
            "unexpected asset name: {name}"
        );
        assert!(name.ends_with(".zip"), "unexpected asset name: {name}");
    }

    #[test]
    fn asset_name_helm() {
        let tool = find_tool("helm").unwrap();
        let name = asset_name(tool, "v3.15.0");
        assert!(
            name.starts_with("helm-v3.15.0-"),
            "unexpected asset name: {name}"
        );
    }

    #[test]
    fn asset_name_trippy() {
        let tool = find_tool("trippy").unwrap();
        let name = asset_name(tool, "0.11.0");
        assert!(
            name.starts_with("trippy-0.11.0-"),
            "unexpected asset name: {name}"
        );
        assert!(
            name.contains(RUST_TARGET),
            "should contain target triple: {name}"
        );
    }

    #[test]
    fn asset_name_jq_uses_macos() {
        let tool = find_tool("jq").unwrap();
        let name = asset_name(tool, "jq-1.7.1");
        if cfg!(target_os = "macos") {
            assert!(name.contains("macos"), "expected 'macos' in: {name}");
        }
    }

    #[test]
    fn extract_semver_basic() {
        assert_eq!(extract_semver("gh version 2.50.0"), Some("2.50.0".into()));
        assert_eq!(
            extract_semver("Terraform v1.9.0\non linux_amd64"),
            Some("1.9.0".into())
        );
        assert_eq!(extract_semver("no version here"), None);
    }

    #[test]
    fn platform_os_is_known() {
        assert_ne!(platform_os(), "unknown");
    }

    #[test]
    fn platform_arch_is_known() {
        assert_ne!(platform_arch(), "unknown");
    }

    #[test]
    fn download_url_kubectl() {
        let tool = find_tool("kubectl").unwrap();
        let url = download_url(tool, "v1.30.0");
        assert!(
            url.starts_with("https://dl.k8s.io/release/v1.30.0/bin/"),
            "unexpected URL: {url}"
        );
        assert!(url.ends_with("/kubectl"), "unexpected URL: {url}");
    }

    #[test]
    fn download_url_gh() {
        let tool = find_tool("gh").unwrap();
        let url = download_url(tool, "v2.50.0");
        assert!(
            url.starts_with("https://github.com/cli/cli/releases/download/v2.50.0/"),
            "unexpected URL: {url}"
        );
    }

    #[test]
    fn is_known_tool_works() {
        assert!(is_known_tool("gh"));
        assert!(is_known_tool("trip")); // binary name of trippy
        assert!(!is_known_tool("nonexistent"));
    }

    #[test]
    fn asset_name_terraform() {
        let tool = find_tool("terraform").unwrap();
        let name = asset_name(tool, "v1.9.0");
        assert!(
            name.starts_with("terraform_1.9.0_"),
            "unexpected asset name: {name}"
        );
        assert!(name.ends_with(".zip"), "unexpected asset name: {name}");
    }

    #[test]
    fn asset_name_k9s_no_version() {
        let tool = find_tool("k9s").unwrap();
        let name = asset_name(tool, "v0.32.5");
        // k9s does not include version in filename
        assert!(
            !name.contains("0.32.5"),
            "k9s asset should NOT contain version: {name}"
        );
        assert!(name.starts_with("k9s_"), "unexpected asset name: {name}");
        assert!(name.ends_with(".tar.gz"), "unexpected asset name: {name}");
    }

    #[test]
    fn asset_name_yq() {
        let tool = find_tool("yq").unwrap();
        let name = asset_name(tool, "v4.44.0");
        let os = platform_os();
        let arch = platform_arch();
        assert_eq!(name, format!("yq_{os}_{arch}.tar.gz"));
    }

    #[test]
    fn asset_name_trivy_capitalization() {
        let tool = find_tool("trivy").unwrap();
        let name = asset_name(tool, "v0.52.0");
        assert!(
            name.starts_with("trivy_0.52.0_"),
            "unexpected asset name: {name}"
        );
        // Verify OS/arch capitalization
        if cfg!(target_os = "macos") {
            assert!(name.contains("macOS"), "expected 'macOS' in: {name}");
            if cfg!(target_arch = "aarch64") {
                assert!(name.contains("ARM64"), "expected 'ARM64' in: {name}");
            } else {
                assert!(name.contains("64bit"), "expected '64bit' in: {name}");
            }
        } else if cfg!(target_os = "linux") {
            assert!(name.contains("Linux"), "expected 'Linux' in: {name}");
        }
    }

    #[test]
    fn asset_name_kubectl_is_bare() {
        let tool = find_tool("kubectl").unwrap();
        let name = asset_name(tool, "v1.30.0");
        assert_eq!(name, "kubectl");
    }

    #[test]
    fn extract_semver_gh_version_output() {
        assert_eq!(
            extract_semver("gh version 2.50.0 (2024-06-01)"),
            Some("2.50.0".into())
        );
    }

    #[test]
    fn extract_semver_terraform_output() {
        assert_eq!(extract_semver("Terraform v1.9.0"), Some("1.9.0".into()));
    }

    #[test]
    fn extract_semver_vault_output() {
        assert_eq!(extract_semver("vault v1.17.0"), Some("1.17.0".into()));
    }

    #[test]
    fn extract_semver_bare_tag() {
        assert_eq!(extract_semver("v0.32.0"), Some("0.32.0".into()));
    }

    #[test]
    fn extract_semver_kubectl_output() {
        assert_eq!(
            extract_semver("Client Version: v1.30.0"),
            Some("1.30.0".into())
        );
    }
}
