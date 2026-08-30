//! Modular account management subsystem for Antigravity profiles.
//!
//! Provides credential persistence, system keyring integration, Google OAuth token parsing,
//! and CLI / TUI interactive account switching.

pub mod auth;
pub mod keyring;
pub mod store;

// Public re-exports for backward compatibility across the codebase
pub use keyring::clear_keyring_token;
pub use store::{
    list_account_infos, list_account_infos_cached, remove_account, save_current_account,
    set_active_account, AccountInfo,
};

use anyhow::Result;
use colored::Colorize;
use inquire::Select;

/// Lists all saved accounts with their active status and quota badges in CLI output.
pub fn list_all_accounts(no_cache: bool) -> Result<()> {
    if no_cache {
        println!(
            "{}",
            "⏳ Fetching live account quotas from CloudCode API...".yellow()
        );
    }

    let accounts = list_account_infos(no_cache);

    if accounts.is_empty() {
        println!("{}", "No saved accounts.".yellow());
        return Ok(());
    }

    println!("{}", "Saved Antigravity Accounts:".bold().underline());
    let mut latest_fetch_time: Option<String> = None;
    let mut is_any_fresh = false;

    for acc in &accounts {
        let quota_badge = acc
            .quota
            .as_ref()
            .map(|q| {
                if q.is_fresh {
                    is_any_fresh = true;
                }
                if latest_fetch_time.is_none() || q.is_fresh {
                    latest_fetch_time = Some(q.formatted_time());
                }
                q.display_badge().cyan().to_string()
            })
            .unwrap_or_else(|| "[quota unavailable]".dimmed().to_string());

        if acc.is_active {
            println!(
                "  {} {} {} {}",
                "*".green().bold(),
                acc.email.bold().magenta(),
                "(active)".green(),
                quota_badge
            );
        } else {
            println!("    {} {}", acc.email, quota_badge);
        }
    }

    if let Some(timestamp) = latest_fetch_time {
        println!();
        if is_any_fresh {
            println!(
                "{} Quota data: {} • Updated: {}",
                "ℹ".blue().bold(),
                "fresh (live)".green().bold(),
                timestamp.bold()
            );
        } else {
            println!(
                "{} Quota data: {} • Last updated: {}",
                "ℹ".blue().bold(),
                "cached (5-min TTL)".yellow(),
                timestamp.dimmed()
            );
        }
    }

    Ok(())
}

/// Launches an interactive selection prompt allowing the user to delete a saved account profile.
pub fn interactive_remove_account() -> Result<()> {
    let accounts = list_account_infos(false);
    if accounts.is_empty() {
        println!("{}", "No saved accounts to remove.".yellow());
        return Ok(());
    }

    let options: Vec<String> = accounts.iter().map(|acc| acc.email.clone()).collect();
    let ans = Select::new("🗑️ Select Account to Delete:", options).prompt();

    match ans {
        Ok(selected) => {
            remove_account(&selected)?;
        }
        Err(_) => {
            println!("Operation cancelled.");
        }
    }

    Ok(())
}

/// Clears the active credentials in the system keyring to prepare for logging into a new account.
///
/// Backs up the current active account before wiping the keyring so no credentials are lost.
pub fn prepare_new_session() -> Result<()> {
    let _ = save_current_account();
    clear_keyring_token()?;

    println!("{} Prepared fresh login session.", "✨".bold());
    println!(
        "👉 Run {} to log in to your new account.",
        "agy".bold().yellow()
    );
    println!(
        "👉 Run {} (or {}) when done to save it!",
        "agym save".bold().cyan(),
        "agym".bold().cyan()
    );

    Ok(())
}
