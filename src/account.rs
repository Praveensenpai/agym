use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use colored::*;
use rusqlite::Connection;
use std::fs::{self, File};
use std::io::{Write};
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn get_home() -> PathBuf {
    let h = std::env::var("HOME").unwrap_or_else(|_| "/home/paisen".to_string());
    PathBuf::from(h)
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

    let id_token = v
        .get("id_token")
        .and_then(|t| t.as_str())
        .or_else(|| {
            v.get("token")
                .and_then(|t| t.get("id_token"))
                .and_then(|t| t.as_str())
        })?;

    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let payload_b64 = parts[1];
    let decoded = URL_SAFE_NO_PAD.decode(payload_b64).ok().or_else(|| {
        let mut padded = payload_b64.to_string();
        let rem = padded.len() % 4;
        if rem == 2 {
            padded.push_str("==");
        } else if rem == 3 {
            padded.push('=');
        }
        base64::engine::general_purpose::STANDARD.decode(padded).ok()
    })?;

    let payload_json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    payload_json.get("email").and_then(|e| e.as_str()).map(|s| s.to_string())
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
            let _ = conn.execute("UPDATE accounts SET is_active = 1 WHERE email LIKE ?1", [&query]);
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

pub fn get_saved_accounts() -> Vec<String> {
    let _ = save_current_account();
    let acc_dir = get_accounts_dir();
    let mut saved = Vec::new();

    if let Ok(entries) = fs::read_dir(acc_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension() == Some(std::ffi::OsStr::new("json")) {
                if let Some(stem) = path.file_stem() {
                    saved.push(stem.to_string_lossy().to_string());
                }
            }
        }
    }
    saved.sort();
    saved
}

pub fn set_active_account(target: &str) {
    let _ = save_current_account();
    let saved = get_saved_accounts();

    let target_lc = target.to_lowercase();
    let matching = saved.iter().find(|email| email.to_lowercase().contains(&target_lc));

    let target_email = match matching {
        Some(m) => m,
        None => {
            println!("{}", format!("Error: Account matching '{}' not found in saved accounts.", target).red());
            println!("\nSaved accounts:");
            list_accounts();
            std::process::exit(1);
        }
    };

    let token_file = get_accounts_dir().join(format!("{}.json", target_email));
    let token_json = match fs::read_to_string(&token_file) {
        Ok(t) => t,
        Err(_) => {
            println!("{}", format!("Error: Failed to read token file for {}", target_email).red());
            std::process::exit(1);
        }
    };

    if write_keyring_token(&token_json) {
        update_sqlite_db(target_email);
        update_gemini_profile(target_email);
        println!("{} {}", "✔ Switched active AGY account to:".green(), target_email.cyan());
    } else {
        println!("{}", format!("Error: Failed to update keyring for {}", target_email).red());
    }
}

pub fn list_accounts() {
    let _ = save_current_account();
    let current_token = get_current_keyring_token();
    let current_email = current_token.as_deref().and_then(extract_email_from_token_json);

    let saved = get_saved_accounts();
    println!("{}", "✨ SAVED AGY ACCOUNTS".cyan());
    println!("-----------------------------------");
    for email in saved {
        if Some(&email) == current_email.as_ref() {
            println!("  {}", format!("* {} (active)", email).green());
        } else {
            println!("    {}", email);
        }
    }
}

pub fn interactive_switch() {
    let _ = save_current_account();
    let saved = get_saved_accounts();
    let current_token = get_current_keyring_token();
    let current_email = current_token.as_deref().and_then(extract_email_from_token_json);

    if saved.is_empty() {
        println!("No saved accounts found.");
        std::process::exit(1);
    }

    let mut formatted_input = String::new();
    for email in &saved {
        let status = if Some(email) == current_email.as_ref() { "(active)" } else { "" };
        formatted_input.push_str(&format!("{} {}\n", email, status));
    }

    let fzf_colors = "bg+:#282a36,bg:#1e1e2e,spinner:#ff79c6,hl:#bd93f9,fg:#f8f8f2,header:#8be9fd,info:#ffb86c,pointer:#ff79c6,marker:#ff79c6,fg+:#f8f8f2,prompt:#50fa7b";

    let child = Command::new("fzf")
        .args([
            "--height=40%",
            "--layout=reverse",
            "--border=rounded",
            "--prompt=👤 Select AGY Account > ",
            "--header=[ ENTER: Switch Account | ESC: Cancel ]",
            &format!("--color={}", fzf_colors),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();

    if let Ok(mut proc) = child {
        if let Some(mut stdin) = proc.stdin.take() {
            let _ = stdin.write_all(formatted_input.as_bytes());
        }
        let output = proc.wait_with_output();
        if let Ok(out) = output {
            if out.status.success() {
                let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let email = line.split_whitespace().next().unwrap_or("");
                if !email.is_empty() {
                    set_active_account(email);
                    return;
                }
            }
        }
    }

    list_accounts();
}
