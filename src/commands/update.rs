//! Self-update command: fetch the latest GitHub release and replace the running binary.
//!
//! Progress and diagnostics go to **stderr**; the final status line goes to **stdout**
//! so that callers can capture or suppress it independently.
//!
//! The command is idempotent: running it when already on the latest version prints
//! "Already up to date" and exits 0.

use std::fs;
use std::io::Read;

// ---------------------------------------------------------------------------
// Compile-time target detection
// ---------------------------------------------------------------------------

/// The Rust target triple this binary was compiled for.
///
/// Used to select the correct release asset from the GitHub release page.
/// Each supported target produces a uniquely named archive in the CD pipeline.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const CURRENT_TARGET: &str = "x86_64-pc-windows-msvc";

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

// Catch unsupported platforms at compile time rather than at runtime.
#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
)))]
compile_error!(
    "diegops update: unsupported target platform — add a CURRENT_TARGET constant for this target"
);

const RELEASES_API: &str =
    "https://api.github.com/repos/dpinto-config/diegops-tools/releases/latest";

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Checks GitHub for a newer release and, if found, downloads and installs it in place.
///
/// Idempotent: exits 0 with a friendly message when already on the latest version.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Checking for updates...");

    let release = fetch_latest()?;

    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or("GitHub API response missing 'tag_name'")?;

    let current = format!("v{}", env!("CARGO_PKG_VERSION"));

    if latest_tag == current.as_str() {
        println!("Already up to date ({current}).");
        return Ok(());
    }

    println!("Update available: {current} → {latest_tag}");
    eprintln!("Downloading {latest_tag} for {CURRENT_TARGET}...");

    let url = find_asset_url(&release)?;
    let bytes = download(&url)?;

    eprintln!("Extracting...");
    let binary = extract_binary(&bytes)?;

    eprintln!("Installing...");
    replace_self(&binary)?;

    println!("Updated to {latest_tag}. Restart diegops to use the new version.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Network helpers
// ---------------------------------------------------------------------------

fn fetch_latest() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let response = ureq::get(RELEASES_API)
        .set("User-Agent", &ua)
        .set("Accept", "application/vnd.github.v3+json")
        .call()?;
    Ok(response.into_json()?)
}

fn find_asset_url(release: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    let assets = release["assets"]
        .as_array()
        .ok_or("GitHub API response missing 'assets'")?;

    // Asset names produced by cd.yml: diegops-<tag>-<target>.<ext>
    let suffix = if cfg!(windows) {
        format!("{CURRENT_TARGET}.zip")
    } else {
        format!("{CURRENT_TARGET}.tar.gz")
    };

    for asset in assets {
        if let Some(name) = asset["name"].as_str() {
            if name.ends_with(&suffix) {
                let url = asset["browser_download_url"]
                    .as_str()
                    .ok_or("asset missing 'browser_download_url'")?;
                return Ok(url.to_owned());
            }
        }
    }

    Err(format!("no release asset found for target '{CURRENT_TARGET}'").into())
}

fn download(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let ua = format!("diegops/{}", env!("CARGO_PKG_VERSION"));
    let mut buf = Vec::new();
    ureq::get(url)
        .set("User-Agent", &ua)
        .call()?
        .into_reader()
        .read_to_end(&mut buf)?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Archive extraction — platform-specific
// ---------------------------------------------------------------------------

/// Extracts the `diegops` binary from a `.tar.gz` archive (all Unix targets).
#[cfg(unix)]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    use std::ffi::OsStr;
    use tar::Archive;

    let mut archive = Archive::new(GzDecoder::new(bytes));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.file_name() == Some(OsStr::new("diegops")) {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    Err("'diegops' binary not found in archive".into())
}

/// Extracts `diegops.exe` from a `.zip` archive (Windows target).
#[cfg(windows)]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use zip::ZipArchive;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;
    let mut file = archive.by_name("diegops.exe")?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Binary replacement — platform-specific
// ---------------------------------------------------------------------------

/// Replaces the running binary with `binary`.
///
/// **Unix**: sets executable permissions (0o755) then atomically renames the
/// temp file into place. Rename on the same filesystem is atomic.
///
/// **Windows**: Windows prevents deleting a running `.exe` but allows renaming
/// it. The current exe is moved to `.old` and the new binary moved into place.
/// The `.old` file is cleaned up at the start of the next update.
fn replace_self(binary: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = std::env::current_exe()?;
    let tmp = current_exe.with_extension("tmp");

    fs::write(&tmp, binary)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
        // Atomic on the same filesystem (temp file sits next to the binary).
        fs::rename(&tmp, &current_exe)?;
    }

    #[cfg(windows)]
    {
        let old = current_exe.with_extension("old");
        // Best-effort cleanup of previous update's leftover.
        if old.exists() {
            let _ = fs::remove_file(&old);
        }
        // Rename current (permitted for running exe on Windows) then move new in.
        fs::rename(&current_exe, &old)?;
        fs::rename(&tmp, &current_exe)?;
    }

    Ok(())
}
