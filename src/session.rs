use anyhow::Result;
use chrono::{DateTime, Local};
use colored::*;
use inquire::Select;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct AgySession {
    pub conversation_id: String,
    pub prompt: String,
    pub timestamp: u64,
}

impl AgySession {
    pub fn formatted_time(&self) -> String {
        if self.timestamp == 0 {
            return "unknown time".to_string();
        }
        let naive = DateTime::from_timestamp(self.timestamp as i64, 0);
        match naive {
            Some(utc) => {
                let local: DateTime<Local> = DateTime::from(utc);
                local.format("%Y-%m-%d %H:%M").to_string()
            }
            None => "unknown time".to_string(),
        }
    }

    pub fn short_id(&self) -> String {
        if self.conversation_id.len() >= 8 {
            self.conversation_id[..8].to_string()
        } else {
            self.conversation_id.clone()
        }
    }
}

pub fn sanitize_prompt(raw: &str) -> String {
    let cleaned = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("<ADDITIONAL_METADATA>")
                && !l.starts_with("<USER_SETTINGS_CHANGE>")
                && !l.starts_with("The current local time is:")
                && !l.starts_with("The user changed setting")
        })
        .collect::<Vec<&str>>()
        .join(" ");

    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "New Conversation".to_string()
    } else if trimmed.chars().count() > 60 {
        format!("{}...", trimmed.chars().take(57).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

pub fn get_search_dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/paisen"));
    let mut dirs = vec![
        home.join(".gemini/antigravity-cli/brain"),
        home.join(".antigravity-agent/brain"),
    ];

    let profiles_dir = home.join(".gemini-profiles");
    if profiles_dir.exists() {
        if let Ok(entries) = fs::read_dir(profiles_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path.join("antigravity-cli/brain"));
                }
            }
        }
    }

    dirs
}

pub fn extract_first_prompt_from_transcript(path: &Path) -> Option<(String, u64)> {
    let meta = path.metadata().ok()?;
    let modified_ts = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if line.contains("\"USER_INPUT\"") || line.contains("\"type\":\"USER_INPUT\"") {
            if let Ok(json) = serde_json::from_str::<Value>(line) {
                if let Some(text) = json.get("content").and_then(|c| c.as_str()) {
                    let sanitized = sanitize_prompt(text);
                    if sanitized != "New Conversation" {
                        return Some((sanitized, modified_ts));
                    }
                }
            }
        }
    }

    Some(("New Conversation".to_string(), modified_ts))
}

pub fn scan_agy_sessions() -> Vec<AgySession> {
    let search_dirs = get_search_dirs();
    let mut session_map: HashMap<String, (String, u64)> = HashMap::new();

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let session_dir = entry.path();
                if session_dir.is_dir() {
                    let cid = session_dir.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if cid.starts_with('.') || cid == "scratch" {
                        continue;
                    }
                    let transcript_path = session_dir.join("logs/transcript.jsonl");
                    let alt_transcript = session_dir.join("transcript.jsonl");

                    let target = if transcript_path.exists() {
                        Some(transcript_path)
                    } else if alt_transcript.exists() {
                        Some(alt_transcript)
                    } else {
                        None
                    };

                    if let Some(t_path) = target {
                        if let Some((prompt, ts)) = extract_first_prompt_from_transcript(&t_path) {
                            session_map.entry(cid).or_insert((prompt, ts));
                        }
                    }
                }
            }
        }
    }

    let mut sessions: Vec<AgySession> = session_map
        .into_iter()
        .map(|(conversation_id, (prompt, timestamp))| AgySession {
            conversation_id,
            prompt,
            timestamp,
        })
        .collect();

    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    sessions
}

pub fn pick_and_resume_session() -> Result<()> {
    let sessions = scan_agy_sessions();
    if sessions.is_empty() {
        println!("{}", "No Antigravity session history found.".yellow());
        return Ok(());
    }

    let options: Vec<String> = sessions
        .iter()
        .map(|s| {
            format!(
                "{} [{}] {}",
                s.short_id().cyan(),
                s.formatted_time().dimmed(),
                s.prompt.bold()
            )
        })
        .collect();

    let ans = Select::new("💬 Select Antigravity Session to Resume:", options).prompt();

    match ans {
        Ok(choice) => {
            let short_id = choice.split_whitespace().next().unwrap_or("").trim();
            if let Some(target) = sessions.iter().find(|s| s.short_id() == short_id) {
                println!("{} Resuming Antigravity session {}...", "🚀".bold(), target.conversation_id.cyan());
                let mut child = Command::new("agy")
                    .args(["resume", &target.conversation_id])
                    .spawn()?;
                let _ = child.wait();
            }
        }
        Err(_) => {
            println!("Operation cancelled.");
        }
    }

    Ok(())
}
