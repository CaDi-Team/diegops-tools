mod commands;

use clap::{CommandFactory, Parser, Subcommand};
use commands::auth::{AuthCommand, GhCommand};
use commands::devtools::{DevtoolsCommand, GitCommand, GpgCommand, SshCommand};
use commands::repo::RepoCommand;
use commands::sync::SyncCommand;
use commands::tool::ToolCommand;
use commands::vault::VaultCommand;

#[derive(Parser)]
#[command(
    name = "diegops",
    version,
    about = "diegops — personal productivity CLI for humans and containers",
    long_about = None,
    // Disable the auto-generated help subcommand so we can define our own
    disable_help_subcommand = true,
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
    Update,
    /// Manage repository workspace — clone, list, and diff repos from a config file
    Repo {
        #[command(subcommand)]
        cmd: RepoCommand,
    },
    /// Show the DiegOps hero screen
    Cadi,
    /// Manage Vault secrets — pull secrets and write .env files from a config file
    Vault {
        #[command(subcommand)]
        cmd: VaultCommand,
    },
    /// Manage authentication tokens for external services
    Auth {
        #[command(subcommand)]
        cmd: AuthCommand,
    },
    /// Developer tools — GPG, SSH, and git identity setup
    Devtools {
        #[command(subcommand)]
        cmd: DevtoolsCommand,
    },
    /// Sync config files to GitHub
    Sync {
        #[command(subcommand)]
        cmd: SyncCommand,
    },
    /// Manage DevOps CLI tools (gh, vault, terraform, helm, k9s, jq, yq, trivy, trippy, kubectl)
    Tool {
        #[command(subcommand)]
        cmd: ToolCommand,
    },
    /// Manage and run ktool (karluiz tools)
    Ktool {
        /// Arguments passed to ktool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run a managed tool directly (gh, vault, terraform, helm, k9s, jq, yq, trivy, trip, kubectl)
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
