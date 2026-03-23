mod commands;

use clap::{CommandFactory, Parser, Subcommand};
use commands::repo::RepoCommand;

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
