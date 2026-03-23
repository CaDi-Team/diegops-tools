mod commands;

use clap::{CommandFactory, Parser, Subcommand};

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
