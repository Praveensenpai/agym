use anyhow::Result;
use chrono::{DateTime, Local};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;
use colored::*;

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

pub fn sanitize_prompt(raw: &str) -> String {
    let stripped = raw.replace("<USER_REQUEST>", "").replace("</USER_REQUEST>", "");
    let cleaned = stripped
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("<ADDITIONAL_METADATA>")
                && !l.starts_with("</ADDITIONAL_METADATA>")
                && !l.starts_with("<USER_SETTINGS_CHANGE>")
                && !l.starts_with("The current local time is:")
                && !l.starts_with("The user changed setting")
        })
        .collect::<Vec<&str>>()
        .join(" ");

    let trimmed = cleaned.trim();
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
                let mut full_prompt = "New Conversation".to_string();

                for line in content.lines() {
                    if line.contains("\"USER_INPUT\"") || line.contains("\"type\":\"USER_INPUT\"") {
                        if let Ok(json) = serde_json::from_str::<Value>(line) {
                            if let Some(text) = json.get("content").and_then(|c| c.as_str()) {
                                let sanitized = sanitize_prompt(text);
                                if sanitized != "New Conversation" {
                                    full_prompt = text.to_string();
                                    break;
                                }
                            }
                        }
                    }
                }

                let summary = sanitize_prompt(&full_prompt);

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
                    full_prompt,
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

pub fn pick_and_resume_session() -> Result<()> {
    let sessions = scan_sessions();
    if sessions.is_empty() {
        println!("{}", "No Antigravity session history found.".yellow());
        return Ok(());
    }

    enable_raw_mode()?;
    let mut out = stdout();
    let _ = execute!(out, EnterAlternateScreen, cursor::Hide);

    let mut selected_idx = 0;
    let mut search_query = String::new();
    let mut expanded_cid: Option<String> = None;

    let result_session_opt = loop {
        let (cols, term_rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let cols_usize = cols as usize;

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
                        || s.profile.as_ref().map_or(false, |p| p.to_lowercase().contains(&q))
                }
            })
            .collect();

        if selected_idx >= filtered.len() && !filtered.is_empty() {
            selected_idx = filtered.len() - 1;
        }

        let _ = execute!(
            out,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        );

        let count_str = format!("({}/{} sessions)", filtered.len(), sessions.len());
        let sep = "─".repeat(cols_usize.min(120));
        print!("\x1b[38;2;189;147;249m{}\x1b[0m\r\n", sep);
        print!(
            "\x1b[1m\x1b[38;2;139;233;253m🔍 Search AGY Session \x1b[38;2;98;114;164m{}\x1b[38;2;139;233;253m > \x1b[38;2;80;250;123m{}\x1b[0m\r\n",
            count_str, search_query
        );
        print!("\x1b[38;2;98;114;164m[ ↑↓: Move | Space/v: Details | Enter: Resume | Esc: Exit ]\x1b[0m\r\n");
        print!("\x1b[38;2;189;147;249m{}\x1b[0m\r\n\r\n", sep);

        if filtered.is_empty() {
            print!("  \x1b[38;2;255;85;85mNo matching sessions found.\x1b[0m\r\n");
        } else {
            let avail_height = (term_rows as usize).saturating_sub(6).max(3);

            let get_item_height = |idx: usize| -> usize {
                let s = filtered[idx];
                if expanded_cid.as_ref() == Some(&s.cid) {
                    let p_lines = s.full_prompt.lines().take(6).count();
                    let prof_line = if s.profile.is_some() { 1 } else { 0 };
                    1 + 4 + prof_line + p_lines + 1
                } else {
                    1
                }
            };

            let mut start_idx = selected_idx;
            let mut h_acc = get_item_height(selected_idx);
            while start_idx > 0 {
                let prev_h = get_item_height(start_idx - 1);
                if h_acc + prev_h > avail_height {
                    break;
                }
                start_idx -= 1;
                h_acc += prev_h;
            }

            let avail_prompt_width = cols_usize.saturating_sub(46).max(15);
            let mut rendered_height = 0;

            for idx in start_idx..filtered.len() {
                let item_h = get_item_height(idx);
                if rendered_height > 0 && rendered_height + item_h > avail_height {
                    break;
                }
                rendered_height += item_h;

                let s = filtered[idx];
                let is_selected = idx == selected_idx;
                let is_expanded = expanded_cid.as_ref() == Some(&s.cid);

                let trunc_summary = if s.summary.chars().count() > avail_prompt_width {
                    let text: String = s.summary.chars().take(avail_prompt_width.saturating_sub(3)).collect();
                    format!("{}...", text)
                } else {
                    s.summary.clone()
                };

                let padded_size = format!("{:>8}", s.size_fmt);

                if is_selected {
                    print!(
                        " \x1b[38;2;80;250;123m▶\x1b[0m \x1b[1m\x1b[38;2;139;233;253m{}\x1b[0m │ \x1b[38;2;255;121;198m{}\x1b[0m │ \x1b[38;2;241;250;140m{}\x1b[0m │ \x1b[1m\x1b[38;2;248;248;242m{}\x1b[0m\r\n",
                        s.datetime, s.short_cid, padded_size, trunc_summary
                    );
                } else {
                    print!(
                        "   \x1b[38;2;98;114;164m{}\x1b[0m │ \x1b[38;2;98;114;164m{}\x1b[0m │ \x1b[38;2;98;114;164m{}\x1b[0m │ \x1b[38;2;98;114;164m{}\x1b[0m\r\n",
                        s.datetime, s.short_cid, padded_size, trunc_summary
                    );
                }

                if is_expanded {
                    let box_w = cols_usize.saturating_sub(6).min(100);
                    let top_bar = format!("┌─ 🔍 FULL SESSION DETAILS ({}) {}", s.size_fmt, "─".repeat(box_w.saturating_sub(35)));
                    print!("    \x1b[38;2;255;184;108m{}\x1b[0m\r\n", top_bar);
                    print!("    \x1b[38;2;255;184;108m│\x1b[0m \x1b[1mFull CID:\x1b[0m \x1b[38;2;255;121;198m{}\x1b[0m\r\n", s.cid);
                    if let Some(ref prof) = s.profile {
                        print!("    \x1b[38;2;255;184;108m│\x1b[0m \x1b[1mAccount Profile:\x1b[0m \x1b[38;2;80;250;123m{}\x1b[0m\r\n", prof);
                    }
                    print!("    \x1b[38;2;255;184;108m│\x1b[0m \x1b[1mDate:\x1b[0m {}  (\x1b[38;2;241;250;140m{} bytes\x1b[0m, {} lines)\r\n", s.datetime, s.size_bytes, s.line_count);
                    print!("    \x1b[38;2;255;184;108m│\x1b[0m \x1b[1mPrompt:\x1b[0m\r\n");
                    for p_line in s.full_prompt.lines().take(6) {
                        print!("    \x1b[38;2;255;184;108m│\x1b[0m   {}\r\n", p_line);
                    }
                    let bot_bar = "└".to_string() + &"─".repeat(box_w.saturating_sub(1));
                    print!("    \x1b[38;2;255;184;108m{}\x1b[0m\r\n", bot_bar);
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
                        break Some(filtered[selected_idx].clone());
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

    if let Some(selected_session) = result_session_opt {
        if let Some(ref prof) = selected_session.profile {
            crate::account::set_active_account(prof);
        }
        println!("🚀 Resuming AGY session {}...", selected_session.cid.cyan());
        let mut child = Command::new("agy")
            .args(["resume", &selected_session.cid])
            .spawn()?;
        let _ = child.wait();
    }

    Ok(())
}
