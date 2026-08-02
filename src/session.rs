use chrono::{DateTime, Local};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use regex::Regex;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{stdout, BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub cid: String,
    pub mtime: u64,
    pub datetime: String,
    pub summary: String,
    pub full_prompt: String,
}

pub fn extract_clean_prompt(content: &str) -> (String, String) {
    let mut raw_text = content.to_string();

    // 1. Extract <USER_REQUEST>...</USER_REQUEST> if present
    if let Some(start) = raw_text.find("<USER_REQUEST>") {
        if let Some(end) = raw_text.find("</USER_REQUEST>") {
            if end > start + 14 {
                raw_text = raw_text[start + 14..end].to_string();
            }
        }
    }

    // 2. Remove <ADDITIONAL_METADATA>...</ADDITIONAL_METADATA>
    if let Some(start) = raw_text.find("<ADDITIONAL_METADATA>") {
        if let Some(end) = raw_text.find("</ADDITIONAL_METADATA>") {
            if end > start {
                let mut cleaned = raw_text[..start].to_string();
                cleaned.push_str(&raw_text[end + 22..]);
                raw_text = cleaned;
            }
        } else {
            raw_text = raw_text[..start].to_string();
        }
    }

    // 3. Strip HTML/XML tags
    let html_re = Regex::new(r"<[^>]+>").unwrap();
    let no_tags = html_re.replace_all(&raw_text, "").to_string();

    // 4. Filter metadata lines
    let lines: Vec<&str> = no_tags
        .lines()
        .filter(|l| {
            let trim = l.trim();
            !trim.is_empty()
                && !trim.starts_with("The current local time is:")
                && !trim.starts_with("The user changed setting")
                && !trim.starts_with("No need to comment on this change")
                && !trim.starts_with("If reporting what model you are")
                && !trim.starts_with("You can embed this image in an artifact")
                && !trim.starts_with("Error: stream reading error")
                && !trim.starts_with("Error: request failed")
        })
        .collect();

    let full = lines.join("\n");
    let single_line = lines.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");

    let summary = if single_line.chars().count() > 70 {
        let truncated: String = single_line.chars().take(70).collect();
        format!("{}...", truncated)
    } else {
        single_line
    };

    (summary, full)
}

pub fn pick_session() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/paisen".to_string());
    let home_path = PathBuf::from(&home);

    let search_roots = vec![
        home_path.join(".gemini/antigravity-cli/brain"),
        home_path.join(".gemini-profiles"),
    ];

    let mut sessions_map: HashMap<String, SessionInfo> = HashMap::new();

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

                                    let mut summary = String::new();
                                    let mut full_prompt = String::new();

                                    if let Ok(file) = File::open(path) {
                                        let reader = BufReader::new(file);
                                        for line in reader.lines().filter_map(|l| l.ok()) {
                                            if line.contains(r#""type":"USER_INPUT""#) {
                                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                                    if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                                                        let (sum, fp) = extract_clean_prompt(content);
                                                        if !sum.trim().is_empty() {
                                                            summary = sum;
                                                            full_prompt = fp;
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if summary.is_empty() {
                                        continue;
                                    }

                                    let dt = DateTime::from_timestamp(mtime as i64, 0)
                                        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    let info = SessionInfo {
                                        cid: cid.clone(),
                                        mtime,
                                        datetime: dt,
                                        summary,
                                        full_prompt,
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
        println!("No AGY sessions found.");
        return;
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));

    let mut search_query = String::new();
    let mut selected_idx = 0;
    let mut expanded_cid: Option<String> = None;

    let _ = enable_raw_mode();
    let mut out = stdout();
    let _ = execute!(out, EnterAlternateScreen, cursor::Hide);

    let result_cid = loop {
        let filtered: Vec<&SessionInfo> = sessions
            .iter()
            .filter(|s| {
                if search_query.is_empty() {
                    true
                } else {
                    let q = search_query.to_lowercase();
                    s.summary.to_lowercase().contains(&q)
                        || s.cid.to_lowercase().contains(&q)
                        || s.datetime.contains(&q)
                }
            })
            .collect();

        if selected_idx >= filtered.len() && !filtered.is_empty() {
            selected_idx = filtered.len() - 1;
        }

        // Render UI
        let _ = execute!(
            out,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        );

        println!("\x1b[1m\x1b[38;2;189;147;249m========================================================================\x1b[0m");
        println!(
            "\x1b[1m\x1b[38;2;139;233;253m🔍 Search AGY Session > \x1b[38;2;80;250;123m{}\x1b[0m",
            search_query
        );
        println!("\x1b[38;2;98;114;164m[ ↑↓: Move | Space/v: Expand Details | Enter: Resume | Esc: Exit ]\x1b[0m");
        println!("\x1b[1m\x1b[38;2;189;147;249m========================================================================\x1b[0m\n");

        if filtered.is_empty() {
            println!("  \x1b[38;2;255;85;85mNo matching sessions found.\x1b[0m");
        } else {
            let max_visible = 12;
            let start_idx = if selected_idx >= max_visible {
                selected_idx - max_visible + 1
            } else {
                0
            };
            let end_idx = (start_idx + max_visible).min(filtered.len());

            for idx in start_idx..end_idx {
                let s = filtered[idx];
                let is_selected = idx == selected_idx;
                let is_expanded = expanded_cid.as_ref() == Some(&s.cid);

                if is_selected {
                    println!(
                        " \x1b[38;2;80;250;123m▶\x1b[0m \x1b[1m\x1b[38;2;139;233;253m{}\x1b[0m │ \x1b[38;2;255;121;198m{}\x1b[0m │ \x1b[1m\x1b[38;2;248;248;242m{}\x1b[0m",
                        s.datetime, s.cid, s.summary
                    );
                } else {
                    println!(
                        "   \x1b[38;2;98;114;164m{}\x1b[0m │ \x1b[38;2;98;114;164m{}\x1b[0m │ \x1b[38;2;98;114;164m{}\x1b[0m",
                        s.datetime, s.cid, s.summary
                    );
                }

                if is_expanded {
                    println!("    \x1b[38;2;255;184;108m┌─ 🔍 FULL PROMPT DETAILS ──────────────────────────────────────────┐\x1b[0m");
                    println!("    \x1b[38;2;255;184;108m│\x1b[0m \x1b[1mCID:\x1b[0m \x1b[38;2;255;121;198m{}\x1b[0m", s.cid);
                    println!("    \x1b[38;2;255;184;108m│\x1b[0m \x1b[1mDate:\x1b[0m {}", s.datetime);
                    for p_line in s.full_prompt.lines() {
                        println!("    \x1b[38;2;255;184;108m│\x1b[0m {}", p_line);
                    }
                    println!("    \x1b[38;2;255;184;108m└──────────────────────────────────────────────────────────────────┘\x1b[0m");
                }
            }
        }

        let _ = out.flush();

        if let Ok(Event::Key(key_event)) = event::read() {
            match key_event.code {
                KeyCode::Esc => break None,
                KeyCode::Char('q') if key_event.modifiers.contains(KeyModifiers::CONTROL) => break None,
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => break None,
                KeyCode::Enter => {
                    if !filtered.is_empty() {
                        break Some(filtered[selected_idx].cid.clone());
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if selected_idx > 0 {
                        selected_idx -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !filtered.is_empty() && selected_idx + 1 < filtered.len() {
                        selected_idx += 1;
                    }
                }
                KeyCode::Char(' ') | KeyCode::Tab | KeyCode::Char('v') => {
                    if !filtered.is_empty() {
                        let cur_cid = &filtered[selected_idx].cid;
                        if expanded_cid.as_ref() == Some(cur_cid) {
                            expanded_cid = None;
                        } else {
                            expanded_cid = Some(cur_cid.clone());
                        }
                    }
                }
                KeyCode::Backspace => {
                    search_query.pop();
                    selected_idx = 0;
                }
                KeyCode::Char(c) => {
                    search_query.push(c);
                    selected_idx = 0;
                }
                _ => {}
            }
        }
    };

    let _ = execute!(out, cursor::Show, LeaveAlternateScreen);
    let _ = disable_raw_mode();

    if let Some(cid) = result_cid {
        println!("▶ Resuming session: \x1b[38;2;139;233;253m{}\x1b[0m", cid);
        let _ = Command::new("agy").args(["--conversation", &cid]).exec();
    } else {
        println!("Cancelled.");
    }
}
