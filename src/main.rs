mod account;
mod quota;
mod session;

use colored::*;
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_version() {
    println!("agym v{}", VERSION);
}

fn print_help() {
    println!("{} v{} - Unified Antigravity CLI Manager", "agym".purple().bold(), VERSION);
    println!("\n{}", "Usage:".bold());
    println!("  agym [command] [options]");
    println!("\n{}", "Commands:".bold());
    println!("  (no args), sessions, s    Interactive fzf session picker");
    println!("  accounts, acc, a          Interactive fzf account switcher");
    println!("  switch <email>            Switch active account by email/query");
    println!("  stats, status, q          Show account quota & model reset times");
    println!("    --verbose               Show all model quotas");
    println!("  list                      List all saved accounts");
    println!("  save                      Save active token from keyring");
    println!("  version, -v, --version    Show agym version");
    println!("  help, -h, --help          Show this help message");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        session::pick_session();
        return;
    }

    let cmd = args[1].to_lowercase();

    match cmd.as_str() {
        "sessions" | "session" | "s" | "ls" => {
            session::pick_session();
        }
        "accounts" | "account" | "acc" | "sw" | "switch" | "use" | "a" => {
            if args.len() > 2 {
                account::set_active_account(&args[2]);
            } else {
                account::interactive_switch();
            }
        }
        "stats" | "status" | "quota" | "info" | "q" => {
            let verbose = args.iter().any(|a| a == "--verbose");
            quota::show_stats(verbose);
        }
        "list" => {
            account::list_accounts();
        }
        "save" => {
            if let Some(email) = account::save_current_account() {
                println!("Saved account: {}", email.cyan());
            } else {
                println!("{}", "No active account token found to save.".red());
            }
        }
        "version" | "-v" | "--version" | "-version" => {
            print_version();
        }
        "help" | "-h" | "--help" => {
            print_help();
        }
        _ => {
            account::set_active_account(&args[1]);
        }
    }
}
