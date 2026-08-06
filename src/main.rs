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
#[command(version = "0.6.3")]
#[command(about = "Unified Antigravity CLI & Account Manager", long_about = None)]
struct Cli {
    /// Account email or query to switch to directly
    account: Option<String>,

    /// Bypass quota cache and fetch live quota from CloudCode API
    #[arg(short = 'n', long = "no-cache", global = true)]
    no_cache: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Save active token from keyring as a saved account profile
    Save,
    /// Back up active account and prepare a fresh session to log in to a new account
    #[command(alias = "add")]
    New,
    /// List all saved Antigravity accounts with model quota
    List {
        /// Bypass quota cache and fetch live quota from CloudCode API
        #[arg(short = 'n', long = "no-cache")]
        no_cache: bool,
    },
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
                println!("{} Saved current Antigravity account as '{}'", "✔".green().bold(), email.bold().cyan());
            } else {
                println!("{}", "✘ No active token found in keyring to save.".red());
            }
        }
        Some(Commands::New) => prepare_new_session()?,
        Some(Commands::List { no_cache }) => list_all_accounts(cli.no_cache || no_cache)?,
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "agym", &mut io::stdout());
        }
        None => ui::run_accounts_tui(cli.no_cache)?,
    }

    Ok(())
}
