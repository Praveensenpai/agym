//! Interactive Session history explorer and AGY conversation launcher.
//!
//! Provides the split-screen TUI interface for inspecting past Antigravity transcripts,
//! full-prompt previews, and resuming sessions via external `agy --conversation <cid>` execution.

use crate::session::{scan_sessions, SessionInfo};
use crate::ui::widgets::{
    filter_session_indices, format_bytes, format_session_detail, format_sessions_header,
    next_index, prev_index, style_dimmed, style_header, style_selected, style_warning,
    EVENT_POLL_TIMEOUT, HELP_SESSIONS_DEFAULT, TITLE_SESSION_DETAIL,
};
use crate::ui::{run_accounts_tui, TerminalGuard};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

/// Action to execute after exiting the Sessions TUI event loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionsOutcome {
    /// Exit the application cleanly.
    Quit,
    /// Transition into the Accounts management TUI.
    SwitchToAccounts,
    /// Launch external `agy` CLI to resume the specified conversation ID.
    ResumeSession(String),
}

/// State machine for the interactive Sessions TUI view.
pub struct SessionsApp {
    /// List of scanned session transcripts.
    pub sessions: Vec<SessionInfo>,
    /// Selection state for the Ratatui Table widget.
    pub state: TableState,
    /// Search filter query string.
    pub filter: String,
    /// Whether user is currently typing into search filter mode.
    pub searching: bool,
    /// Whether the detail preview pane is currently expanded.
    pub show_detail: bool,
}

impl SessionsApp {
    /// Scans session transcripts from disk and initializes table state.
    pub fn new() -> Self {
        let sessions = scan_sessions();
        let mut state = TableState::default();
        if !sessions.is_empty() {
            state.select(Some(0));
        }

        Self {
            sessions,
            state,
            filter: String::new(),
            searching: false,
            show_detail: false,
        }
    }

    /// Handles keyboard events, returning a `SessionsOutcome` when exiting the view.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        filtered_indices: &[usize],
    ) -> Option<SessionsOutcome> {
        if self.searching {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.searching = false;
                }
                KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => self.filter.push(c),
                _ => {}
            }
            return None;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(SessionsOutcome::Quit),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(SessionsOutcome::Quit)
            }
            KeyCode::Char('/') => {
                self.searching = true;
                None
            }
            KeyCode::Char(' ') | KeyCode::Char('v') => {
                self.show_detail = !self.show_detail;
                None
            }
            KeyCode::Char('a') | KeyCode::Tab => Some(SessionsOutcome::SwitchToAccounts),
            KeyCode::Enter => self
                .state
                .selected()
                .and_then(|i| filtered_indices.get(i))
                .and_then(|&real_idx| self.sessions.get(real_idx))
                .map(|s| SessionsOutcome::ResumeSession(s.cid.clone())),
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.select(Some(next_index(
                    self.state.selected(),
                    filtered_indices.len(),
                )));
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.select(Some(prev_index(
                    self.state.selected(),
                    filtered_indices.len(),
                )));
                None
            }
            _ => None,
        }
    }

    /// Renders header, session table, optional detail preview pane, and footer into the frame.
    pub fn render(&mut self, frame: &mut Frame, filtered_indices: &[usize]) {
        let layout_constraints = if self.show_detail {
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
            .split(frame.area());

        let header = Paragraph::new(format_sessions_header(filtered_indices.len(), &self.filter))
            .style(style_header())
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, chunks[0]);

        let rows: Vec<Row> = filtered_indices
            .iter()
            .filter_map(|&idx| self.sessions.get(idx))
            .map(|s| {
                Row::new(vec![
                    s.short_cid.clone(),
                    s.datetime.clone(),
                    format_bytes(s.size_bytes),
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

        frame.render_stateful_widget(table, chunks[1], &mut self.state);

        let footer_idx = if self.show_detail {
            let selected_session = self
                .state
                .selected()
                .and_then(|i| filtered_indices.get(i))
                .and_then(|&real_idx| self.sessions.get(real_idx));
            let detail_text = format_session_detail(selected_session);

            let detail_block = Paragraph::new(detail_text)
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(TITLE_SESSION_DETAIL)
                        .borders(Borders::ALL)
                        .border_style(style_warning()),
                );
            frame.render_widget(detail_block, chunks[2]);
            3
        } else {
            2
        };

        let status_text = if self.searching {
            format!(
                " Search: {} (Press Enter to confirm, Esc to clear)",
                self.filter
            )
        } else {
            HELP_SESSIONS_DEFAULT.to_string()
        };

        let footer = Paragraph::new(status_text)
            .style(style_dimmed())
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[footer_idx]);
    }
}

impl Default for SessionsApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs the interactive Sessions Explorer TUI event loop.
///
/// Handles session browsing, split detail previewing, filtering, and launches
/// `agy --conversation <cid>` after restoring standard terminal state.
pub fn run_sessions_tui() -> Result<()> {
    let mut guard = TerminalGuard::new()?;
    let mut app = SessionsApp::new();

    let outcome = loop {
        let filtered_indices = filter_session_indices(&app.sessions, &app.filter);

        guard
            .terminal_mut()
            .draw(|f| app.render(f, &filtered_indices))?;

        if event::poll(EVENT_POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                if let Some(outcome) = app.handle_key(key, &filtered_indices) {
                    break outcome;
                }
            }
        }
    };

    drop(guard);

    match outcome {
        SessionsOutcome::Quit => Ok(()),
        SessionsOutcome::SwitchToAccounts => run_accounts_tui(),
        SessionsOutcome::ResumeSession(cid) => {
            let status = std::process::Command::new("agy")
                .args(["--conversation", &cid])
                .status()
                .with_context(|| format!("Failed to execute 'agy --conversation {cid}'"))?;
            if !status.success() {
                eprintln!("'agy' exited with status: {status}");
            }
            Ok(())
        }
    }
}
