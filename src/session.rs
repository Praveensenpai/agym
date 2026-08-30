use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Represents summary metadata for a previously recorded conversation session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Full 36-character UUID conversation ID.
    pub cid: String,
    /// Truncated 8-character ID for compact table display.
    pub short_cid: String,
    /// Localized human-readable timestamp string (`YYYY-MM-DD HH:MM`).
    pub datetime: String,
    /// Unix timestamp in seconds for temporal sorting.
    pub timestamp: u64,
    /// Transcript file size in bytes.
    pub size_bytes: u64,
    /// Number of lines in the transcript file.
    pub line_count: usize,
    /// Sanitized single-line summary of the conversation's first user prompt.
    pub summary: String,
    /// Cleaned multi-line text of the conversation's first prompt.
    pub full_prompt: String,
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

/// Scans standard Antigravity CLI and profile brain directories for session transcripts.
pub fn scan_sessions() -> Vec<SessionInfo> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let mut search_roots = vec![
        home.join(".gemini/antigravity-cli/brain"),
        home.join(".antigravity-agent/brain"),
    ];

    let profiles_dir = home.join(".gemini-profiles");
    if profiles_dir.exists() {
        if let Ok(entries) = fs::read_dir(&profiles_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    search_roots.push(path.join("antigravity-cli/brain"));
                    search_roots.push(path.join("gemini/antigravity-cli/brain"));
                }
            }
        }
    }

    let mut session_map: HashMap<String, SessionInfo> = HashMap::new();

    for root in search_roots {
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(&root)
            .max_depth(20)
            .into_iter()
            .filter_map(|e| e.ok())
        {
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
                    line_count,
                    summary,
                    full_prompt: if full_prompt.is_empty() {
                        "New Conversation".to_string()
                    } else {
                        full_prompt
                    },
                };

                let existing = session_map.get(&cid);
                if existing.is_none_or(|e| modified_ts > e.timestamp) {
                    session_map.insert(cid, item);
                }
            }
        }
    }

    let mut sessions: Vec<SessionInfo> = session_map.into_values().collect();
    sessions.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
    sessions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500B");
        assert_eq!(format_bytes(1024), "1.0KB");
        assert_eq!(format_bytes(1536), "1.5KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0MB");
        assert_eq!(format_bytes(2500 * 1024), "2.4MB");
    }

    #[test]
    fn test_clean_user_text() {
        let raw = "<USER_REQUEST>\nFix the bug\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\ninfo\n</ADDITIONAL_METADATA>";
        let cleaned = clean_user_text(raw);
        assert_eq!(cleaned, "Fix the bug\ninfo");
    }

    #[test]
    fn test_sanitize_summary() {
        let raw = "<USER_REQUEST>\n   Hello world   \nMore text\n</USER_REQUEST>";
        assert_eq!(sanitize_summary(raw), "Hello world More text");
        assert_eq!(sanitize_summary(""), "New Conversation");
        assert_eq!(
            sanitize_summary("<USER_REQUEST></USER_REQUEST>"),
            "New Conversation"
        );
    }
}
