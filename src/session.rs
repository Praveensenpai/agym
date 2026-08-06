use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub cid: String,
    pub short_cid: String,
    pub datetime: String,
    pub timestamp: u64,
    pub size_bytes: u64,
    pub size_fmt: String,
    pub line_count: usize,
    pub summary: String,
    pub full_prompt: String,
    pub profile: Option<String>,
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn clean_user_text(raw: &str) -> String {
    let mut s = raw.to_string();
    let tags = [
        "<USER_REQUEST>",
        "</USER_REQUEST>",
        "<USER_SETTINGS_CHANGE>",
        "</USER_SETTINGS_CHANGE>",
        "<ADDITIONAL_METADATA>",
        "</ADDITIONAL_METADATA>",
        "<EPHEMERAL_MESSAGE>",
        "</EPHEMERAL_MESSAGE>",
    ];
    for tag in tags {
        s = s.replace(tag, "");
    }

    let cleaned = s
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with('<')
                && !l.starts_with("The current local time is:")
                && !l.starts_with("The user changed setting")
                && !l.starts_with("The user has uploaded")
                && !l.starts_with("┌─")
                && !l.starts_with("└─")
                && !l.starts_with('│')
                && !l.starts_with("~ ❯")
                && !l.starts_with("~ ✗")
        })
        .collect::<Vec<&str>>()
        .join("\n");

    cleaned.trim().to_string()
}

pub fn sanitize_summary(raw: &str) -> String {
    let cleaned = clean_user_text(raw);
    let single_line = cleaned
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<&str>>()
        .join(" ");

    let trimmed = single_line.trim();
    if trimmed.is_empty() {
        "New Conversation".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn scan_sessions() -> Vec<SessionInfo> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/paisen"));
    let mut search_roots = vec![
        (home.join(".gemini/antigravity-cli/brain"), None),
        (home.join(".antigravity-agent/brain"), None),
    ];

    let profiles_dir = home.join(".gemini-profiles");
    if profiles_dir.exists() {
        if let Ok(entries) = fs::read_dir(&profiles_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    let prof_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    search_roots.push((path.join("antigravity-cli/brain"), Some(prof_name.clone())));
                    search_roots.push((path.join("gemini/antigravity-cli/brain"), Some(prof_name)));
                }
            }
        }
    }

    let mut session_map: HashMap<String, SessionInfo> = HashMap::new();

    for (root, prof_name) in search_roots {
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(&root).max_depth(20).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.file_name() == Some(std::ffi::OsStr::new("transcript.jsonl")) {
                let meta = match path.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let modified_ts = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                let datetime = DateTime::from_timestamp(modified_ts as i64, 0)
                    .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let size_bytes = meta.len();
                let size_fmt = format_bytes(size_bytes);

                let content = match fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let line_count = content.lines().count();
                let mut raw_prompt = "New Conversation".to_string();

                for line in content.lines() {
                    if line.contains("\"USER_INPUT\"") || line.contains("\"type\":\"USER_INPUT\"") {
                        if let Ok(json) = serde_json::from_str::<Value>(line) {
                            if let Some(text) = json.get("content").and_then(|c| c.as_str()) {
                                let summary_candidate = sanitize_summary(text);
                                if summary_candidate != "New Conversation" {
                                    raw_prompt = text.to_string();
                                    break;
                                }
                            }
                        }
                    }
                }

                let full_prompt = clean_user_text(&raw_prompt);
                let summary = sanitize_summary(&raw_prompt);

                let cid = path
                    .ancestors()
                    .find(|p| {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            name.len() == 36 && name.contains('-')
                        } else {
                            false
                        }
                    })
                    .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                if cid == "unknown" || cid == "scratch" {
                    continue;
                }

                let short_cid = if cid.len() >= 8 {
                    cid[..8].to_string()
                } else {
                    cid.clone()
                };

                let item = SessionInfo {
                    cid: cid.clone(),
                    short_cid,
                    datetime,
                    timestamp: modified_ts,
                    size_bytes,
                    size_fmt,
                    line_count,
                    summary,
                    full_prompt: if full_prompt.is_empty() { "New Conversation".to_string() } else { full_prompt },
                    profile: prof_name.clone(),
                };

                let existing = session_map.get(&cid);
                if existing.map_or(true, |e| modified_ts > e.timestamp) {
                    session_map.insert(cid, item);
                }
            }
        }
    }

    let mut sessions: Vec<SessionInfo> = session_map.into_values().collect();
    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    sessions
}


