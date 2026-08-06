use crate::quota::{fetch_quota_cached, load_quota_cache, AccountQuotaInfo};
use anyhow::{anyhow, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use colored::*;
use inquire::Select;
use rusqlite::Connection;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub email: String,
    pub is_active: bool,
    pub quota: Option<AccountQuotaInfo>,
    pub file_path: PathBuf,
}

fn get_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/paisen"))
}

pub fn get_accounts_dir() -> PathBuf {
    let p = get_home().join(".gemini-accounts");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn get_profiles_dir() -> PathBuf {
    let p = get_home().join(".gemini-profiles");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn get_db_path() -> PathBuf {
    get_home().join(".antigravity-agent/cloud_accounts.db")
}

pub fn get_gemini_link() -> PathBuf {
    get_home().join(".gemini")
}

pub fn get_current_keyring_token() -> Option<String> {
    let output = Command::new("secret-tool")
        .args(["lookup", "service", "gemini", "username", "antigravity"])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

pub fn write_keyring_token(token_json: &str) -> bool {
    let child = Command::new("secret-tool")
        .args([
            "store",
            "--label=Password for 'antigravity' on 'gemini'",
            "service",
            "gemini",
            "username",
            "antigravity",
        ])
        .stdin(Stdio::piped())
        .spawn();

    if let Ok(mut proc) = child {
        if let Some(mut stdin) = proc.stdin.take() {
            let _ = stdin.write_all(token_json.as_bytes());
        }
        return proc.wait().map(|s| s.success()).unwrap_or(false);
    }
    false
}

pub fn extract_email_from_token_json(token_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(token_json).ok()?;

    // Try id_token JWT path first (old agy format)
    let id_token = v.get("id_token").and_then(|t| t.as_str()).or_else(|| {
        v.get("token")
            .and_then(|t| t.get("id_token"))
            .and_then(|t| t.as_str())
    });

    if let Some(id_token) = id_token {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() >= 2 {
            let payload_b64 = parts[1];
            let decoded = URL_SAFE_NO_PAD.decode(payload_b64).ok().or_else(|| {
                let mut padded = payload_b64.to_string();
                let rem = padded.len() % 4;
                if rem == 2 {
                    padded.push_str("==");
                } else if rem == 3 {
                    padded.push('=');
                }
                base64::engine::general_purpose::STANDARD
                    .decode(padded)
                    .ok()
            });

            if let Some(bytes) = decoded {
                if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if let Some(email) = payload.get("email").and_then(|e| e.as_str()) {
                        return Some(email.to_string());
                    }
                }
            }
        }
    }

    // Fallback: new agy token format has no id_token — resolve email via Google tokeninfo API
    let access_token = v
        .get("access_token")
        .and_then(|t| t.as_str())
        .or_else(|| {
            v.get("token")
                .and_then(|t| t.get("access_token"))
                .and_then(|t| t.as_str())
        })?;

    let url = format!(
        "https://www.googleapis.com/oauth2/v1/tokeninfo?access_token={}",
        access_token
    );

    let resp = reqwest::blocking::get(&url).ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let info: serde_json::Value = resp.json().ok()?;
    info.get("email")
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
}

pub fn save_current_account() -> Option<String> {
    let token_json = get_current_keyring_token()?;
    let email = extract_email_from_token_json(&token_json)?;

    let acc_dir = get_accounts_dir();
    let file_path = acc_dir.join(format!("{}.json", email));
    if let Ok(mut f) = File::create(&file_path) {
        let _ = f.write_all(token_json.as_bytes());
        return Some(email);
    }
    None
}

pub fn update_sqlite_db(email: &str) {
    let db_path = get_db_path();
    if db_path.exists() {
        if let Ok(conn) = Connection::open(&db_path) {
            let _ = conn.execute("UPDATE accounts SET is_active = 0", []);
            let query = format!("%{}%", email);
            let _ = conn.execute(
                "UPDATE accounts SET is_active = 1 WHERE email LIKE ?1",
                [&query],
            );
        }
    }
}

pub fn update_gemini_profile(email: &str) {
    let prefix = email.split('@').next().unwrap_or(email);
    let prof_dir = get_profiles_dir().join(prefix);
    let _ = fs::create_dir_all(&prof_dir);

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

    let _ = unix_fs::symlink(&prof_dir, &gemini_link);
}

fn list_account_infos_with<F>(mut quota_for: F, detect_active: bool) -> Vec<AccountInfo>
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
            if path.extension() == Some(std::ffi::OsStr::new("json")) {
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
        let pct_a = a
            .quota
            .as_ref()
            .map(|q| {
                q.gemini_percent
                    .unwrap_or(0)
                    .max(q.claude_percent.unwrap_or(0))
            })
            .unwrap_or(0);
        let pct_b = b
            .quota
            .as_ref()
            .map(|q| {
                q.gemini_percent
                    .unwrap_or(0)
                    .max(q.claude_percent.unwrap_or(0))
            })
            .unwrap_or(0);
        pct_b.cmp(&pct_a).then_with(|| a.email.cmp(&b.email))
    });
    accounts
}

pub fn list_account_infos(no_cache: bool) -> Vec<AccountInfo> {
    list_account_infos_with(
        |stem, path| fetch_quota_cached(stem, path, no_cache).ok(),
        true,
    )
}

/// Load only non-expired quota data from disk so the TUI can render immediately.
pub fn list_account_infos_cached() -> Vec<AccountInfo> {
    let cache = load_quota_cache();
    list_account_infos_with(
        |stem, _| cache.get(stem).filter(|quota| !quota.is_expired()).cloned(),
        false,
    )
}

pub fn set_active_account(target: &str) {
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
            list_all_accounts(false).ok();
            std::process::exit(1);
        }
    };

    let token_json = match fs::read_to_string(&target_acc.file_path) {
        Ok(t) => t,
        Err(_) => {
            println!(
                "{}",
                format!("Error: Failed to read token file for {}", target_acc.email).red()
            );
            std::process::exit(1);
        }
    };

    if write_keyring_token(&token_json) {
        update_sqlite_db(&target_acc.email);
        update_gemini_profile(&target_acc.email);
        println!(
            "{} {}",
            "✔ Switched active AGY account to:".green().bold(),
            target_acc.email.bold().cyan()
        );
    } else {
        println!(
            "{}",
            format!("Error: Failed to update keyring for {}", target_acc.email).red()
        );
    }
}

pub fn remove_account(account_name: &str) -> Result<()> {
    let acc_dir = get_accounts_dir();
    let target_path = acc_dir.join(format!("{}.json", account_name));

    if !target_path.exists() {
        return Err(anyhow!("Account '{}' not found", account_name));
    }

    fs::remove_file(&target_path)?;
    println!(
        "{} Removed account: {}",
        "✔".green().bold(),
        account_name.bold().yellow()
    );
    Ok(())
}

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

pub fn prepare_new_session() -> Result<()> {
    let _ = save_current_account();

    let _ = Command::new("secret-tool")
        .args(["clear", "service", "gemini", "username", "antigravity"])
        .output();

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
