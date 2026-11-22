mod commands;
mod examples;
mod help;
mod secrets_backend;
mod ui;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use commands::all::AllCommand;
use commands::auth::{AuthCommand, GhCommand};
use commands::devtools::{DevtoolsCommand, GitCommand, GpgCommand, SshCommand};
use commands::repo::RepoCommand;
use commands::secrets::SecretsCommand;
use commands::shell::ShellCommand;
use commands::sync::SyncCommand;
use commands::tool::ToolCommand;
use commands::vault::VaultCommand;

#[derive(Parser)]
#[command(
    name = "diegops",
    version = concat!("v", env!("CARGO_PKG_VERSION")),
    about = "personal productivity CLI for humans and containers",
    long_about = "diegops — personal productivity CLI for humans and containers\n\n\
        Workspace management, secret injection, cloud config sync,\n\
        managed DevOps toolbox, and developer workstation setup\n\
        in a single static binary.",
    after_long_help = "TOOL PASSTHROUGH:\n  \
        Run managed tools directly: diegops <tool> <args>\n  \
        Example: diegops gh pr list, diegops jq '.name' file.json\n  \
        Available: gh, vault, bao, terraform, helm, k9s, kubectl, jq, yq, trivy, trip\n\n\
        Run 'diegops help <command>' or 'diegops <command> --help' for details.\n\n\
        EXIT CODES:\n  \
        0  success\n  \
        1  the command failed (message starts with 'error:')\n  \
        2  usage error: unknown command, bad flag or argument",
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Accept defaults and confirmations without asking
    #[arg(short = 'y', long, global = true)]
    yes: bool,
    /// Never prompt; fail (exit 2) if input is needed. Also: $CI, $DIEGOPS_NO_INPUT
    #[arg(long, global = true, alias = "non-interactive")]
    no_input: bool,
    /// Show what would change and change nothing (commands that write)
    #[arg(short = 'n', long, global = true)]
    dry_run: bool,
    /// Hide progress lines (errors, warnings and results still print)
    #[arg(short = 'q', long, global = true, conflicts_with = "verbose")]
    quiet: bool,
    /// Explain where config, backend and credentials come from
    #[arg(short = 'v', long, global = true)]
    verbose: bool,
    /// Machine-readable output (read commands only; schema in the README)
    #[arg(long, global = true)]
    json: bool,
    /// Secrets backend: infisical | vault (alias openbao). Overrides $DIEGOPS_SECRETS_BACKEND
    #[arg(long, global = true, value_name = "NAME")]
    backend: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print version information
    Version,
    /// First-run setup wizard: backend, credentials, GitHub token, then doctor
    #[command(long_about = "First-run setup wizard.\n\n\
        Steps: secrets backend → its credentials (validated right away) → GitHub\n\
        token (validated with GET /user) → review → write → 'diegops doctor'.\n\
        Steps that already work are shown as ✓ and skipped; --reset STEP redoes one.\n\
        Every prompt has a flag/env var, so --no-input (or CI) runs the same path.\n\
        Secrets are never taken as flag values: the Client Secret comes from\n\
        $INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET, --client-secret-file or a hidden prompt.\n\
        --dry-run stops at the review screen.")]
    Init {
        /// Infisical URL (default Infisical Cloud US)
        #[arg(long, env = "INFISICAL_API_URL", value_name = "URL")]
        site_url: Option<String>,
        /// Infisical project ID
        #[arg(long, env = "DIEGOPS_INFISICAL_PROJECT_ID", value_name = "ID")]
        project_id: Option<String>,
        /// Machine identity Client ID (Universal Auth)
        #[arg(long, env = "INFISICAL_UNIVERSAL_AUTH_CLIENT_ID", value_name = "ID")]
        client_id: Option<String>,
        /// Existing file (mode 600) holding the Client Secret
        #[arg(
            long,
            env = "DIEGOPS_INFISICAL_CLIENT_SECRET_FILE",
            value_name = "PATH"
        )]
        client_secret_file: Option<std::path::PathBuf>,
        /// Vault/OpenBao address (token: $VAULT_TOKEN or 'bao login')
        #[arg(long, env = "VAULT_ADDR", value_name = "URL")]
        vault_addr: Option<String>,
        /// Do not set up a GitHub token
        #[arg(long, env = "DIEGOPS_SKIP_GITHUB")]
        skip_github: bool,
        /// Redo a step that already works: backend, github, all
        #[arg(long, value_name = "STEP", value_parser = ["backend", "github", "all"])]
        reset: Vec<String>,
    },
    /// Check this machine's setup and show how to fix each problem
    #[command(
        long_about = "Check this machine's setup and show how to fix each problem.\n\n\
        Checks: the diegops binary, config files, the secrets backend (config,\n\
        credentials, client secret file permissions, a test call), the GitHub token\n\
        and sync repo, and managed tools on PATH.\n\
        ✓ ok   ! warning (optional or risky)   ✗ problem   - skipped\n\
        Exits 1 if any ✗. Nothing is changed."
    )]
    Doctor {
        /// Do not contact the backend or GitHub
        #[arg(long)]
        offline: bool,
    },
    /// Explain a term in plain words (backend, infisical, pat, …)
    Explain {
        /// Term to explain; omit to list all terms
        term: Vec<String>,
    },
    /// Print a shell completion script (bash, zsh, fish, powershell)
    #[command(long_about = "Print a shell completion script to stdout.\n\n\
        'diegops shell init' already loads it for zsh and bash. To load it by hand,\n\
        add the matching line to your shell rc file.")]
    Completion {
        /// Shell to generate for
        shell: clap_complete::Shell,
    },
    /// Print the man page (roff) to stdout
    #[command(hide = true)]
    Man,
    /// Show help for diegops or for a command (e.g. diegops help repo apply)
    Help {
        /// Command path, e.g. `repo apply`
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Update diegops to the latest released version
    #[command(long_about = "Update diegops to the latest released version.\n\n\
        Downloads the latest release from GitHub, replaces the current binary\n\
        in place. Idempotent — skips if already up to date.\n\n\
        If installed in a system path, you may need: sudo diegops update")]
    Update,
    /// Manage git repository workspace
    #[command(
        long_about = "Manage git repository workspace — clone, list, and diff repos.\n\n\
        Config: ~/.diegops/repos.yaml (override with --config or $DIEGOPS_REPOS_CONFIG)\n\n\
        Quick start:\n  \
        diegops repo init       # create sample config\n  \
        diegops repo apply      # clone all missing repos\n  \
        diegops repo list-diff  # see what's missing"
    )]
    Repo {
        #[command(subcommand)]
        cmd: RepoCommand,
    },
    /// Show the DiegOps hero screen
    Cadi,
    /// Bootstrap a fresh workstation in one shot
    #[command(long_about = "Bootstrap a fresh workstation in one shot.\n\n\
            Runs 'diegops all pull' (sync pull → SSH keys → repo apply →\n\
            .env injection → secrets pull → agents restore → shell init),\n\
            verifying GitHub and the secrets backend first.\n\n\
            Finishes with a summary report and the hero banner.\n\
            Requires: GitHub token and secrets-backend credentials.")]
    Bootstrap,
    /// Push or pull everything: repos, secrets, agent context, and cloud config
    #[command(long_about = "Push or pull everything in one command.\n\n\
            all pull: sync pull → SSH keys → repo apply → .env injection → secrets pull →\n\
                      agents restore (missing profiles only) → shell init\n\
            all push: secrets push → agents backup → sync push\n\n\
            Continues past a failed step and prints a summary at the end.\n\
            Requires: GitHub token and secrets-backend credentials (for pull).")]
    All {
        #[command(subcommand)]
        cmd: AllCommand,
    },
    /// Set up shell environment (zsh, oh-my-zsh, plugins, config)
    #[command(
        long_about = "Set up shell environment — zsh, oh-my-zsh, plugins, and config.\n\n\
            Installs zsh if missing, sets it as default shell, installs oh-my-zsh\n\
            and plugins, writes a managed shell config, and injects source lines.\n\
            Idempotent — safe to run repeatedly."
    )]
    Shell {
        #[command(subcommand)]
        cmd: ShellCommand,
    },
    /// Project .env files: pull values from the secrets backend and write them
    #[command(
        long_about = "Project .env files — pull values from the secrets backend and write them.\n\
        (Formerly 'diegops vault'; the old name still works.)\n\n\
        Config: ~/.diegops/repo-vault.yaml (override with --config or $DIEGOPS_VAULT_CONFIG)\n\
        Requires: a configured secrets backend (Infisical default, or Vault/OpenBao)\n\
        in ~/.config/diegops/config.toml — see https://github.com/CaDi-Team/diegops-tools/blob/develop/docs/secrets-backends.md.\n\n\
        Quick start:\n  \
        diegops env init      # create sample config\n  \
        diegops env apply     # pull secrets and write .env files"
    )]
    Env {
        #[command(subcommand)]
        cmd: VaultCommand,
    },
    /// Old name of `env`
    #[command(hide = true)]
    Vault {
        #[command(subcommand)]
        cmd: VaultCommand,
    },
    /// Read/write secrets in the configured backend (Infisical default, Vault/OpenBao)
    #[command(long_about = "Read and write secrets in the configured backend.\n\n\
        Backend: --backend > $DIEGOPS_SECRETS_BACKEND > `backend` in\n\
        ~/.config/diegops/config.toml > infisical (Infisical Cloud).\n\
        Paths are <mount>/<path> as in Vault KV v2. See https://github.com/CaDi-Team/diegops-tools/blob/develop/docs/secrets-backends.md.")]
    Kv {
        #[command(subcommand)]
        cmd: commands::kv::KvCommand,
    },
    /// Back up and restore local AI agent context (diegops agents hermes …)
    #[command(long_about = "Back up and restore local AI agent context.\n\n\
        Name the agent explicitly: diegops agents hermes backup|restore|status|memory|secrets.\n\
        Group views: diegops agents status | install-timer | init.\n\
        Registry: ~/.diegops/agents.yaml ($DIEGOPS_AGENTS_CONFIG). Conversations,\n\
        state.db, .env and tokens are never written to git.")]
    Agents {
        #[command(subcommand)]
        cmd: commands::agents::AgentsCommand,
    },
    /// Secret files (~/.ssh, ~/.kube, …): push, pull and status
    #[command(
        long_about = "Secret files (~/.ssh, ~/.kube, …) — push, pull, and check sync status.\n\
        (Formerly 'diegops secrets'; the old name still works.)\n\n\
        Config: ~/.diegops/secrets.yaml (override with --config or $DIEGOPS_SECRETS_CONFIG)\n\n\
        Quick start:\n  \
        diegops files init      # create sample config\n  \
        diegops files push      # push secret files to destinations\n  \
        diegops files status    # show sync state"
    )]
    Files {
        #[command(subcommand)]
        cmd: SecretsCommand,
    },
    /// Old name of `files`
    #[command(hide = true)]
    Secrets {
        #[command(subcommand)]
        cmd: SecretsCommand,
    },
    /// Manage authentication tokens (GitHub, kenv)
    #[command(long_about = "Manage authentication tokens for external services.\n\n\
        Storage: ~/.diegops/tokens/ (0600 permissions on Unix)\n\
        Resolution: $GITHUB_TOKEN > $GH_TOKEN > stored token > `gh auth token`\n\n\
        Quick start:\n  \
        diegops auth gh login         # store GitHub token\n  \
        diegops auth status           # check all providers")]
    Auth {
        #[command(subcommand)]
        cmd: AuthCommand,
    },
    /// Developer workstation setup (git, gpg, ssh)
    #[command(
        long_about = "Developer workstation setup — git identity, GPG signing, SSH keys.\n\n\
        Identity resolution: --name/--email flags > git config > GitHub API (gh)\n\n\
        Quick start:\n  \
        diegops devtools git set              # set git identity\n  \
        diegops devtools gpg set              # full GPG setup\n  \
        diegops devtools ssh create --name github --type ed25519"
    )]
    Devtools {
        #[command(subcommand)]
        cmd: DevtoolsCommand,
    },
    /// Cloud-sync config files to a private GitHub repo
    #[command(
        long_about = "Cloud-sync ~/.diegops/ config files to a private GitHub repo.\n\n\
        Auto-creates repo: diegops-{username}-memory (private)\n\
        Set $DIEGOPS_PROFILE (e.g. work) to use diegops-{username}-{profile}-memory\n\
        Syncs everything except tokens/ and bin/\n\
        Requires: diegops auth gh login first\n\n\
        Usage:\n  \
        diegops sync push     # upload local configs to GitHub\n  \
        diegops sync pull     # download configs from GitHub\n  \
        diegops sync status   # show diff between local and cloud"
    )]
    Sync {
        #[command(subcommand)]
        cmd: SyncCommand,
    },
    /// Manage DevOps CLI tools — install, update, remove
    #[command(
        long_about = "Manage DevOps CLI tools — install, update, and remove.\n\n\
        All binaries stored in ~/.diegops/bin/\n\
        Available: gh, vault, bao, terraform, helm, k9s, kubectl, jq, yq, trivy, trippy\n\n\
        Usage:\n  \
        diegops tool list              # show available tools\n  \
        diegops tool install <name>    # install a tool\n  \
        diegops tool update            # update all installed tools\n  \
        diegops tool remove <name>     # remove a tool\n\n\
        Then use directly: diegops <tool> <args>\n  \
        Example: diegops gh pr list, diegops jq '.name' file.json"
    )]
    Tool {
        #[command(subcommand)]
        cmd: ToolCommand,
    },
    /// Manage and run ktool (karluiz tools)
    #[command(
        long_about = "Manage and run ktool (karluiz tools) as a sidecar binary.\n\n\
        Usage:\n  \
        diegops ktool update              # install/update ktool\n  \
        diegops ktool <args>              # forward to ktool\n\n\
        Examples:\n  \
        diegops ktool kenv list           # list kenv secrets\n  \
        diegops ktool auth kenv login T   # authenticate with kenv\n  \
        diegops ktool magic               # karluiz hero screen"
    )]
    Ktool {
        /// Arguments passed to ktool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run a managed tool directly (gh, vault, bao, terraform, helm, k9s, kubectl, jq, yq, trivy, trip)
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Rust ignores SIGPIPE, so `println!` panics when stdout is a closed pipe
/// (`diegops ... | head`). Restore the default so we exit quietly like other
/// Unix tools.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: called once at startup, before any threads are spawned.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn main() {
    reset_sigpipe();
    if let Err(e) = run() {
        eprintln!("{}", render_error(&e.to_string()));
        std::process::exit(1);
    }
}

/// `diegops env …` (formerly `vault`): project .env files.
fn run_env(cmd: VaultCommand) -> Result<(), Box<dyn std::error::Error>> {
    let config_path_str;
    match cmd {
        VaultCommand::Apply { path, config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::vault::apply(cfg, path.as_deref())?;
        }
        VaultCommand::List { config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::vault::list(cfg)?;
        }
        VaultCommand::ListDiff { config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::vault::list_diff(cfg)?;
        }
        VaultCommand::Init => {
            commands::vault::init(None)?;
        }
    }
    Ok(())
}

/// `diegops files …` (formerly `secrets`): secret files like ~/.ssh.
fn run_files(cmd: SecretsCommand) -> Result<(), Box<dyn std::error::Error>> {
    let config_path_str;
    match cmd {
        SecretsCommand::Push { path, config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::secrets::push(cfg, path.as_deref())?;
        }
        SecretsCommand::Pull { path, config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::secrets::pull(cfg, path.as_deref())?;
        }
        SecretsCommand::Status { config } => {
            config_path_str = config;
            let cfg = config_path_str.as_deref().map(std::path::Path::new);
            commands::secrets::status(cfg)?;
        }
        SecretsCommand::Init => {
            commands::secrets::init(None)?;
        }
        SecretsCommand::Update {
            repos_config,
            config,
        } => {
            let repos_cfg = repos_config.as_deref().map(std::path::Path::new);
            let secrets_cfg = config.as_deref().map(std::path::Path::new);
            commands::secrets::update(secrets_cfg, repos_cfg)?;
        }
    }
    Ok(())
}

/// Old command names kept as hidden aliases (ADR 0001 D1).
fn warn_renamed(old: &str, new: &str) {
    eprintln!("warning: 'diegops {old}' is now 'diegops {new}'; the old name will be removed in a future release");
}

/// Space-joined subcommand path, e.g. "env apply".
fn command_path(m: &clap::ArgMatches) -> String {
    let mut parts = Vec::new();
    let mut cur = m;
    while let Some((name, sub)) = cur.subcommand() {
        // Renamed commands (#48) share the rules of their new names.
        parts.push(
            match (parts.is_empty(), name) {
                (true, "vault") => "env",
                (true, "secrets") => "files",
                _ => name,
            }
            .to_string(),
        );
        cur = sub;
    }
    parts.join(" ")
}

/// Formats an error for humans: `error: <first line>` followed by any
/// continuation lines (hints) exactly as written, never Debug-quoted.
fn render_error(msg: &str) -> String {
    format!("error: {}", msg.trim_end())
}

/// The full command tree with examples and the ENVIRONMENT section attached.
pub(crate) fn cli_command() -> clap::Command {
    examples::apply(Cli::command())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cmd = cli_command();
    let cmd = cmd.clone().help_template(help::template(&cmd));
    let matches = cmd.get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let (yes, dry_run, backend) = (cli.yes, cli.dry_run, cli.backend.clone());
    ui::set(ui::Flags {
        yes,
        no_input: cli.no_input,
        dry_run,
        quiet: cli.quiet,
        verbose: cli.verbose,
        json: cli.json,
    });
    let path = command_path(&matches);
    let json_key = matches!(
        &cli.command,
        Some(Commands::Kv {
            cmd: commands::kv::KvCommand::Get { key: Some(_), .. }
        })
    );
    if cli.json && json_key {
        eprintln!("error: --json lists key names only; it never prints a value\n  hint: drop the KEY argument, or drop --json to read the value");
        std::process::exit(2);
    }
    if let Err(msg) = ui::check_json(cli.json, &path) {
        eprintln!("{msg}");
        std::process::exit(2);
    }
    if let Err(msg) = ui::check_dry_run(dry_run, &path) {
        eprintln!("{msg}");
        std::process::exit(2);
    }
    if let Some(b) = backend.as_deref() {
        // One place decides the backend for every command (ADR 0001 D4).
        let cfg = secrets_backend::config::Config::default();
        if let Err(e) = secrets_backend::config::select_kind(&cfg, Some(b), &|_| None) {
            eprintln!("error: {e}\n  hint: use --backend infisical or --backend vault");
            std::process::exit(2);
        }
        std::env::set_var("DIEGOPS_SECRETS_BACKEND", b);
    }
    match cli.command {
        Some(Commands::Init {
            site_url,
            project_id,
            client_id,
            client_secret_file,
            vault_addr,
            skip_github,
            reset,
        }) => {
            commands::init::run(commands::init::Given {
                backend: backend
                    .clone()
                    .or_else(|| std::env::var("DIEGOPS_SECRETS_BACKEND").ok()),
                site_url,
                project_id,
                client_id,
                client_secret_file,
                client_secret: std::env::var("INFISICAL_UNIVERSAL_AUTH_CLIENT_SECRET").ok(),
                vault_addr,
                skip_github,
                reset,
            })?;
        }
        Some(Commands::Doctor { offline }) => {
            commands::doctor::run(ui::flags().json, offline)?;
        }
        Some(Commands::Explain { term }) => {
            let t = term.join(" ");
            commands::explain::run((!t.is_empty()).then_some(t.as_str()))?;
        }
        Some(Commands::Completion { shell }) => {
            clap_complete::generate(shell, &mut cli_command(), "diegops", &mut std::io::stdout());
        }
        Some(Commands::Man) => {
            clap_mangen::Man::new(cli_command()).render(&mut std::io::stdout())?;
        }
        Some(Commands::Version) => {
            println!("diegops v{}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Help { command }) if !command.is_empty() => {
            let root = cli_command();
            if let Err((bad, names, help_cmd)) = help::check_path(&root, &command) {
                eprintln!("{}", help::unknown_command(&bad, &names, &help_cmd));
                std::process::exit(2);
            }
            let mut argv = vec!["diegops".to_string()];
            argv.extend(command);
            argv.push("--help".to_string());
            if let Err(e) = root.try_get_matches_from(argv) {
                e.exit();
            }
        }
        Some(Commands::Help { .. }) | None => {
            let root = cli_command();
            root.clone()
                .help_template(help::template(&root))
                .print_long_help()?;
            println!();
        }
        Some(Commands::Update) => {
            commands::update::run()?;
        }
        Some(Commands::Cadi) => {
            commands::cadi::run();
        }
        Some(Commands::Bootstrap) => {
            commands::bootstrap::run()?;
        }
        Some(Commands::All { cmd }) => match cmd {
            AllCommand::Pull => {
                let results = commands::all::pull()?;
                commands::all::finish(&results, "pull")?;
            }
            AllCommand::Push => {
                let results = commands::all::push()?;
                commands::all::finish(&results, "push")?;
            }
        },
        Some(Commands::Shell { cmd }) => match cmd {
            ShellCommand::Init => {
                commands::shell::init(dry_run, yes)?;
            }
        },
        Some(Commands::Repo { cmd }) => {
            let config_path_str;
            match cmd {
                RepoCommand::Apply { path, config } => {
                    config_path_str = config;
                    let cfg = config_path_str.as_deref().map(std::path::Path::new);
                    commands::repo::apply(cfg, path.as_deref())?;
                }
                RepoCommand::List { config } => {
                    config_path_str = config;
                    let cfg = config_path_str.as_deref().map(std::path::Path::new);
                    commands::repo::list(cfg)?;
                }
                RepoCommand::ListDiff { config } => {
                    config_path_str = config;
                    let cfg = config_path_str.as_deref().map(std::path::Path::new);
                    commands::repo::list_diff(cfg)?;
                }
                RepoCommand::Init => {
                    commands::repo::init()?;
                }
            }
        }
        Some(Commands::Env { cmd }) => run_env(cmd)?,
        Some(Commands::Vault { cmd }) => {
            warn_renamed("vault", "env");
            run_env(cmd)?
        }
        Some(Commands::Files { cmd }) => run_files(cmd)?,
        Some(Commands::Secrets { cmd }) => {
            warn_renamed("secrets", "files");
            run_files(cmd)?
        }
        Some(Commands::Agents { cmd }) => {
            commands::agents::run(cmd)?;
        }
        Some(Commands::Kv { cmd }) => {
            commands::kv::run(cmd, backend.as_deref())?;
        }
        Some(Commands::Auth { cmd }) => match cmd {
            AuthCommand::Gh { cmd: gh_cmd } => match gh_cmd {
                GhCommand::Login {
                    token,
                    with_token,
                    from_gh,
                } => {
                    commands::auth::gh_login_cmd(token, with_token, from_gh)?;
                }
                GhCommand::Logout => {
                    commands::auth::gh_logout()?;
                }
                GhCommand::Whoami => {
                    commands::auth::gh_whoami()?;
                }
            },
            AuthCommand::Status => {
                commands::auth::status()?;
            }
            AuthCommand::Logout => {
                commands::auth::logout_all()?;
            }
        },
        Some(Commands::Devtools { cmd }) => match cmd {
            DevtoolsCommand::Git { cmd: git_cmd } => match git_cmd {
                GitCommand::Set { name, email } => {
                    commands::devtools_git::set(name.as_deref(), email.as_deref())?;
                }
            },
            DevtoolsCommand::Gpg { cmd: gpg_cmd } => match gpg_cmd {
                GpgCommand::Init { name, email } => {
                    commands::devtools_gpg::init(name.as_deref(), email.as_deref())?;
                }
                GpgCommand::Set => {
                    commands::devtools_gpg::set()?;
                }
                GpgCommand::Restart => {
                    commands::devtools_gpg::restart()?;
                }
            },
            DevtoolsCommand::Ssh { cmd: ssh_cmd } => match ssh_cmd {
                SshCommand::List => {
                    commands::devtools_ssh::list()?;
                }
                SshCommand::Config => {
                    commands::devtools_ssh::config()?;
                }
                SshCommand::Create {
                    name,
                    r#type,
                    email,
                } => {
                    commands::devtools_ssh::create(name.as_deref(), &r#type, email.as_deref())?;
                }
            },
        },
        Some(Commands::Sync { cmd }) => match cmd {
            SyncCommand::Push => commands::sync::push()?,
            SyncCommand::Pull => commands::sync::pull()?,
            SyncCommand::Status => commands::sync::status()?,
        },
        Some(Commands::Tool { cmd }) => match cmd {
            ToolCommand::List => commands::tool::list()?,
            ToolCommand::Install { name } => commands::tool::install(&name)?,
            ToolCommand::Update { name } => commands::tool::update(name.as_deref())?,
            ToolCommand::Remove { name } => commands::tool::remove(&name)?,
        },
        Some(Commands::Ktool { args }) => {
            commands::ktool::run(&args)?;
        }
        Some(Commands::External(args)) => {
            if let Some(name) = args.first() {
                if commands::tool::is_known_tool(name) {
                    commands::tool::passthrough(name, &args[1..])?;
                } else {
                    let mut names = help::visible_names(&cli_command());
                    names.extend(commands::tool::tool_names());
                    eprintln!("{}", help::unknown_command(name, &names, "diegops --help"));
                    std::process::exit(2);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn render_error_prefixes_and_keeps_hint_lines() {
        let out = super::render_error("config not found: x\n  hint: create it\n");
        assert_eq!(out, "error: config not found: x\n  hint: create it");
    }

    #[test]
    fn version_string_is_not_empty() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
