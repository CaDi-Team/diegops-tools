//! The `diegops shell` command family — shell environment setup.

use clap::Subcommand;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Shell subcommands.
#[derive(Subcommand)]
pub enum ShellCommand {
    /// Set up zsh, oh-my-zsh, plugins, and managed shell config
    Init,
}

const ZSH_CONFIG: &str = "\
# diegops managed shell config — synced across workstations
# Do not edit manually — changes will be overwritten by `diegops shell init`.
# Machine-specific overrides go in ~/.zshrc AFTER the source line.

# -- Oh My Zsh -----------------------------------------------------------------
export ZSH=\"$HOME/.oh-my-zsh\"
ZSH_THEME=\"robbyrussell\"
plugins=(
  git
  kubectl
  docker
  terraform
  helm
  gh
  ansible
  colored-man-pages
  command-not-found
  zsh-autosuggestions
  zsh-syntax-highlighting
)
source \"$ZSH/oh-my-zsh.sh\"

# -- PATH ----------------------------------------------------------------------
export PATH=\"$HOME/.diegops/bin:$HOME/.local/bin:$PATH\"

# -- Kubeconfig discovery -------------------------------------------------------
export KUBECONFIG=$(find ~/.kube -maxdepth 1 -type f \\( -name \"*.yaml\" -o -name \"*.yml\" -o -name \"config\" \\) 2>/dev/null | tr '\\n' ':' | sed 's/:$//')

# -- Environment ----------------------------------------------------------------
export VAULT_ADDR=\"https://cadi-vault.cadi-labs.com\"

# -- Aliases --------------------------------------------------------------------
alias code='code-insiders'
alias cc='claude --dangerously-skip-permissions'
alias k='kubectl'
";

const BASH_CONFIG: &str = "\
# diegops managed shell config (bash) — synced across workstations
# Do not edit manually — changes will be overwritten by `diegops shell init`.
# Machine-specific overrides go in ~/.bashrc AFTER the source line.

# -- PATH ----------------------------------------------------------------------
export PATH=\"$HOME/.diegops/bin:$HOME/.local/bin:$PATH\"

# -- Kubeconfig discovery -------------------------------------------------------
export KUBECONFIG=$(find ~/.kube -maxdepth 1 -type f \\( -name \"*.yaml\" -o -name \"*.yml\" -o -name \"config\" \\) 2>/dev/null | tr '\\n' ':' | sed 's/:$//')

# -- Environment ----------------------------------------------------------------
export VAULT_ADDR=\"https://cadi-vault.cadi-labs.com\"

# -- Aliases --------------------------------------------------------------------
alias code='code-insiders'
alias cc='claude --dangerously-skip-permissions'
alias k='kubectl'
";

const ZSH_SOURCE_LINE: &str =
    "[ -f \"$HOME/.diegops/shell/diegops.zsh\" ] && source \"$HOME/.diegops/shell/diegops.zsh\"";

const BASH_SOURCE_LINE: &str =
    "[ -f \"$HOME/.diegops/shell/diegops.bash\" ] && source \"$HOME/.diegops/shell/diegops.bash\"";

const ZSH_SOURCE_MARKER: &str = "diegops/shell/diegops.zsh";
const BASH_SOURCE_MARKER: &str = "diegops/shell/diegops.bash";

/// Sets up the shell environment: installs zsh, oh-my-zsh, custom plugins,
/// writes managed config files, and injects source lines.
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let home = super::common::home_dir()?;

    // Step 1: Check/install zsh
    eprint!("[1/7] Checking zsh ... ");
    ensure_zsh()?;

    // Step 2: Check/set default shell
    eprint!("[2/7] Checking default shell ... ");
    ensure_default_shell();

    // Step 3: Check/install oh-my-zsh
    eprint!("[3/7] Checking oh-my-zsh ... ");
    ensure_ohmyzsh(&home)?;

    // Step 4: Check/install custom plugins
    eprint!("[4/7] Checking custom plugins ... ");
    ensure_custom_plugins(&home);

    // Step 5: Write managed config
    eprint!("[5/7] Writing managed config ... ");
    write_managed_config(&home)?;

    // Step 6: Inject source line in .zshrc
    eprint!("[6/7] Injecting source line in .zshrc ... ");
    inject_source_line(&home.join(".zshrc"), ZSH_SOURCE_MARKER, ZSH_SOURCE_LINE)?;

    // Step 7: Bash support
    eprint!("[7/7] Bash support ... ");
    setup_bash(&home);

    eprintln!();
    eprintln!("Shell configured. Restart your terminal or run: source ~/.zshrc");
    Ok(())
}

/// Checks if zsh is installed; installs it if not.
fn ensure_zsh() -> Result<(), Box<dyn std::error::Error>> {
    if command_exists("zsh") {
        eprintln!("OK (already installed)");
        return Ok(());
    }

    if command_exists("apt-get") {
        run_cmd("sudo", &["apt-get", "update", "-qq"])?;
        run_cmd("sudo", &["apt-get", "install", "-y", "zsh"])?;
    } else if command_exists("brew") {
        run_cmd("brew", &["install", "zsh"])?;
    } else if command_exists("dnf") {
        run_cmd("sudo", &["dnf", "install", "-y", "zsh"])?;
    } else if command_exists("apk") {
        run_cmd("sudo", &["apk", "add", "zsh"])?;
    } else {
        return Err(
            "zsh not found and no supported package manager detected. Install zsh manually.".into(),
        );
    }

    eprintln!("INSTALLED");
    Ok(())
}

/// Checks if zsh is the default shell; sets it if not.
fn ensure_default_shell() {
    let current_shell = std::env::var("SHELL").unwrap_or_default();
    if current_shell.ends_with("/zsh") {
        eprintln!("OK (already zsh)");
        return;
    }

    let zsh_path = which("zsh").unwrap_or_else(|| "/usr/bin/zsh".to_string());
    match run_cmd("chsh", &["-s", &zsh_path]) {
        Ok(()) => eprintln!("OK (set to zsh)"),
        Err(e) => eprintln!("WARN: could not set default shell: {e}"),
    }
}

/// Checks if oh-my-zsh is installed; clones it if not.
fn ensure_ohmyzsh(home: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let omz_dir = home.join(".oh-my-zsh");
    if omz_dir.exists() {
        eprintln!("OK (already installed)");
        return Ok(());
    }

    run_cmd(
        "git",
        &[
            "clone",
            "--depth=1",
            "https://github.com/ohmyzsh/ohmyzsh.git",
            &omz_dir.to_string_lossy(),
        ],
    )?;
    eprintln!("INSTALLED");
    Ok(())
}

/// Checks and installs custom oh-my-zsh plugins.
fn ensure_custom_plugins(home: &PathBuf) {
    let custom_dir = home.join(".oh-my-zsh").join("custom").join("plugins");

    let plugins = [
        (
            "zsh-autosuggestions",
            "https://github.com/zsh-users/zsh-autosuggestions.git",
        ),
        (
            "zsh-syntax-highlighting",
            "https://github.com/zsh-users/zsh-syntax-highlighting.git",
        ),
    ];

    let mut results: Vec<String> = Vec::new();
    for (name, url) in &plugins {
        let dest = custom_dir.join(name);
        if dest.exists() {
            results.push(format!("OK {name}"));
            continue;
        }
        match run_cmd("git", &["clone", "--depth=1", url, &dest.to_string_lossy()]) {
            Ok(()) => results.push(format!("INSTALLED {name}")),
            Err(e) => results.push(format!("WARN {name}: {e}")),
        }
    }
    eprintln!("{}", results.join(", "));
}

/// Writes the managed zsh config file.
fn write_managed_config(home: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let shell_dir = home.join(".diegops").join("shell");
    fs::create_dir_all(&shell_dir)?;

    let zsh_path = shell_dir.join("diegops.zsh");
    fs::write(&zsh_path, ZSH_CONFIG)?;
    eprintln!("OK ({})", zsh_path.display());
    Ok(())
}

/// Injects a source line at the end of an rc file if not already present.
fn inject_source_line(
    rc_path: &PathBuf,
    marker: &str,
    source_line: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !rc_path.exists() {
        fs::write(rc_path, "")?;
    }

    let content = fs::read_to_string(rc_path)?;
    if content.contains(marker) {
        eprintln!("OK (already present)");
        return Ok(());
    }

    let mut appended = content;
    if !appended.ends_with('\n') && !appended.is_empty() {
        appended.push('\n');
    }
    appended.push_str(&format!(
        "\n# diegops managed config — loaded last\n{source_line}\n"
    ));
    fs::write(rc_path, appended)?;
    eprintln!("OK (injected)");
    Ok(())
}

/// Sets up bash support: writes diegops.bash and injects source line.
fn setup_bash(home: &PathBuf) {
    let bashrc = home.join(".bashrc");
    if !bashrc.exists() {
        eprintln!("SKIP (no .bashrc found)");
        return;
    }

    let shell_dir = home.join(".diegops").join("shell");
    if let Err(e) = fs::create_dir_all(&shell_dir) {
        eprintln!("WARN: could not create shell dir: {e}");
        return;
    }

    let bash_path = shell_dir.join("diegops.bash");
    if let Err(e) = fs::write(&bash_path, BASH_CONFIG) {
        eprintln!("WARN: could not write diegops.bash: {e}");
        return;
    }

    match inject_source_line(&bashrc, BASH_SOURCE_MARKER, BASH_SOURCE_LINE) {
        Ok(()) => {}
        Err(e) => eprintln!("WARN: could not inject bash source line: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns true if a command exists on PATH.
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Returns the full path of a command, or None.
fn which(cmd: &str) -> Option<String> {
    Command::new("which").arg(cmd).output().ok().and_then(|o| {
        if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else {
            None
        }
    })
}

/// Runs a command, returning an error if it fails.
fn run_cmd(program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}").into())
    }
}
