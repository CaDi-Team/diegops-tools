//! The `diegops shell` command family — shell environment setup.

use clap::Subcommand;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Shell subcommands.
#[derive(Subcommand)]
pub enum ShellCommand {
    /// Set up zsh, oh-my-zsh, plugins, and managed shell config
    #[command(
        long_about = "Set up zsh, oh-my-zsh, plugins, and the diegops managed shell config.\n\n\
        What it changes (each step is skipped when already done):\n  \
        1. installs zsh with your package manager (sudo apt-get/dnf/rpm-ostree/apk, or brew)\n  \
        2. makes zsh your login shell (chsh -s)\n  \
        3. clones oh-my-zsh into ~/.oh-my-zsh\n  \
        4. clones zsh-autosuggestions and zsh-syntax-highlighting into ~/.oh-my-zsh/custom/plugins\n  \
        5. writes ~/.diegops/shell/diegops.zsh (overwritten on every run; your own\n     \
           aliases go in ~/.diegops/shell/aliases.zsh, loaded and never overwritten)\n  \
        6. appends one source line to ~/.zshrc (your own lines are kept)\n  \
        7. if ~/.bashrc exists: writes ~/.diegops/shell/diegops.bash and appends a source line\n\n\
        Run with --dry-run first to see exactly what would happen. In a terminal it asks\n\
        before using sudo or chsh; --yes skips the questions."
    )]
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

# -- Aliases --------------------------------------------------------------------
alias k='kubectl'

# -- Personal additions (yours; synced by 'diegops sync', never overwritten) -------
[ -f \"$HOME/.diegops/shell/aliases.zsh\" ] && source \"$HOME/.diegops/shell/aliases.zsh\"

# -- Completions ----------------------------------------------------------------
command -v diegops >/dev/null && source <(diegops completion zsh)
";

const BASH_CONFIG: &str = "\
# diegops managed shell config (bash) — synced across workstations
# Do not edit manually — changes will be overwritten by `diegops shell init`.
# Machine-specific overrides go in ~/.bashrc AFTER the source line.

# -- PATH ----------------------------------------------------------------------
export PATH=\"$HOME/.diegops/bin:$HOME/.local/bin:$PATH\"

# -- Kubeconfig discovery -------------------------------------------------------
export KUBECONFIG=$(find ~/.kube -maxdepth 1 -type f \\( -name \"*.yaml\" -o -name \"*.yml\" -o -name \"config\" \\) 2>/dev/null | tr '\\n' ':' | sed 's/:$//')

# -- Aliases --------------------------------------------------------------------
alias k='kubectl'

# -- Personal additions (yours; synced by 'diegops sync', never overwritten) -------
[ -f \"$HOME/.diegops/shell/aliases.bash\" ] && source \"$HOME/.diegops/shell/aliases.bash\"

# -- Completions ----------------------------------------------------------------
command -v diegops >/dev/null && source <(diegops completion bash)
";

const ZSH_SOURCE_LINE: &str =
    "[ -f \"$HOME/.diegops/shell/diegops.zsh\" ] && source \"$HOME/.diegops/shell/diegops.zsh\"";

const BASH_SOURCE_LINE: &str =
    "[ -f \"$HOME/.diegops/shell/diegops.bash\" ] && source \"$HOME/.diegops/shell/diegops.bash\"";

const ZSH_SOURCE_MARKER: &str = "diegops/shell/diegops.zsh";
const BASH_SOURCE_MARKER: &str = "diegops/shell/diegops.bash";

/// Sets up the shell environment: installs zsh, oh-my-zsh, custom plugins,
/// writes managed config files, and injects source lines.
const PLUGINS: &[(&str, &str)] = &[
    (
        "zsh-autosuggestions",
        "https://github.com/zsh-users/zsh-autosuggestions.git",
    ),
    (
        "zsh-syntax-highlighting",
        "https://github.com/zsh-users/zsh-syntax-highlighting.git",
    ),
];

/// Package-manager command that installs zsh, if one is available.
fn zsh_install_cmds() -> Option<Vec<Vec<&'static str>>> {
    if command_exists("apt-get") {
        Some(vec![
            vec!["sudo", "apt-get", "update", "-qq"],
            vec!["sudo", "apt-get", "install", "-y", "zsh"],
        ])
    } else if command_exists("brew") {
        Some(vec![vec!["brew", "install", "zsh"]])
    } else if command_exists("rpm-ostree") {
        // Immutable Fedora (Bazzite, Silverblue, Kinoite): rpm-ostree layers packages
        // atomically on top of the read-only base image. --idempotent skips if already
        // installed; --apply-live applies the change to the running system without reboot.
        Some(vec![vec![
            "sudo",
            "rpm-ostree",
            "install",
            "--idempotent",
            "--apply-live",
            "zsh",
        ]])
    } else if command_exists("dnf") {
        Some(vec![vec!["sudo", "dnf", "install", "-y", "zsh"]])
    } else if command_exists("apk") {
        Some(vec![vec!["sudo", "apk", "add", "zsh"]])
    } else {
        None
    }
}

/// Describes what `init` would do on this machine, without changing anything.
pub fn plan(home: &Path) -> Vec<String> {
    let tilde = |p: &Path| p.display().to_string();
    let mut out = Vec::new();
    out.push(if command_exists("zsh") {
        "[1/7] zsh: already installed".to_string()
    } else {
        match zsh_install_cmds() {
            Some(cmds) => format!(
                "[1/7] zsh: would run: {}",
                cmds.iter()
                    .map(|c| c.join(" "))
                    .collect::<Vec<_>>()
                    .join(" && ")
            ),
            None => "[1/7] zsh: not found and no package manager detected (install it by hand)"
                .to_string(),
        }
    });
    let shell = std::env::var("SHELL").unwrap_or_default();
    out.push(if shell.ends_with("/zsh") {
        "[2/7] login shell: already zsh".to_string()
    } else {
        let zsh = which("zsh").unwrap_or_else(|| "/usr/bin/zsh".to_string());
        format!("[2/7] login shell: would run: chsh -s {zsh}")
    });
    let omz = home.join(".oh-my-zsh");
    out.push(if omz.exists() {
        "[3/7] oh-my-zsh: already installed".to_string()
    } else {
        format!(
            "[3/7] oh-my-zsh: would run: git clone --depth=1 https://github.com/ohmyzsh/ohmyzsh.git {}",
            tilde(&omz)
        )
    });
    let plugins = omz.join("custom").join("plugins");
    let missing: Vec<&str> = PLUGINS
        .iter()
        .filter(|(n, _)| !plugins.join(n).exists())
        .map(|(n, _)| *n)
        .collect();
    out.push(if missing.is_empty() {
        "[4/7] plugins: already installed".to_string()
    } else {
        format!(
            "[4/7] plugins: would clone {} into {}",
            missing.join(", "),
            tilde(&plugins)
        )
    });
    let shell_dir = home.join(".diegops").join("shell");
    out.push(format!(
        "[5/7] managed config: would write {}",
        tilde(&shell_dir.join("diegops.zsh"))
    ));
    let rc_line = |rc: &Path, marker: &str| {
        let present = fs::read_to_string(rc)
            .map(|c| c.contains(marker))
            .unwrap_or(false);
        if present {
            format!("source line already in {}", tilde(rc))
        } else {
            format!("would append source line to {}", tilde(rc))
        }
    };
    out.push(format!(
        "[6/7] .zshrc: {}",
        rc_line(&home.join(".zshrc"), ZSH_SOURCE_MARKER)
    ));
    let bashrc = home.join(".bashrc");
    out.push(if bashrc.exists() {
        format!(
            "[7/7] bash: would write {}; {}",
            tilde(&shell_dir.join("diegops.bash")),
            rc_line(&bashrc, BASH_SOURCE_MARKER).replace("would ", "")
        )
    } else {
        "[7/7] bash: skipped (no ~/.bashrc)".to_string()
    });
    out
}

/// Asks before sudo/chsh (ADR 0001 D3): `--yes` proceeds, a terminal asks,
/// no terminal refuses with a hint instead of assuming yes.
fn confirm(question: &str, assume_yes: bool) -> Result<bool, Box<dyn std::error::Error>> {
    if assume_yes {
        return Ok(true);
    }
    crate::ui::confirm(question)
}

pub fn init(dry_run: bool, yes: bool) -> Result<(), Box<dyn std::error::Error>> {
    let home = super::common::home_dir()?;
    if dry_run {
        println!("diegops shell init — dry run (nothing is changed)\n");
        for line in plan(&home) {
            println!("  {line}");
        }
        println!("\nRun 'diegops shell init' to apply.");
        return Ok(());
    }

    // Step 1: Check/install zsh
    eprint!("[1/7] Checking zsh ... ");
    ensure_zsh(yes)?;

    // Step 2: Check/set default shell
    eprint!("[2/7] Checking default shell ... ");
    ensure_default_shell(yes);

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
fn ensure_zsh(yes: bool) -> Result<(), Box<dyn std::error::Error>> {
    if command_exists("zsh") {
        eprintln!("OK (already installed)");
        return Ok(());
    }
    let cmds = zsh_install_cmds()
        .ok_or("zsh not found and no supported package manager detected. Install zsh manually.")?;
    let shown: Vec<String> = cmds.iter().map(|c| c.join(" ")).collect();
    if !confirm(&format!("Install zsh with: {}?", shown.join(" && ")), yes)? {
        return Err(
            "zsh install cancelled\n  hint: install zsh yourself, then rerun 'diegops shell init'"
                .into(),
        );
    }
    for c in &cmds {
        run_cmd(c[0], &c[1..])?;
    }
    eprintln!("INSTALLED");
    Ok(())
}

/// Checks if zsh is the default shell; sets it if not.
fn ensure_default_shell(yes: bool) {
    let current_shell = std::env::var("SHELL").unwrap_or_default();
    if current_shell.ends_with("/zsh") {
        eprintln!("OK (already zsh)");
        return;
    }

    let zsh_path = which("zsh").unwrap_or_else(|| "/usr/bin/zsh".to_string());
    match confirm(
        &format!("Make zsh your login shell (chsh -s {zsh_path})?"),
        yes,
    ) {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("SKIP (kept {current_shell})");
            return;
        }
        Err(_) => {
            eprintln!(
                "SKIP (kept {current_shell}; no terminal to confirm, pass --yes to change it)"
            );
            return;
        }
    }
    match run_cmd("chsh", &["-s", &zsh_path]) {
        Ok(()) => eprintln!("OK (set to zsh)"),
        Err(e) => eprintln!("warning: could not set default shell: {e}"),
    }
}

/// Checks if oh-my-zsh is installed; clones it if not.
fn ensure_ohmyzsh(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
fn ensure_custom_plugins(home: &Path) {
    let custom_dir = home.join(".oh-my-zsh").join("custom").join("plugins");

    let mut results: Vec<String> = Vec::new();
    for (name, url) in PLUGINS {
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

/// Aliases that older versions forced on everyone; kept per user on upgrade.
const OLD_PERSONAL_ALIASES: &[&str] = &[
    "alias code='code-insiders'",
    "alias cc='claude --dangerously-skip-permissions'",
];

/// From an old managed config, the personal aliases it contained (if any).
fn carry_over_aliases(old_managed: &str) -> Option<String> {
    let found: Vec<&str> = OLD_PERSONAL_ALIASES
        .iter()
        .copied()
        .filter(|a| old_managed.lines().any(|l| l.trim() == *a))
        .collect();
    if found.is_empty() {
        return None;
    }
    Some(format!(
        "# Personal aliases — yours to edit. Loaded by the diegops managed config,\n\
         # synced by 'diegops sync', never overwritten by 'diegops shell init'.\n\
         # Moved here from the managed config by diegops (they used to apply to everyone).\n{}\n",
        found.join("\n")
    ))
}

/// Before overwriting the managed file, moves its old personal aliases to
/// ~/.diegops/shell/aliases.<shell> (only if that file does not exist yet).
fn migrate_personal_aliases(home: &Path, shell: &str) {
    let dir = home.join(".diegops").join("shell");
    let personal = dir.join(format!("aliases.{shell}"));
    if personal.exists() {
        return;
    }
    let Ok(old) = fs::read_to_string(dir.join(format!("diegops.{shell}"))) else {
        return;
    };
    if let Some(text) = carry_over_aliases(&old) {
        match fs::write(&personal, text) {
            Ok(()) => eprintln!(
                "\n  note: your aliases (cc, code) moved to {}; edit or delete it there",
                personal.display()
            ),
            Err(e) => eprintln!("\n  warning: could not write {}: {e}", personal.display()),
        }
    }
}

/// Writes the managed zsh config file.
fn write_managed_config(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let shell_dir = home.join(".diegops").join("shell");
    fs::create_dir_all(&shell_dir)?;

    migrate_personal_aliases(home, "zsh");
    let zsh_path = shell_dir.join("diegops.zsh");
    fs::write(&zsh_path, ZSH_CONFIG)?;
    eprintln!("OK ({})", zsh_path.display());
    Ok(())
}

/// Injects a source line at the end of an rc file if not already present.
fn inject_source_line(
    rc_path: &Path,
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
fn setup_bash(home: &Path) {
    let bashrc = home.join(".bashrc");
    if !bashrc.exists() {
        eprintln!("SKIP (no .bashrc found)");
        return;
    }

    let shell_dir = home.join(".diegops").join("shell");
    if let Err(e) = fs::create_dir_all(&shell_dir) {
        eprintln!("warning: could not create shell dir: {e}");
        return;
    }

    let bash_path = shell_dir.join("diegops.bash");
    migrate_personal_aliases(home, "bash");
    if let Err(e) = fs::write(&bash_path, BASH_CONFIG) {
        eprintln!("warning: could not write diegops.bash: {e}");
        return;
    }

    match inject_source_line(&bashrc, BASH_SOURCE_MARKER, BASH_SOURCE_LINE) {
        Ok(()) => {}
        Err(e) => eprintln!("warning: could not inject bash source line: {e}"),
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

/// Runs a command, capturing stderr for error reporting.
///
/// On success the command's output is suppressed (progress messages are handled
/// by the caller via `eprintln!`). On failure a structured error is returned
/// containing the full command line, exit code, and any captured stderr so the
/// user has enough information to debug or report the issue.
fn run_cmd(program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("failed to spawn `{program}`: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let full_cmd = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");

    let code = output
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "killed by signal".to_string());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();

    if stderr.is_empty() {
        Err(format!(
            "`{full_cmd}` failed (exit code {code})\n\
             No output was captured. Try running the command manually for details."
        )
        .into())
    } else {
        // Indent each line of stderr for readable, copy-pasteable output.
        let indented = stderr
            .lines()
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        Err(format!("`{full_cmd}` failed (exit code {code})\n{indented}").into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn managed_configs_have_no_personal_aliases() {
        for c in [super::ZSH_CONFIG, super::BASH_CONFIG] {
            assert!(!c.contains("--dangerously"), "unsafe alias");
            assert!(!c.contains("code-insiders"), "personal alias");
            assert!(
                c.contains(".diegops/shell/aliases."),
                "no personal aliases hook"
            );
        }
    }

    #[test]
    fn old_managed_aliases_are_carried_to_the_personal_file() {
        let old = "alias code='code-insiders'\nalias cc='claude --dangerously-skip-permissions'\nalias k='kubectl'\n";
        let kept = super::carry_over_aliases(old).expect("migrated");
        assert!(kept.contains("alias code='code-insiders'"));
        assert!(kept.contains("alias cc='claude --dangerously-skip-permissions'"));
        assert!(!kept.contains("alias k="));
        assert!(super::carry_over_aliases(super::ZSH_CONFIG).is_none());
        assert!(super::carry_over_aliases("").is_none());
    }

    #[test]
    fn migration_writes_aliases_once() {
        let home = std::env::temp_dir().join(format!("diegops-alias-{}", std::process::id()));
        let dir = home.join(".diegops/shell");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("diegops.zsh"),
            "alias cc='claude --dangerously-skip-permissions'\n",
        )
        .unwrap();
        super::migrate_personal_aliases(&home, "zsh");
        let a = std::fs::read_to_string(dir.join("aliases.zsh")).unwrap();
        assert!(a.contains("alias cc="));
        std::fs::write(dir.join("aliases.zsh"), "# mine\n").unwrap();
        super::migrate_personal_aliases(&home, "zsh");
        assert_eq!(
            std::fs::read_to_string(dir.join("aliases.zsh")).unwrap(),
            "# mine\n"
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn managed_configs_load_completions() {
        assert!(super::ZSH_CONFIG.contains("diegops completion zsh"));
        assert!(super::BASH_CONFIG.contains("diegops completion bash"));
    }

    use super::*;

    #[test]
    fn shell_configs_do_not_hard_code_a_vault_address() {
        for cfg in [ZSH_CONFIG, BASH_CONFIG] {
            assert!(
                !cfg.contains("VAULT_ADDR"),
                "backend address belongs in config.toml"
            );
        }
    }
}
