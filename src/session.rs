use chrono::{DateTime, Local};
use colored::*;
use inquire::Select;
use regex::Regex;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub cid: String,
    pub mtime: u64,
    pub datetime: String,
    pub prompt: String,
}

impl std::fmt::Display for SessionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} │ {} │ {}",
            self.datetime.cyan(),
            self.cid.magenta(),
            self.prompt
        )
    }
}

pub fn pick_session() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/paisen".to_string());
    let home_path = PathBuf::from(&home);

    let search_roots = vec![
        home_path.join(".gemini/antigravity-cli/brain"),
        home_path.join(".gemini-profiles"),
    ];

    let mut sessions_map: HashMap<String, SessionInfo> = HashMap::new();
    let html_re = Regex::new(r"<[^>]+>").unwrap();

    for root in search_roots {
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(&root).max_depth(6).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.file_name() == Some(std::ffi::OsStr::new("transcript.jsonl")) {
                if let Some(logs_dir) = path.parent() {
                    if logs_dir.file_name() == Some(std::ffi::OsStr::new("logs")) {
                        if let Some(sys_gen) = logs_dir.parent() {
                            if sys_gen.file_name() == Some(std::ffi::OsStr::new(".system_generated")) {
                                if let Some(cid_dir) = sys_gen.parent() {
                                    let cid = cid_dir.file_name().unwrap_or_default().to_string_lossy().to_string();
                                    if cid.is_empty() {
                                        continue;
                                    }

                                    let metadata = match fs::metadata(path) {
                                        Ok(m) => m,
                                        Err(_) => continue,
                                    };

                                    let mtime = metadata
                                        .modified()
                                        .ok()
                                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);

                                    let mut prompt = String::new();
                                    if let Ok(file) = File::open(path) {
                                        let reader = BufReader::new(file);
                                        for line in reader.lines().filter_map(|l| l.ok()) {
                                            if line.contains(r#""type":"USER_INPUT""#) {
                                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                                    if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                                                        let clean = html_re.replace_all(content, "").to_string();
                                                        let single_line = clean.split_whitespace().collect::<Vec<_>>().join(" ");
                                                        if !single_line.trim().is_empty() {
                                                            prompt = single_line;
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if prompt.is_empty() {
                                        continue;
                                    }

                                    let dt = DateTime::from_timestamp(mtime as i64, 0)
                                        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    let info = SessionInfo {
                                        cid: cid.clone(),
                                        mtime,
                                        datetime: dt,
                                        prompt,
                                    };

                                    sessions_map
                                        .entry(cid)
                                        .and_modify(|existing| {
                                            if mtime > existing.mtime {
                                                *existing = info.clone();
                                            }
                                        })
                                        .or_insert(info);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut sessions: Vec<SessionInfo> = sessions_map.into_values().collect();
    if sessions.is_empty() {
        println!("{}", "No AGY sessions found.".yellow());
        return;
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));

    let ans = Select::new("🔍 Search AGY Session >", sessions)
        .with_page_size(15)
        .with_help_message("Type to filter | ↑↓ to navigate | Enter to resume | Esc to exit")
        .prompt();

    match ans {
        Ok(selected) => {
            println!("{} {}", "▶ Resuming session:".green(), selected.cid.cyan());
            let _ = Command::new("agy").args(["--conversation", &selected.cid]).exec();
        }
        Err(_) => {
            println!("{}", "Cancelled.".yellow());
        }
    }
}
