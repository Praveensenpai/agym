mod account;
mod quota;
mod session;
mod ui;

use account::*;
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use colored::Colorize;
use session::*;
use std::io;

#[derive(Parser)]
#[command(name = "agym")]
#[command(author = "Praveensenpai")]
#[command(disable_version_flag = true)]
#[command(version = "0.6.11")]
#[command(about = "Unified Antigravity CLI & Account Manager", long_about = None)]
struct Cli {
    /// Show the application version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: bool,

    /// Account email or query to switch to directly
    account: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Save active token from keyring as a saved account profile
    Save,
    /// Generate shell autocompletion scripts (bash, zsh, fish, powershell, elvish)
    Completions {
        /// Target shell for autocompletions
        shell: Shell,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(target) = cli.account {
        set_active_account(&target);
        return Ok(());
    }

    match cli.command {
        Some(Commands::Save) => {
            if let Some(email) = save_current_account() {
                println!(
                    "{} Saved current Antigravity account as '{}'",
                    "✔".green().bold(),
                    email.bold().cyan()
                );
            } else {
                println!("{}", "✘ No active token found in keyring to save.".red());
            }
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "agym", &mut io::stdout());
        }
        None => ui::run_accounts_tui()?,
    }

    Ok(())
}
