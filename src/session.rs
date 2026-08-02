use chrono::{DateTime, Local};
use regex::Regex;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub cid: String,
    pub mtime: u64,
    pub datetime: String,
    pub prompt: String,
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
                // Verify structure: .../<cid>/.system_generated/logs/transcript.jsonl
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

                                    // Extract prompt from first USER_INPUT line
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

                                    // Retain newer mtime if cid exists
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
        println!("No AGY sessions found.");
        return;
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));

    let fzf_colors = "bg+:#282a36,bg:#1e1e2e,spinner:#ff79c6,hl:#bd93f9,fg:#f8f8f2,header:#8be9fd,info:#ffb86c,pointer:#ff79c6,marker:#ff79c6,fg+:#f8f8f2,prompt:#50fa7b";

    let mut formatted_input = String::new();
    for s in &sessions {
        formatted_input.push_str(&format!("{} │ {} │ {}\n", s.datetime, s.cid, s.prompt));
    }

    let child = Command::new("fzf")
        .args([
            "--ansi",
            "--height=60%",
            "--layout=reverse",
            "--border=rounded",
            "--prompt=🔍 Search Session > ",
            "--header=[ ENTER: Resume Session | ESC: Exit ]",
            &format!("--color={}", fzf_colors),
            "--delimiter=│",
            "--with-nth=1,3",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();

    let selected_line = match child {
        Ok(mut proc) => {
            if let Some(mut stdin) = proc.stdin.take() {
                let _ = stdin.write_all(formatted_input.as_bytes());
            }
            let output = proc.wait_with_output();
            match output {
                Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
                _ => {
                    println!("Cancelled.");
                    return;
                }
            }
        }
        Err(_) => {
            // Fallback CLI selection if fzf is not installed
            println!("fzf not found. Recent sessions:");
            for (idx, s) in sessions.iter().take(10).enumerate() {
                println!("[{}] {} | {} | {}", idx + 1, s.datetime, s.cid, s.prompt);
            }
            return;
        }
    };

    if selected_line.is_empty() {
        println!("Cancelled.");
        return;
    }

    let parts: Vec<&str> = selected_line.split('│').collect();
    if parts.len() >= 2 {
        let cid = parts[1].trim();
        if !cid.is_empty() {
            println!("▶ Resuming session: \x1b[38;2;139;233;253m{}\x1b[0m", cid);
            let _ = Command::new("agy").args(["--conversation", cid]).exec();
        }
    }
}
