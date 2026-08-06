mod account;
mod quota;
mod session;

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
#[command(version = "0.3.0")]
#[command(about = "Unified Antigravity CLI & Account Manager", long_about = None)]
struct Cli {
    /// Bypass quota cache and fetch live quota from CloudCode API
    #[arg(short = 'n', long = "no-cache", global = true)]
    no_cache: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Switch to an Antigravity account interactively or by email/query
    #[command(alias = "acc")]
    Switch {
        /// Account email or query to switch to
        account: Option<String>,
    },
    /// Save active token from keyring as a saved account profile
    Save,
    /// Back up active account and prepare a fresh session to log in to a new account
    #[command(alias = "add")]
    New,
    /// Pick and resume a previous Antigravity chat session
    #[command(alias = "s")]
    Sessions,
    /// List all saved Antigravity accounts with model quota
    List {
        /// Bypass quota cache and fetch live quota from CloudCode API
        #[arg(short = 'n', long = "no-cache")]
        no_cache: bool,
    },
    /// Remove a saved Antigravity account
    Remove {
        /// Account email to remove
        account: String,
    },
    /// Generate shell autocompletion scripts (bash, zsh, fish, powershell, elvish)
    Completions {
        /// Target shell for autocompletions
        shell: Shell,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Switch { account }) => match account {
            Some(name) => {
                set_active_account(&name);
            }
            None => {
                interactive_switch(cli.no_cache)?;
            }
        },
        Some(Commands::Save) => {
            if let Some(email) = save_current_account() {
                println!("{} Saved current Antigravity account as '{}'", "✔".green().bold(), email.bold().cyan());
            } else {
                println!("{}", "✘ No active token found in keyring to save.".red());
            }
        }
        Some(Commands::New) => prepare_new_session()?,
        Some(Commands::Sessions) => pick_and_resume_session()?,
        Some(Commands::List { no_cache }) => list_all_accounts(cli.no_cache || no_cache)?,
        Some(Commands::Remove { account }) => remove_account(&account)?,
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "agym", &mut io::stdout());
        }
        None => interactive_switch(cli.no_cache)?,
    }

    Ok(())
}
