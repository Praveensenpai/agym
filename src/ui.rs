use crate::account::{
    interactive_remove_account, list_account_infos, list_account_infos_cached, prepare_new_session,
    set_active_account,
};
use crate::session::scan_sessions;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap},
    Terminal,
};
use std::io;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

pub fn style_header() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn style_selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn style_dimmed() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn run_accounts_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = TableState::default();
    let mut filter = String::new();
    let mut searching = false;
    let mut is_refreshing = true;
    let mut last_refresh_time: Option<Instant> = None;
    let mut cooldown_msg: Option<(String, Instant)> = None;

    let mut accounts = Vec::new();

    let (tx, rx): (
        Sender<(Vec<crate::account::AccountInfo>, bool)>,
        Receiver<(Vec<crate::account::AccountInfo>, bool)>,
    ) = channel();

    if !accounts.is_empty() {
        state.select(Some(0));
    }

    // Load cache and live quotas away from the UI thread. The cache result is
    // displayed first; a live fetch only happens if the cache is expired (>5 min).
    let initial_tx = tx.clone();
    std::thread::spawn(move || {
        let cached = list_account_infos_cached();
        let _ = initial_tx.send((cached, false));
        let fresh = list_account_infos(false); // false = respect 5-min TTL
        let _ = initial_tx.send((fresh, true));
    });

    let res = loop {
        if let Ok((fresh_accounts, is_fresh)) = rx.try_recv() {
            accounts = fresh_accounts;
            if is_fresh {
                is_refreshing = false;
            }
            if state.selected().is_none() && !accounts.is_empty() {
                state.select(Some(0));
            }
        }

        let filtered_indices: Vec<usize> = accounts
            .iter()
            .enumerate()
            .filter_map(|(idx, acc)| {
                if filter.is_empty() || acc.email.to_lowercase().contains(&filter.to_lowercase()) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if let Err(e) = terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let active_acc = accounts
                .iter()
                .find(|a| a.is_active)
                .map(|a| a.email.as_str())
                .unwrap_or("None");

            let header_text = format!(
                " 🤖 AGYM — Antigravity Accounts ({}) | Active: {}{}",
                accounts.len(),
                active_acc,
                if is_refreshing { " | Refreshing... ⏳" } else { "" }
            );

            let header = Paragraph::new(header_text)
                .style(style_header())
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let rows: Vec<Row> = if accounts.is_empty() {
                vec![Row::new(vec![
                    "".to_string(),
                    "⏳ Loading accounts…".to_string(),
                    "Please wait".to_string(),
                ])]
            } else {
                filtered_indices.iter().map(|&idx| {
                    let acc = &accounts[idx];
                    let status = if acc.is_active {
                        "* ACTIVE".to_string()
                    } else {
                        "  INACTIVE".to_string()
                    };

                    let quota_str = acc
                        .quota
                        .as_ref()
                        .map(|q| q.display_badge())
                        .unwrap_or_else(|| "[quota unavailable]".to_string());

                    Row::new(vec![status, acc.email.clone(), quota_str])
                }).collect()
            };

            let table = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(30),
                    Constraint::Min(30),
                ],
            )
            .header(
                Row::new(vec!["Status", "Account Email", "Quota Metrics"])
                    .style(style_header()),
            )
            .block(Block::default().borders(Borders::ALL))
            .row_highlight_style(style_selected());

            f.render_stateful_widget(table, chunks[1], &mut state);

            let mut custom_status: Option<(String, Style)> = None;
            if let Some((ref msg, show_until)) = cooldown_msg {
                if Instant::now() < show_until {
                    custom_status = Some((
                        msg.clone(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ));
                } else {
                    cooldown_msg = None;
                }
            }

            let (status_text, footer_style) = if let Some((msg, style)) = custom_status {
                (msg, style)
            } else if is_refreshing {
                (
                    " ⏳ Fetching live account quotas in background... Navigate freely with [↑/↓]".to_string(),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )
            } else if searching {
                (
                    format!(" Search: {} (Press Enter to confirm, Esc to clear)", filter),
                    style_header(),
                )
            } else {
                (
                    " [Enter] Switch | [s] Sessions | [n] New Log | [d] Delete | [r] Refresh | [/] Filter | [q] Quit".to_string(),
                    style_dimmed(),
                )
            };

            let footer = Paragraph::new(status_text)
                .style(footer_style)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        }) {
            break Err(e.into());
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if searching {
                    match key.code {
                        KeyCode::Esc => {
                            filter.clear();
                            searching = false;
                        }
                        KeyCode::Enter => {
                            searching = false;
                        }
                        KeyCode::Backspace => {
                            filter.pop();
                        }
                        KeyCode::Char(c) => {
                            filter.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break Ok(())
                        }
                        KeyCode::Char('/') => {
                            searching = true;
                        }
                        KeyCode::Char('r') => {
                            let now = Instant::now();
                            if is_refreshing {
                                cooldown_msg = Some((
                                    " ⏳ Refresh is already running in background...".to_string(),
                                    now + Duration::from_secs(3),
                                ));
                            } else if let Some(last_time) = last_refresh_time {
                                let elapsed = last_time.elapsed();
                                if elapsed < Duration::from_secs(15) {
                                    let remaining = 15 - elapsed.as_secs();
                                    cooldown_msg = Some((
                                        format!(" ⏳ Refresh on 15s cooldown. Please wait {}s before refreshing again.", remaining),
                                        now + Duration::from_secs(3),
                                    ));
                                } else {
                                    is_refreshing = true;
                                    last_refresh_time = Some(now);
                                    let tx_clone = tx.clone();
                                    std::thread::spawn(move || {
                                        let fresh = list_account_infos(true);
                                        let _ = tx_clone.send((fresh, true));
                                    });
                                }
                            } else {
                                is_refreshing = true;
                                last_refresh_time = Some(now);
                                let tx_clone = tx.clone();
                                std::thread::spawn(move || {
                                    let fresh = list_account_infos(true);
                                    let _ = tx_clone.send((fresh, true));
                                });
                            }
                        }
                        KeyCode::Char('n') => {
                            disable_raw_mode()?;
                            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                            let _ = prepare_new_session();
                            return Ok(());
                        }
                        KeyCode::Char('d') | KeyCode::Delete => {
                            disable_raw_mode()?;
                            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                            let _ = interactive_remove_account();
                            return Ok(());
                        }
                        KeyCode::Char('s') | KeyCode::Tab => {
                            disable_raw_mode()?;
                            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                            return run_sessions_tui();
                        }
                        KeyCode::Enter => {
                            if let Some(i) = state.selected() {
                                if i < filtered_indices.len() {
                                    let real_idx = filtered_indices[i];
                                    let target_email = &accounts[real_idx].email;
                                    set_active_account(target_email);
                                    break Ok(());
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let i = match state.selected() {
                                Some(i) => {
                                    if filtered_indices.is_empty() {
                                        0
                                    } else if i >= filtered_indices.len() - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            state.select(Some(i));
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let i = match state.selected() {
                                Some(i) => {
                                    if filtered_indices.is_empty() {
                                        0
                                    } else if i == 0 {
                                        filtered_indices.len().saturating_sub(1)
                                    } else {
                                        i - 1
                                    }
                                }
                                None => 0,
                            };
                            state.select(Some(i));
                        }
                        _ => {}
                    }
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

pub fn run_sessions_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let sessions = scan_sessions();
    let mut state = TableState::default();
    let mut filter = String::new();
    let mut searching = false;
    let mut show_detail = false;

    if !sessions.is_empty() {
        state.select(Some(0));
    }

    let res = loop {
        let filtered_indices: Vec<usize> = sessions
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| {
                if filter.is_empty()
                    || s.cid.to_lowercase().contains(&filter.to_lowercase())
                    || s.summary.to_lowercase().contains(&filter.to_lowercase())
                {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if let Err(e) = terminal.draw(|f| {
            let layout_constraints = if show_detail {
                vec![
                    Constraint::Length(3),
                    Constraint::Percentage(50),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ]
            } else {
                vec![
                    Constraint::Length(3),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ]
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(layout_constraints)
                .split(f.area());

            let header_text = format!(
                " 💬 AGYM — Session Explorer ({}) | Filter: {}",
                filtered_indices.len(),
                if filter.is_empty() { "None" } else { &filter }
            );

            let header = Paragraph::new(header_text)
                .style(style_header())
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let rows: Vec<Row> = filtered_indices
                .iter()
                .map(|&idx| {
                    let s = &sessions[idx];
                    Row::new(vec![
                        s.short_cid.clone(),
                        s.datetime.clone(),
                        s.size_fmt.clone(),
                        format!("{} lines", s.line_count),
                        s.summary.clone(),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(10),
                    Constraint::Length(18),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Min(30),
                ],
            )
            .header(
                Row::new(vec!["CID", "Date/Time", "Size", "Lines", "Prompt Summary"])
                    .style(style_header()),
            )
            .block(Block::default().borders(Borders::ALL))
            .row_highlight_style(style_selected());

            f.render_stateful_widget(table, chunks[1], &mut state);

            let mut footer_idx = 2;

            if show_detail {
                footer_idx = 3;
                let detail_text = if let Some(i) = state.selected() {
                    if i < filtered_indices.len() {
                        let s = &sessions[filtered_indices[i]];
                        format!(
                            "ID: {}\nDate: {}\nSize: {} | Lines: {}\n\n{}",
                            s.cid, s.datetime, s.size_fmt, s.line_count, s.full_prompt
                        )
                    } else {
                        "No session selected.".to_string()
                    }
                } else {
                    "No session selected.".to_string()
                };

                let detail_block = Paragraph::new(detail_text)
                    .wrap(Wrap { trim: false })
                    .block(
                        Block::default()
                            .title(" 🔍 Session Detail Preview ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Yellow)),
                    );
                f.render_widget(detail_block, chunks[2]);
            }

            let status_text = if searching {
                format!(" Search: {} (Press Enter to confirm, Esc to clear)", filter)
            } else {
                format!(
                    " [Enter] Resume | [Space/v] Toggle Preview | [a] Accounts | [/] Filter | [q] Quit"
                )
            };

            let footer = Paragraph::new(status_text)
                .style(style_dimmed())
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[footer_idx]);
        }) {
            break Err(e.into());
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if searching {
                    match key.code {
                        KeyCode::Esc => {
                            filter.clear();
                            searching = false;
                        }
                        KeyCode::Enter => {
                            searching = false;
                        }
                        KeyCode::Backspace => {
                            filter.pop();
                        }
                        KeyCode::Char(c) => {
                            filter.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break Ok(())
                        }
                        KeyCode::Char('/') => {
                            searching = true;
                        }
                        KeyCode::Char(' ') | KeyCode::Char('v') => {
                            show_detail = !show_detail;
                        }
                        KeyCode::Char('a') | KeyCode::Tab => {
                            disable_raw_mode()?;
                            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                            return run_accounts_tui();
                        }
                        KeyCode::Enter => {
                            if let Some(i) = state.selected() {
                                if i < filtered_indices.len() {
                                    let real_idx = filtered_indices[i];
                                    let target_cid = &sessions[real_idx].cid;
                                    disable_raw_mode()?;
                                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                                    std::process::Command::new("agy")
                                        .args(["resume", target_cid])
                                        .status()?;
                                    return Ok(());
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let i = match state.selected() {
                                Some(i) => {
                                    if filtered_indices.is_empty() {
                                        0
                                    } else if i >= filtered_indices.len() - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            state.select(Some(i));
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let i = match state.selected() {
                                Some(i) => {
                                    if filtered_indices.is_empty() {
                                        0
                                    } else if i == 0 {
                                        filtered_indices.len().saturating_sub(1)
                                    } else {
                                        i - 1
                                    }
                                }
                                None => 0,
                            };
                            state.select(Some(i));
                        }
                        _ => {}
                    }
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}
