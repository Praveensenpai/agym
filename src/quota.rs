use crate::account::{
    extract_email_from_token_json, get_accounts_dir, get_current_keyring_token,
    get_gemini_link, get_profiles_dir, get_saved_accounts, save_current_account,
};
use chrono::{DateTime, Local, Utc};
use colored::*;
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use walkdir::WalkDir;

pub fn format_relative_time(ts: u64) -> String {
    if ts == 0 {
        return "Never".to_string();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now < ts {
        return "Just now".to_string();
    }
    let diff = now - ts;
    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

pub fn get_last_used(email: &str) -> String {
    let prefix = email.split('@').next().unwrap_or(email);
    let mut possible_dirs = vec![
        get_profiles_dir().join(format!("{}/antigravity-cli/brain", prefix)),
        get_profiles_dir().join(format!("{}/gemini/antigravity-cli/brain", prefix)),
    ];

    let gemini_link = get_gemini_link();
    if let Ok(target) = fs::read_link(&gemini_link) {
        if target.to_string_lossy().contains(prefix) {
            possible_dirs.push(get_home().join(".gemini/antigravity-cli/brain"));
        }
    }

    let mut latest_time: u64 = 0;

    for bdir in possible_dirs {
        if !bdir.exists() {
            continue;
        }
        for entry in WalkDir::new(bdir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_name() == "transcript.jsonl" {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        let secs = modified
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        if secs > latest_time {
                            latest_time = secs;
                        }
                    }
                }
            }
        }
    }

    if latest_time > 0 {
        let dt = DateTime::from_timestamp(latest_time as i64, 0)
            .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let rel = format_relative_time(latest_time);
        format!("{} ({})", dt, rel)
    } else {
        "Never".to_string()
    }
}

fn get_home() -> PathBuf {
    let h = std::env::var("HOME").unwrap_or_else(|_| "/home/paisen".to_string());
    PathBuf::from(h)
}

pub fn fetch_account_models(acc_path: &Path) -> Option<Value> {
    let content = fs::read_to_string(acc_path).ok()?;
    let mut tok_json: Value = serde_json::from_str(&content).ok()?;

    let mut access_tok = tok_json
        .get("access_token")
        .and_then(|v| v.as_str())
        .or_else(|| tok_json.get("token").and_then(|t| t.get("access_token")).and_then(|v| v.as_str()))
        .map(|s| s.to_string())?;

    let refresh_tok = tok_json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .or_else(|| tok_json.get("token").and_then(|t| t.get("refresh_token")).and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let client = Client::builder().timeout(Duration::from_secs(10)).build().ok()?;
    let url = "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels";

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_tok))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Antigravity/1.0")
        .json(&serde_json::json!({}))
        .send();

    let mut res_val: Option<Value> = resp.ok().and_then(|r| r.json().ok());

    let has_models = res_val
        .as_ref()
        .and_then(|v| v.get("models"))
        .map(|m| !m.is_null())
        .unwrap_or(false);

    if !has_models {
        if let Some(ref_tok) = refresh_tok {
            let ref_url = "https://oauth2.googleapis.com/token";
            let ref_resp = client
                .post(ref_url)
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", ref_tok.as_str()),
                    (
                        "client_id",
                        "764086051850-6qr4p6gpi6hn506pt8ejuq83di341pvb.apps.googleusercontent.com",
                    ),
                ])
                .send();

            if let Ok(ref_r) = ref_resp {
                if let Ok(ref_json) = ref_r.json::<Value>() {
                    if let Some(new_access) = ref_json.get("access_token").and_then(|v| v.as_str()) {
                        access_tok = new_access.to_string();

                        // Update JSON file
                        if let Some(tok_obj) = tok_json.get_mut("token") {
                            if tok_obj.is_object() {
                                tok_obj["access_token"] = Value::String(new_access.to_string());
                            }
                        } else {
                            tok_json["access_token"] = Value::String(new_access.to_string());
                        }

                        if let Ok(new_content) = serde_json::to_string_pretty(&tok_json) {
                            let _ = fs::write(acc_path, new_content);
                        }

                        let retry_resp = client
                            .post(url)
                            .header("Authorization", format!("Bearer {}", access_tok))
                            .header("Content-Type", "application/json")
                            .header("User-Agent", "Antigravity/1.0")
                            .json(&serde_json::json!({}))
                            .send();

                        res_val = retry_resp.ok().and_then(|r| r.json().ok());
                    }
                }
            }
        }
    }

    res_val
}

pub fn make_progress_bar(frac: Option<f64>) -> String {
    let f = match frac {
        Some(val) => val.clamp(0.0, 1.0),
        None => return format!("{}[───────────────] N/A{}", "\x1b[38;2;98;114;164m", "\x1b[0m"),
    };

    let percent = f * 100.0;
    let filled = (f * 15.0 + 0.5) as usize;
    let filled = filled.min(15);
    let empty = 15 - filled;

    let bar_color = if f > 0.5 {
        "\x1b[38;2;80;250;123m" // GREEN
    } else if f > 0.2 {
        "\x1b[38;2;241;250;140m" // YELLOW
    } else {
        "\x1b[38;2;255;85;85m" // RED
    };

    let gray = "\x1b[38;2;98;114;164m";
    let reset = "\x1b[0m";

    let filled_bar = "█".repeat(filled);
    let empty_bar = "░".repeat(empty);

    format!("[{}{}{}{}] {}{:5.1}%{}", bar_color, filled_bar, gray, empty_bar, bar_color, percent, reset)
}

pub fn format_reset_time(reset_str: Option<&str>) -> String {
    let s = match reset_str {
        Some(val) if !val.is_empty() => val,
        _ => return format!("{}No reset info{}", "\x1b[38;2;98;114;164m", "\x1b[0m"),
    };

    let reset_dt = DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ").map(|d| d.with_timezone(&Utc)));

    let reset_dt = match reset_dt {
        Ok(dt) => dt,
        Err(_) => return format!("{}{}{}", "\x1b[38;2;98;114;164m", s, "\x1b[0m"),
    };

    let now = Utc::now();
    let diff = reset_dt.signed_duration_since(now);

    if diff.num_seconds() <= 0 {
        return format!("{}Resets now{}", "\x1b[38;2;80;250;123m", "\x1b[0m");
    }

    let hrs = diff.num_hours();
    let mins = diff.num_minutes() % 60;

    let text = if hrs > 24 {
        format!("Resets in {}d {}h", hrs / 24, hrs % 24)
    } else if hrs > 0 {
        format!("Resets in {}h {}m", hrs, mins)
    } else {
        format!("Resets in {}m", mins)
    };

    format!("{}{}{}", "\x1b[38;2;98;114;164m", text, "\x1b[0m")
}

pub fn show_stats(verbose: bool) {
    let _ = save_current_account();
    let current_token = get_current_keyring_token();
    let active_email = current_token.as_deref().and_then(extract_email_from_token_json);

    let saved = get_saved_accounts();
    if saved.is_empty() {
        println!("{}", "No saved accounts found.".red());
        return;
    }

    println!("\n{}", "╭──────────────────────────────────────────────────────────────╮".purple().bold());
    println!("{}", "│ ✨ AGY ACCOUNTS QUOTA & MODEL STATS                         │".purple().bold());
    println!("{}\n", "╰──────────────────────────────────────────────────────────────╯".purple().bold());

    let primary_mids = [
        "gemini-3.6-flash-high",
        "gemini-3.1-pro-high",
        "claude-sonnet-4-6",
        "claude-opus-4-6-thinking",
        "gpt-oss-120b-medium",
    ];

    let primary_labels = [
        "⚡ Gemini 3.6 Flash",
        "🧠 Gemini 3.1 Pro",
        "🎭 Claude Sonnet 4.6",
        "🔮 Claude Opus 4.6",
        "🤖 GPT 120B Medium",
    ];

    for email in saved {
        let is_active = active_email.as_ref() == Some(&email);
        let status_badge = if is_active {
            format!("{}★ ACTIVE{}", "\x1b[38;2;80;250;123m", "\x1b[0m")
        } else {
            format!("{}  INACTIVE{}", "\x1b[38;2;98;114;164m", "\x1b[0m")
        };

        let email_disp = if is_active {
            format!("\x1b[1m\x1b[38;2;139;233;253m{:34}\x1b[0m", email)
        } else {
            format!("\x1b[1m\x1b[38;2;255;184;108m{:34}\x1b[0m", email)
        };

        let last_used = get_last_used(&email);

        println!("  {} {}", email_disp, status_badge);
        println!("  {} {}", "🕒 Last Used:".magenta(), last_used.truecolor(255, 121, 198));

        let acc_file = get_accounts_dir().join(format!("{}.json", email));
        let res = fetch_account_models(&acc_file);

        let has_models = res
            .as_ref()
            .and_then(|v| v.get("models"))
            .map(|m| !m.is_null())
            .unwrap_or(false);

        if !has_models {
            println!(
                "  {}\n",
                format!("⚠️ Failed to fetch quota info (Switch with 'agym use {}' and run 'agy' to auto-auth/refresh)", email).red()
            );
            continue;
        }

        let res_val = res.unwrap();
        let models_map = res_val.get("models").and_then(|m| m.as_object());

        println!("  {}", "Model Quotas:".bold());

        for (i, m_id) in primary_mids.iter().enumerate() {
            let label = primary_labels[i];
            let padded_label = format!("{:22}", label);

            if let Some(m_info) = models_map.and_then(|m| m.get(*m_id)) {
                let frac = m_info
                    .get("quotaInfo")
                    .and_then(|q| q.get("remainingFraction"))
                    .and_then(|f| f.as_f64());
                let reset_t = m_info
                    .get("quotaInfo")
                    .and_then(|q| q.get("resetTime"))
                    .and_then(|r| r.as_str());

                let pbar = make_progress_bar(frac);
                let reset_disp = format_reset_time(reset_t);

                println!("    │ {} {}  ({})", padded_label.cyan(), pbar, reset_disp);
            } else {
                let pbar = make_progress_bar(None);
                println!("    │ {} {}", padded_label.truecolor(98, 114, 164), pbar);
            }
        }

        if verbose {
            println!("\n  {}", "Other Models:".truecolor(98, 114, 164).bold());
            if let Some(map) = models_map {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for m_id in keys {
                    if primary_mids.contains(&m_id.as_str()) {
                        continue;
                    }
                    let m_info = &map[m_id];
                    let frac = m_info
                        .get("quotaInfo")
                        .and_then(|q| q.get("remainingFraction"))
                        .and_then(|f| f.as_f64());
                    let reset_t = m_info
                        .get("quotaInfo")
                        .and_then(|q| q.get("resetTime"))
                        .and_then(|r| r.as_str());

                    let pbar = make_progress_bar(frac);
                    let reset_disp = format_reset_time(reset_t);
                    let padded_id = format!("{:22}", m_id);

                    println!("    │ {} {}  ({})", padded_id.truecolor(98, 114, 164), pbar, reset_disp);
                }
            }
        }
        println!();
    }
}
