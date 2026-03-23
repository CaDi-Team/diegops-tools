//! Repository workspace management — clone, list, and diff repos from a YAML config.
//!
//! Config file (`repos.yaml`) format:
//! ```yaml
//! targets:
//!   - path: $HOME/github/org/group
//!     repos:
//!       - git@github.com:Org/repo-name.git
//! ```
//!
//! Config resolution order:
//! 1. `--config <path>` CLI flag
//! 2. `DIEGOPS_REPOS_CONFIG` environment variable
//! 3. `~/.diegops/repos.yaml` (default)

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Clap sub-command definitions (re-exported so main.rs can embed them)
// ---------------------------------------------------------------------------

/// Repository workspace sub-commands.
#[derive(clap::Subcommand)]
pub enum RepoCommand {
    /// Clone all repos from the config. Existing clones are skipped (idempotent).
    Apply {
        /// Only process targets whose expanded path starts with this prefix.
        ///
        /// Accepts $HOME-prefixed paths, e.g. `$HOME/github/cadilabs/products/cadibrain`.
        #[arg(long)]
        path: Option<String>,

        /// Path to the repos.yaml config file.
        ///
        /// Defaults to ~/.config/diegops/repos.yaml or $DIEGOPS_REPOS_CONFIG.
        #[arg(long)]
        config: Option<String>,
    },

    /// List repos defined in the config that are already cloned locally.
    List {
        /// Path to the repos.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Show repos defined in the config that are NOT yet cloned locally.
    #[command(name = "list-diff")]
    ListDiff {
        /// Path to the repos.yaml config file.
        #[arg(long)]
        config: Option<String>,
    },

    /// Create a sample repos.yaml in ~/.diegops/ to get started.
    ///
    /// Does nothing if the file already exists (idempotent).
    Init,
}

// ---------------------------------------------------------------------------
// Config structures
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ReposConfig {
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    path: String,
    repos: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Clones all repos from the config that are not yet present locally.
///
/// Idempotent: targets whose directory already contains a `.git` folder are
/// skipped without error. The target directory is created if it does not exist.
///
/// If `path_filter` is given, only targets whose expanded path starts with the
/// filter prefix are processed. `$HOME` in the filter is expanded automatically.
pub fn apply(
    config_path: Option<&Path>,
    path_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(config_path)?;
    let filter = path_filter.map(expand_home);

    let mut n_cloned: usize = 0;
    let mut n_skipped: usize = 0;
    let mut failures: Vec<String> = Vec::new();

    for target in &config.targets {
        let dest = expand_home(&target.path);

        if let Some(ref f) = filter {
            if !dest.starts_with(f) {
                continue;
            }
        }

        eprintln!("\n{}", target.path);

        if let Err(e) = fs::create_dir_all(&dest) {
            eprintln!("  error  could not create {}: {e}", dest.display());
            // Record every repo in this target as failed
            for url in &target.repos {
                failures.push(format!("{url} (directory error)"));
            }
            continue;
        }

        for repo_url in &target.repos {
            let Some(name) = repo_name(repo_url) else {
                eprintln!("  error  could not parse repo name from: {repo_url}");
                failures.push(repo_url.clone());
                continue;
            };

            let clone_dest = dest.join(name);

            if clone_dest.join(".git").exists() {
                eprintln!("  skip   {name}");
                n_skipped += 1;
                continue;
            }

            eprintln!("  clone  {name}");
            match git_clone(repo_url, &clone_dest) {
                Ok(()) => n_cloned += 1,
                Err(e) => {
                    eprintln!("  fail   {name}: {e}");
                    failures.push(format!("{repo_url} → {}", clone_dest.display()));
                }
            }
        }
    }

    eprintln!();
    println!(
        "Done: {n_cloned} cloned, {n_skipped} skipped, {} failed",
        failures.len()
    );

    if !failures.is_empty() {
        eprintln!("\nFailed repos:");
        for f in &failures {
            eprintln!("  {f}");
        }
        return Err(format!("{} clone(s) failed", failures.len()).into());
    }

    Ok(())
}

/// Prints repos from the config that are currently cloned locally (have a `.git` dir).
///
/// Output goes to stdout so it can be piped. Summary count goes to stderr.
pub fn list(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(config_path)?;
    let mut total: usize = 0;

    for target in &config.targets {
        let dest = expand_home(&target.path);
        let cloned: Vec<&str> = target
            .repos
            .iter()
            .filter_map(|url| {
                let name = repo_name(url)?;
                if dest.join(name).join(".git").exists() {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        if !cloned.is_empty() {
            println!("{}", target.path);
            for name in &cloned {
                println!("  {name}");
            }
            println!();
            total += cloned.len();
        }
    }

    eprintln!("{total} repositories cloned locally.");
    Ok(())
}

/// Prints repos from the config that are NOT yet cloned locally.
///
/// Output goes to stdout so it can be piped. Summary and hints go to stderr.
pub fn list_diff(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(config_path)?;
    let mut total: usize = 0;

    for target in &config.targets {
        let dest = expand_home(&target.path);
        let missing: Vec<(&str, &str)> = target
            .repos
            .iter()
            .filter_map(|url| {
                let name = repo_name(url)?;
                if !dest.join(name).join(".git").exists() {
                    Some((name, url.as_str()))
                } else {
                    None
                }
            })
            .collect();

        if !missing.is_empty() {
            println!("{}", target.path);
            for (name, url) in &missing {
                println!("  {name:<45}  {url}");
            }
            println!();
            total += missing.len();
        }
    }

    if total == 0 {
        println!("All repositories are cloned locally.");
    } else {
        eprintln!("{total} repositories not yet cloned. Run `diegops repo apply` to clone them.");
    }

    Ok(())
}

/// Creates `~/.diegops/repos.yaml` with a commented sample config.
///
/// Idempotent: if the file already exists it prints its location and exits 0
/// without modifying anything.
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let dir = diegops_dir()?;
    let config_path = dir.join("repos.yaml");

    if config_path.exists() {
        println!("Config already exists: {}", config_path.display());
        println!("Edit it directly or run `diegops repo list-diff` to see missing repos.");
        return Ok(());
    }

    fs::create_dir_all(&dir)?;
    fs::write(&config_path, SAMPLE_CONFIG)?;

    println!("Created: {}", config_path.display());
    println!("Edit the file to add your repositories, then run `diegops repo apply`.");
    Ok(())
}

const SAMPLE_CONFIG: &str = "\
# diegops repository workspace configuration
#
# Each target defines a local directory and the git repos to clone into it.
# Run `diegops repo apply` to clone everything.
# Run `diegops repo list-diff` to see what is missing.
#
# Path rules:
#   - Use $HOME as a portable prefix (works on Linux, macOS, and WSL).
#   - Repos are cloned as <path>/<repo-name> (name derived from the URL).
#   - SSH keys must be configured before running apply.

targets:

  - path: $HOME/github/my-org/products/my-product
    repos:
      - git@github.com:my-org/my-product-frontend.git
      - git@github.com:my-org/my-product-backend.git

  - path: $HOME/github/my-org/infra
    repos:
      - git@github.com:my-org/infra-iac.git
      - git@github.com:my-org/infra-k8s-config.git
";

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn load_config(path: Option<&Path>) -> Result<ReposConfig, Box<dyn std::error::Error>> {
    let config_path = match path {
        Some(p) => p.to_owned(),
        None => {
            if let Ok(env_path) = std::env::var("DIEGOPS_REPOS_CONFIG") {
                PathBuf::from(env_path)
            } else {
                default_config_path()?
            }
        }
    };

    let content = fs::read_to_string(&config_path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            format!(
                "repos config not found: {}\n  Hint: create it or pass --config <path>",
                config_path.display()
            )
        } else {
            format!("could not read {}: {e}", config_path.display())
        }
    })?;

    serde_yaml::from_str::<ReposConfig>(&content)
        .map_err(|e| format!("invalid repos config {}: {e}", config_path.display()).into())
}

fn default_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(home_dir()?.join(".diegops").join("repos.yaml"))
}

fn diegops_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(home_dir()?.join(".diegops"))
}

/// Returns the current user's home directory.
///
/// Checks `$HOME` first (Unix convention), then `$USERPROFILE` (Windows convention).
fn home_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| "home directory not set ($HOME / $USERPROFILE)".into())
}

/// Expands a `$HOME`-prefixed path string to an absolute `PathBuf`.
///
/// Uses `.join()` per component — never string concatenation — so the result
/// is always a valid `PathBuf` regardless of platform path separator.
///
/// Also handles the `/$HOME/...` form (leading slash before `$HOME`) in case
/// the caller passes a shell-expanded value with an extra slash.
fn expand_home(path: &str) -> PathBuf {
    // Normalise `/$HOME/...` → `$HOME/...`
    let path = if path.starts_with("/$HOME") {
        &path[1..]
    } else {
        path
    };

    if let Some(rest) = path.strip_prefix("$HOME") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_default();

        let relative = rest.trim_start_matches('/');
        if relative.is_empty() {
            return home;
        }
        let mut p = home;
        for component in relative.split('/') {
            if !component.is_empty() {
                p = p.join(component);
            }
        }
        p
    } else {
        PathBuf::from(path)
    }
}

/// Extracts the repo name from a git SSH URL.
///
/// Works for both `git@github.com:org/repo.git` and custom host aliases
/// like `git@github-pacifico:org/repo.git`.
fn repo_name(url: &str) -> Option<&str> {
    url.rsplit('/').next().map(|s| s.trim_end_matches(".git"))
}

/// Runs `git clone <url> <dest>` and returns an error with stderr on failure.
fn git_clone(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(dest)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_owned().into())
    }
}
