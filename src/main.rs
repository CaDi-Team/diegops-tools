mod commands;

use clap::{CommandFactory, Parser, Subcommand};
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
    version,
    about = "diegops — personal productivity CLI for humans and containers",
    long_about = "diegops — personal productivity CLI for humans and containers\n\n\
        Workspace management, secret injection, cloud config sync,\n\
        managed DevOps toolbox, and developer workstation setup\n\
        in a single static binary.",
    after_long_help = "TOOL PASSTHROUGH:\n  \
        Run managed tools directly: diegops <tool> <args>\n  \
        Example: diegops gh pr list, diegops jq '.name' file.json\n  \
        Available: gh, vault, terraform, helm, k9s, kubectl, jq, yq, trivy, trip\n\n\
        Run 'diegops <command> --help' for details on any command.",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print version information
    Version,
    /// Show help and available commands
    Help,
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
            Verifies GitHub and Vault authentication, then runs:\n  \
            sync pull → repo apply → vault apply → secrets pull\n\n\
            Finishes with a summary report and the hero banner.\n\
            Requires: GitHub token and Vault session.")]
    Bootstrap,
    /// Set up shell environment (zsh, oh-my-zsh, plugins, config)
    #[command(long_about = "Set up shell environment — zsh, oh-my-zsh, plugins, and config.\n\n\
            Installs zsh if missing, sets it as default shell, installs oh-my-zsh\n\
            and plugins, writes a managed shell config, and injects source lines.\n\
            Idempotent — safe to run repeatedly.")]
    Shell {
        #[command(subcommand)]
        cmd: ShellCommand,
    },
    /// Manage Vault secrets — pull secrets and write .env files
    #[command(
        long_about = "Manage Vault secrets — pull secrets and write .env files.\n\n\
        Config: ~/.diegops/repo-vault.yaml (override with --config or $DIEGOPS_VAULT_CONFIG)\n\
        Requires: vault CLI on PATH, VAULT_ADDR set, authenticated session.\n\n\
        Quick start:\n  \
        diegops vault init    # create sample config\n  \
        diegops vault apply   # pull secrets and write .env files"
    )]
    Vault {
        #[command(subcommand)]
        cmd: VaultCommand,
    },
    /// Manage secret files — push, pull, and status
    #[command(
        long_about = "Manage secret files — push, pull, and check sync status.\n\n\
        Config: ~/.diegops/secrets.yaml (override with --config or $DIEGOPS_SECRETS_CONFIG)\n\n\
        Quick start:\n  \
        diegops secrets init    # create sample config\n  \
        diegops secrets push    # push secret files to destinations\n  \
        diegops secrets status  # show sync state"
    )]
    Secrets {
        #[command(subcommand)]
        cmd: SecretsCommand,
    },
    /// Manage authentication tokens (GitHub, kenv)
    #[command(long_about = "Manage authentication tokens for external services.\n\n\
        Storage: ~/.diegops/tokens/ (0600 permissions on Unix)\n\
        Resolution: stored file > $GITHUB_TOKEN env var > unauthenticated\n\n\
        Quick start:\n  \
        diegops auth gh login <PAT>   # store GitHub token\n  \
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
        Available: gh, vault, terraform, helm, k9s, kubectl, jq, yq, trivy, trippy\n\n\
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
    /// Run a managed tool directly (gh, vault, terraform, helm, k9s, kubectl, jq, yq, trivy, trip)
    #[command(external_subcommand)]
    External(Vec<String>),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Version) => {
            println!("diegops v{}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Help) | None => {
            Cli::command().print_long_help()?;
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
        Some(Commands::Shell { cmd }) => match cmd {
            ShellCommand::Init => {
                commands::shell::init()?;
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
        Some(Commands::Vault { cmd }) => {
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
        }
        Some(Commands::Secrets { cmd }) => {
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
            }
        }
        Some(Commands::Auth { cmd }) => match cmd {
            AuthCommand::Gh { cmd: gh_cmd } => match gh_cmd {
                GhCommand::Login { token } => {
                    commands::auth::gh_login(&token)?;
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
                    eprintln!("Unknown command: {name}");
                    eprintln!("Run 'diegops help' for available commands.");
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_string_is_not_empty() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
