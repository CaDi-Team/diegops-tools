//! ktool sidecar management: download/update and proxy the ktool binary.
//!
//! `diegops ktool update` downloads the latest ktool from GitHub releases.
//! Any other arguments are forwarded to the managed ktool binary at
//! `~/.diegops/bin/ktool`, propagating its exit code.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

// ---------------------------------------------------------------------------
// Compile-time target detection (same as update.rs)
// ---------------------------------------------------------------------------

/// The Rust target triple this binary was compiled for.
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
    "ktool: unsupported target platform — add a CURRENT_TARGET constant for this target"
);

const RELEASES_API: &str =
    "https://api.github.com/repos/CaDi-Team/karluiz-tool-cli/releases/latest";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the path to the managed ktool binary: `~/.diegops/bin/ktool`.
fn ktool_bin_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let bin_dir = super::common::diegops_dir()?.join("bin");
    let name = if cfg!(windows) { "ktool.exe" } else { "ktool" };
    Ok(bin_dir.join(name))
}

/// Fetches the latest release metadata from the GitHub API.
fn fetch_latest(token: Option<&str>) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let mut req = ureq::get(RELEASES_API)
        .set("User-Agent", &ua)
        .set("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    let response = req.call();

    match response {
        Ok(resp) => Ok(resp.into_json()?),
        Err(ureq::Error::Status(404, _)) if token.is_none() => Err(
            "GitHub API returned 404. If this is a private repo, run 'diegops auth gh login <PAT>' first".into(),
        ),
        Err(ureq::Error::Status(401 | 403, _)) => Err(
            "stored GitHub token is no longer valid. Run 'diegops auth gh login <PAT>' to update it"
                .into(),
        ),
        Err(e) => Err(e.into()),
    }
}

/// Finds the download URL for the ktool asset matching the current target.
fn find_asset_url(
    release: &serde_json::Value,
    has_token: bool,
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
                // Private repos: use the API URL with Accept: application/octet-stream.
                // Public repos: use browser_download_url (no auth needed).
                let url_field = if has_token {
                    "url"
                } else {
                    "browser_download_url"
                };
                let url = asset[url_field]
                    .as_str()
                    .ok_or(format!("asset missing '{url_field}'"))?;
                return Ok(url.to_owned());
            }
        }
    }

    Err(format!("no ktool release asset found for target '{CURRENT_TARGET}'").into())
}

/// Downloads a URL and returns the raw bytes.
fn download(url: &str, token: Option<&str>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let mut req = ureq::get(url).set("User-Agent", &ua);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
        // API asset URLs require Accept: application/octet-stream to get the binary.
        req = req.set("Accept", "application/octet-stream");
    }
    let mut buf = Vec::new();
    req.call()?.into_reader().read_to_end(&mut buf)?;
    Ok(buf)
}

/// Extracts the `ktool` binary from a `.tar.gz` archive (all Unix targets).
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

/// Extracts `ktool.exe` from a `.zip` archive (Windows target).
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

/// Installs the ktool binary to `~/.diegops/bin/ktool`.
fn install_binary(binary: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let path = ktool_bin_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, binary)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

/// Returns the version string of the currently installed ktool, or `None`.
fn installed_version() -> Option<String> {
    let path = ktool_bin_path().ok()?;
    if !path.exists() {
        return None;
    }
    let output = Command::new(&path).arg("version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract version tag (e.g. "ktool v1.2.3" -> "v1.2.3", or just "v1.2.3")
    let trimmed = stdout.trim();
    for word in trimmed.split_whitespace() {
        if word.starts_with('v') {
            return Some(word.to_owned());
        }
    }
    // Fallback: return the whole trimmed output
    Some(trimmed.to_owned())
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Dispatches ktool subcommands.
///
/// - `ktool update` downloads/updates the managed ktool binary.
/// - Any other args are forwarded to the managed binary.
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.first().map(|s| s.as_str()) == Some("update") {
        return update();
    }

    // --help is handled by diegops itself (clap), not forwarded
    if args.first().map(|s| s.as_str()) == Some("--help") {
        println!("Manage and run ktool (karluiz tools)");
        println!();
        println!("Usage: diegops ktool <COMMAND|ARGS>");
        println!();
        println!("Commands:");
        println!("  update    Download or update ktool to the latest version");
        println!("  <args>    Any other arguments are forwarded to ktool");
        return Ok(());
    }

    passthrough(args)
}

/// Downloads the latest ktool release and installs it to `~/.diegops/bin/ktool`.
fn update() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Checking for ktool updates...");

    let token = super::auth::load_gh_token().ok().flatten();
    let release = fetch_latest(token.as_deref())?;

    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or("GitHub API response missing 'tag_name'")?;

    if let Some(current) = installed_version() {
        if current == latest_tag {
            println!("ktool already up to date ({current}).");
            return Ok(());
        }
        println!("ktool update available: {current} -> {latest_tag}");
    } else {
        println!("Installing ktool {latest_tag}...");
    }

    eprintln!("Downloading ktool {latest_tag} for {CURRENT_TARGET}...");
    let url = find_asset_url(&release, token.is_some())?;
    let bytes = download(&url, token.as_deref())?;

    eprintln!("Extracting...");
    let binary = extract_binary(&bytes)?;

    eprintln!("Installing...");
    install_binary(&binary)?;

    println!("ktool updated to {latest_tag}.");
    Ok(())
}

/// Forwards arguments to the managed ktool binary, propagating its exit code.
///
/// If the child process exits with a non-zero code, `std::process::exit` is
/// called directly because Rust's `Result` cannot propagate arbitrary exit
/// codes through `main`.
fn passthrough(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let bin_path = ktool_bin_path()?;
    if !bin_path.exists() {
        return Err("ktool not installed. Run 'diegops ktool update' first.".into());
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
