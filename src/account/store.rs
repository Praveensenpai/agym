//! Account persistence, directory resolution, profile symlinks, and SQLite synchronization.
//!
//! This module manages files in `~/.gemini-accounts/`, profile storage in `~/.gemini-profiles/`,
//! synchronization with Antigravity SQLite state databases, and account listing/sorting.

use super::auth::extract_email_from_token_json;
use super::keyring::{get_current_keyring_token, write_keyring_token};
use crate::quota::{clear_quota_cache, fetch_quota_cached, load_quota_cache, AccountQuotaInfo};
use anyhow::{anyhow, bail, Result};
use colored::Colorize;
use rusqlite::Connection;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;

/// Subdirectory name under the user home directory where account token JSON files are stored.
pub const ACCOUNTS_DIR_NAME: &str = ".gemini-accounts";

/// Subdirectory name under the user home directory where individual account profiles reside.
pub const PROFILES_DIR_NAME: &str = ".gemini-profiles";

/// Relative path from user home to the Antigravity agent SQLite database.
pub const AGENT_DB_REL_PATH: &str = ".antigravity-agent/cloud_accounts.db";

/// Name of the primary Gemini configuration directory/symlink in user home.
pub const GEMINI_LINK_NAME: &str = ".gemini";

/// Expected file extension for credential files.
pub const JSON_EXTENSION: &str = "json";

/// Information about a saved Antigravity account and its quota status.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// Account email address identifying this profile.
    pub email: String,
    /// Whether this account is currently active in the OS keyring and symlink.
    pub is_active: bool,
    /// Cached or live quota metrics for Gemini and Claude models.
    pub quota: Option<AccountQuotaInfo>,
    /// Path to the `<email>.json` credential file.
    pub file_path: PathBuf,
}

/// Resolves the user home directory safely without hardcoded paths.
pub fn get_home() -> PathBuf {
    dirs::home_dir()
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Returns the path to the account credential storage directory (`~/.gemini-accounts/`),
/// creating it if it does not yet exist.
pub fn get_accounts_dir() -> PathBuf {
    let p = get_home().join(ACCOUNTS_DIR_NAME);
    let _ = fs::create_dir_all(&p);
    p
}

/// Returns the path to the account profiles directory (`~/.gemini-profiles/`),
/// creating it if it does not yet exist.
pub fn get_profiles_dir() -> PathBuf {
    let p = get_home().join(PROFILES_DIR_NAME);
    let _ = fs::create_dir_all(&p);
    p
}

/// Returns the path to the Antigravity agent SQLite database.
pub fn get_db_path() -> PathBuf {
    get_home().join(AGENT_DB_REL_PATH)
}

/// Returns the path to the `~/.gemini` profile symlink.
pub fn get_gemini_link() -> PathBuf {
    get_home().join(GEMINI_LINK_NAME)
}

/// Saves the current credential from the OS keyring as a named `<email>.json` profile.
///
/// Invalidates the quota cache upon a successful write so newly saved credentials
/// trigger refreshed metrics. Returns `Some(email)` on success, or `None` on failure.
pub fn save_current_account() -> Option<String> {
    let token_json = get_current_keyring_token()?;
    let email = extract_email_from_token_json(&token_json)?;

    let acc_dir = get_accounts_dir();
    let file_path = acc_dir.join(format!("{}.json", email));
    if let Ok(mut f) = File::create(&file_path) {
        let _ = f.write_all(token_json.as_bytes());
        clear_quota_cache();
        return Some(email);
    }
    None
}

/// Synchronizes the active account selection with Antigravity agent's local SQLite database.
pub fn update_sqlite_db(email: &str) -> Result<()> {
    let db_path = get_db_path();
    if db_path.exists() {
        let conn = Connection::open(&db_path)?;
        conn.execute("UPDATE accounts SET is_active = 0", [])?;
        let query = format!("%{}%", email);
        conn.execute(
            "UPDATE accounts SET is_active = 1 WHERE email LIKE ?1",
            [&query],
        )?;
    }
    Ok(())
}

/// Updates the `~/.gemini` profile symlink to point to the active account's profile directory.
pub fn update_gemini_profile(email: &str) -> Result<()> {
    let prefix = email.split('@').next().unwrap_or(email);
    let prof_dir = get_profiles_dir().join(prefix);
    fs::create_dir_all(&prof_dir)?;

    let gemini_link = get_gemini_link();

    if gemini_link.exists() || gemini_link.is_symlink() {
        if let Ok(meta) = fs::symlink_metadata(&gemini_link) {
            if meta.file_type().is_symlink() {
                let _ = fs::remove_file(&gemini_link);
            } else if meta.is_dir() {
                let backup = get_profiles_dir().join(format!("{}_backup", prefix));
                let _ = fs::rename(&gemini_link, backup);
            }
        }
    }

    unix_fs::symlink(&prof_dir, &gemini_link)?;
    Ok(())
}

/// Internal helper to calculate sorting priority for an account based on max quota percentage.
fn account_quota_priority(account: &AccountInfo) -> u32 {
    account
        .quota
        .as_ref()
        .map(|q| {
            q.gemini_percent
                .unwrap_or(0)
                .max(q.claude_percent.unwrap_or(0))
        })
        .unwrap_or(0)
}

/// Discovers saved accounts, fetches quota via the provided closure, and sorts by quota descending.
pub fn list_account_infos_with<F>(mut quota_for: F, detect_active: bool) -> Vec<AccountInfo>
where
    F: FnMut(&str, &PathBuf) -> Option<AccountQuotaInfo>,
{
    let current_email = if detect_active {
        let _ = save_current_account();
        let current_token = get_current_keyring_token();
        current_token
            .as_deref()
            .and_then(extract_email_from_token_json)
    } else {
        None
    };

    let acc_dir = get_accounts_dir();
    let mut accounts = Vec::new();

    if let Ok(entries) = fs::read_dir(acc_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension() == Some(std::ffi::OsStr::new(JSON_EXTENSION)) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem.starts_with('.') {
                        continue;
                    }
                    let is_active = Some(stem) == current_email.as_deref();
                    let quota = quota_for(stem, &path);
                    accounts.push(AccountInfo {
                        email: stem.to_string(),
                        is_active,
                        quota,
                        file_path: path,
                    });
                }
            }
        }
    }

    accounts.sort_by(|a, b| {
        let pct_a = account_quota_priority(a);
        let pct_b = account_quota_priority(b);
        pct_b.cmp(&pct_a).then_with(|| a.email.cmp(&b.email))
    });

    accounts
}

/// Returns all saved account profiles with quota metrics, optionally bypassing cache.
pub fn list_account_infos(no_cache: bool) -> Vec<AccountInfo> {
    list_account_infos_with(
        |stem, path| fetch_quota_cached(stem, path, no_cache).ok(),
        true,
    )
}

/// Loads only non-expired quota data from disk so the TUI can render immediately.
pub fn list_account_infos_cached() -> Vec<AccountInfo> {
    let cache = load_quota_cache();
    list_account_infos_with(
        |stem, _| cache.get(stem).filter(|quota| !quota.is_expired()).cloned(),
        false,
    )
}

/// Switches the active account in the system keyring, SQLite database, and profile symlink.
pub fn set_active_account(target: &str) -> Result<()> {
    let _ = save_current_account();
    let accounts = list_account_infos(false);

    let target_lc = target.to_lowercase();
    let matching = accounts
        .iter()
        .find(|acc| acc.email.to_lowercase().contains(&target_lc));

    let target_acc = match matching {
        Some(m) => m,
        None => {
            println!(
                "{}",
                format!("Error: Account matching '{}' not found.", target).red()
            );
            super::list_all_accounts(false).ok();
            bail!("Account matching '{}' not found", target);
        }
    };

    let token_json = fs::read_to_string(&target_acc.file_path).map_err(|e| {
        println!(
            "{}",
            format!("Error: Failed to read token file for {}", target_acc.email).red()
        );
        anyhow!("Failed to read token file for {}: {}", target_acc.email, e)
    })?;

    if write_keyring_token(&token_json) {
        update_sqlite_db(&target_acc.email)?;
        update_gemini_profile(&target_acc.email)?;
        println!(
            "{} {}",
            "✔ Switched active AGY account to:".green().bold(),
            target_acc.email.bold().cyan()
        );
        Ok(())
    } else {
        println!(
            "{}",
            format!("Error: Failed to update keyring for {}", target_acc.email).red()
        );
        bail!("Failed to update keyring for {}", target_acc.email)
    }
}

/// Deletes the saved credential file for the specified account from storage.
pub fn remove_account(account_name: &str) -> Result<()> {
    let acc_dir = get_accounts_dir();
    let target_path = acc_dir.join(format!("{}.json", account_name));

    if !target_path.exists() {
        bail!("Account '{}' not found", account_name);
    }

    fs::remove_file(&target_path)?;
    println!(
        "{} Removed account: {}",
        "✔".green().bold(),
        account_name.bold().yellow()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_extraction() {
        let email = "user.name@example.com";
        let prefix = email.split('@').next().unwrap_or(email);
        assert_eq!(prefix, "user.name");

        let non_email = "plain_user";
        let prefix_non = non_email.split('@').next().unwrap_or(non_email);
        assert_eq!(prefix_non, "plain_user");
    }

    #[test]
    fn test_get_home_not_empty() {
        let home = get_home();
        assert!(!home.as_os_str().is_empty());
    }

    #[test]
    fn test_account_info_debug_and_clone() {
        let acc = AccountInfo {
            email: "test@domain.com".to_string(),
            is_active: true,
            quota: None,
            file_path: PathBuf::from("/tmp/test@domain.com.json"),
        };
        let cloned = acc.clone();
        assert_eq!(cloned.email, "test@domain.com");
        assert!(cloned.is_active);
        assert_eq!(cloned.file_path, PathBuf::from("/tmp/test@domain.com.json"));
        let debug_str = format!("{:?}", acc);
        assert!(debug_str.contains("test@domain.com"));
    }

    #[test]
    fn test_account_priority_sorting() {
        let mut accounts = [
            AccountInfo {
                email: "b@test.com".to_string(),
                is_active: false,
                quota: Some(AccountQuotaInfo {
                    plan_type: None,
                    gemini_percent: Some(50),
                    claude_percent: Some(20),
                    top_model_name: None,
                    top_model_percent: None,
                    fetched_at: 0,
                    is_fresh: false,
                }),
                file_path: PathBuf::new(),
            },
            AccountInfo {
                email: "a@test.com".to_string(),
                is_active: false,
                quota: Some(AccountQuotaInfo {
                    plan_type: None,
                    gemini_percent: Some(90),
                    claude_percent: Some(10),
                    top_model_name: None,
                    top_model_percent: None,
                    fetched_at: 0,
                    is_fresh: false,
                }),
                file_path: PathBuf::new(),
            },
            AccountInfo {
                email: "c@test.com".to_string(),
                is_active: false,
                quota: None,
                file_path: PathBuf::new(),
            },
        ];

        accounts.sort_by(|a, b| {
            let pct_a = account_quota_priority(a);
            let pct_b = account_quota_priority(b);
            pct_b.cmp(&pct_a).then_with(|| a.email.cmp(&b.email))
        });

        assert_eq!(accounts[0].email, "a@test.com"); // 90%
        assert_eq!(accounts[1].email, "b@test.com"); // 50%
        assert_eq!(accounts[2].email, "c@test.com"); // 0%
    }
}
