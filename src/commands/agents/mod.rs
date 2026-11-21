//! `diegops agents <kind> <verb>` — back up and restore local AI agent
//! context (definition, reviewed memories; secrets via the secrets backend).
//!
//! Each agent is named explicitly (`agents hermes backup`); only views that
//! span agents live at group level (`agents status`, `agents install-timer`).
//! Registry: `~/.diegops/agents.yaml`. Only `hermes` is implemented.

pub mod gitrepo;
pub mod hermes;
pub mod scan;
pub mod support;

use std::path::{Path, PathBuf};

use clap::Subcommand;

use support::{Kind, Registry};

type BoxError = Box<dyn std::error::Error>;

/// `diegops agents ...`
#[derive(Subcommand)]
pub enum AgentsCommand {
    /// Hermes Agent (~/.hermes and ~/.hermes/profiles/*)
    Hermes {
        #[command(subcommand)]
        cmd: HermesCommand,
    },
    /// Last backup of every registered agent
    Status,
    /// Install a daily systemd user timer that backs up every registered agent
    InstallTimer,
    /// Write a sample ~/.diegops/agents.yaml
    Init,
}

/// `diegops agents hermes ...`
#[derive(Subcommand)]
pub enum HermesCommand {
    /// Back up profile definitions (SOUL, config, skills, hooks, cron) to git
    Backup {
        /// Only this profile (default: all)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Restore profiles from the backup repo and inject .env from the secrets backend
    Restore {
        /// Only this profile (default: all)
        #[arg(long)]
        profile: Option<String>,
        /// Restore under DIR/<profile> instead of the Hermes home (dry run)
        #[arg(long)]
        to: Option<PathBuf>,
        /// Overwrite existing profiles (replaced files kept as *.pre-restore)
        #[arg(long)]
        force: bool,
        /// Do not inject .env from the secrets backend
        #[arg(long)]
        no_secrets: bool,
    },
    /// Backup state and pending changes per profile
    Status,
    /// Review and push memories (MEMORY.md / USER.md); never automatic
    Memory {
        #[command(subcommand)]
        cmd: MemoryCommand,
    },
    /// Move profile .env files into the secrets backend
    Secrets {
        #[command(subcommand)]
        cmd: SecretsCommand,
    },
}

/// `diegops agents hermes memory ...`
#[derive(Subcommand)]
pub enum MemoryCommand {
    /// Show what changed in memories since the last push
    Review {
        /// Only this profile (default: all)
        #[arg(long)]
        profile: Option<String>,
    },
    /// Push memories after review
    Push {
        /// Only this profile (default: all)
        #[arg(long)]
        profile: Option<String>,
    },
}

/// `diegops agents hermes secrets ...`
#[derive(Subcommand)]
pub enum SecretsCommand {
    /// Upload each profile's .env to <secrets_path>/<profile> (values never printed)
    Push {
        /// Only this profile (default: all)
        #[arg(long)]
        profile: Option<String>,
    },
}

const SAMPLE: &str = "\
# diegops agents registry — see `diegops agents --help`.
repo:
  path: ~/agents-context          # local clone of a PRIVATE repo
  # url: git@github.com:you/agents-context.git
  # branch: main
# heartbeat_kv: me/agents/heartbeat#url  # optional ping URL (healthchecks.io, Grafana…)
agents:
  - kind: hermes
    root: ~/.hermes
    secrets_path: me/agents/hermes      # .env per profile at <secrets_path>/<profile>
    # secrets_backend: infisical        # infisical (default, free) | openbao | vault
";

/// Entry point.
pub fn run(cmd: AgentsCommand) -> Result<(), BoxError> {
    match cmd {
        AgentsCommand::Init => init(),
        AgentsCommand::Status => {
            let (_, reg) = Registry::load(None)?;
            let state = support::RunState::load()?;
            for a in &reg.agents {
                hermes::print_run(&state, a.kind.as_str());
            }
            Ok(())
        }
        AgentsCommand::InstallTimer => {
            let (_, reg) = Registry::load(None)?;
            install_timer(&reg)
        }
        AgentsCommand::Hermes { cmd } => {
            let (_, reg) = Registry::load(None)?;
            match cmd {
                HermesCommand::Backup { profile } => hermes::backup(&reg, profile.as_deref()),
                HermesCommand::Restore {
                    profile,
                    to,
                    force,
                    no_secrets,
                } => hermes::restore(
                    &reg,
                    &hermes::RestoreOpts {
                        profile: profile.as_deref(),
                        to: to.as_deref(),
                        force,
                        no_secrets,
                        missing_only: false,
                    },
                ),
                HermesCommand::Status => hermes::status(&reg),
                HermesCommand::Memory { cmd } => match cmd {
                    MemoryCommand::Review { profile } => {
                        hermes::memory_review(&reg, profile.as_deref())
                    }
                    MemoryCommand::Push { profile } => {
                        hermes::memory_push(&reg, profile.as_deref())
                    }
                },
                HermesCommand::Secrets { cmd } => match cmd {
                    SecretsCommand::Push { profile } => {
                        hermes::secrets_push(&reg, profile.as_deref())
                    }
                },
            }
        }
    }
}

fn init() -> Result<(), BoxError> {
    let p = super::common::diegops_dir()?.join("agents.yaml");
    if p.exists() {
        return Err(format!("{} already exists", p.display()).into());
    }
    std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(&p, SAMPLE)?;
    println!("wrote {}", p.display());
    Ok(())
}

fn not_implemented(k: Kind) -> BoxError {
    format!(
        "agent kind '{}' is registered but not implemented yet",
        k.as_str()
    )
    .into()
}

/// Backs up every registered agent (used by `all push`). No registry = no-op.
pub fn backup_registered() -> Result<(), BoxError> {
    let Some(reg) = load_optional()? else {
        eprintln!("(no ~/.diegops/agents.yaml — skipped)");
        return Ok(());
    };
    for a in &reg.agents {
        match a.kind {
            Kind::Hermes => hermes::backup(&reg, None)?,
            k => return Err(not_implemented(k)),
        }
    }
    Ok(())
}

/// Restores agents whose profiles are missing locally (used by `all pull`).
pub fn restore_missing() -> Result<(), BoxError> {
    let Some(reg) = load_optional()? else {
        eprintln!("(no ~/.diegops/agents.yaml — skipped)");
        return Ok(());
    };
    for a in &reg.agents {
        match a.kind {
            Kind::Hermes => hermes::restore(
                &reg,
                &hermes::RestoreOpts {
                    profile: None,
                    to: None,
                    force: false,
                    no_secrets: false,
                    missing_only: true,
                },
            )?,
            k => return Err(not_implemented(k)),
        }
    }
    Ok(())
}

fn load_optional() -> Result<Option<Registry>, BoxError> {
    let p = super::common::resolve_config_path(None, "DIEGOPS_AGENTS_CONFIG", "agents.yaml")?;
    if !p.exists() {
        return Ok(None);
    }
    Registry::load(Some(&p)).map(|(_, r)| Some(r))
}

fn install_timer(reg: &Registry) -> Result<(), BoxError> {
    let kinds: Vec<Kind> = reg.agents.iter().map(|a| a.kind).collect();
    if let Some(k) = kinds.iter().find(|k| **k != Kind::Hermes) {
        return Err(not_implemented(*k));
    }
    let exe = std::env::current_exe()?;
    let (svc, tmr) = support::render_units(&exe, &kinds);
    let dir = super::common::home_dir()?.join(".config/systemd/user");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{}.service", support::UNIT)), svc)?;
    std::fs::write(dir.join(format!("{}.timer", support::UNIT)), tmr)?;
    let timer = format!("{}.timer", support::UNIT);
    for args in [
        vec!["--user", "daemon-reload"],
        vec!["--user", "enable", "--now", timer.as_str()],
    ] {
        let st = std::process::Command::new("systemctl")
            .args(&args)
            .status()?;
        if !st.success() {
            return Err(format!("systemctl {} failed", args.join(" ")).into());
        }
    }
    println!(
        "installed {0}.timer (daily). Logs: journalctl --user -u {0}.service",
        support::UNIT
    );
    Ok(())
}
