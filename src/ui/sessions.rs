//! Interactive Session history explorer and AGY conversation launcher.
//!
//! Provides the split-screen TUI interface for inspecting past Antigravity transcripts,
//! full-prompt previews, and resuming sessions via external `agy --conversation <cid>` execution.

mod confirm;
mod table;

use crate::account::{current_active_email, email_prefix, set_active_account};
use crate::session::{import_conversation, scan_sessions, SessionInfo};
use crate::ui::widgets::{
    cycle_button_focus, filter_session_indices, next_index, prev_index, style_dimmed,
    EVENT_POLL_TIMEOUT, HELP_SESSIONS_CONFIRM, HELP_SESSIONS_DEFAULT, RESUME_BUTTON_COUNT,
};
use crate::ui::{run_accounts_tui, TerminalGuard};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    widgets::{Block, Borders, Paragraph, TableState},
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
    /// Switch to the owning account, then resume the conversation.
    SwitchAndResume { account: String, cid: String },
    /// Copy the conversation into the active account, then resume the copy.
    CopyAndResume { account: String, cid: String },
}

/// State for the account-switch confirmation overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmState {
    /// Index into `SessionsApp::sessions` for the session to resume.
    pub session_idx: usize,
    /// Focused button index (`0` = Switch, `1` = Copy, `2` = Cancel).
    pub focused: usize,
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
    /// Pending switch confirmation overlay state, if visible.
    pub pending_switch: Option<ConfirmState>,
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
            pending_switch: None,
        }
    }

    /// Returns the currently active account prefix (email local-part), or empty if unknown.
    pub fn active_account() -> String {
        current_active_email()
            .map(|e| email_prefix(&e).to_string())
            .unwrap_or_default()
    }

    /// Determines whether the selected session needs an account switch before resuming.
    ///
    /// Returns `Some(outcome)` immediately when resumable on the active account,
    /// otherwise records a pending confirmation and returns `None`.
    fn request_resume(&mut self, filtered_indices: &[usize]) -> Option<SessionsOutcome> {
        let real_idx = self
            .state
            .selected()
            .and_then(|i| filtered_indices.get(i))
            .copied()?;
        let session = self.sessions.get(real_idx)?;
        let active = Self::active_account();

        if session.account == "default" || session.account == active {
            return Some(SessionsOutcome::ResumeSession(session.cid.clone()));
        }

        self.pending_switch = Some(ConfirmState {
            session_idx: real_idx,
            focused: 0,
        });
        None
    }

    /// Activates the focused confirmation button, producing the matching outcome.
    fn confirm_activate(&mut self) -> Option<SessionsOutcome> {
        let state = self.pending_switch.take()?;
        let session = self.sessions.get(state.session_idx)?;
        match state.focused {
            0 => Some(SessionsOutcome::SwitchAndResume {
                account: session.account.clone(),
                cid: session.cid.clone(),
            }),
            1 => Some(SessionsOutcome::CopyAndResume {
                account: session.account.clone(),
                cid: session.cid.clone(),
            }),
            _ => None,
        }
    }

    /// Handles keys while the switch confirmation overlay is visible.
    fn handle_confirm_key(&mut self, key: KeyEvent) -> Option<SessionsOutcome> {
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.set_confirm_focus(0);
                self.confirm_activate()
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.set_confirm_focus(1);
                self.confirm_activate()
            }
            KeyCode::Enter => self.confirm_activate(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.pending_switch = None;
                None
            }
            KeyCode::Left | KeyCode::BackTab => {
                self.cycle_confirm_focus(false);
                None
            }
            KeyCode::Right | KeyCode::Tab => {
                self.cycle_confirm_focus(true);
                None
            }
            _ => None,
        }
    }

    /// Sets the focused button index, clamped to the valid range.
    fn set_confirm_focus(&mut self, index: usize) {
        if let Some(state) = self.pending_switch.as_mut() {
            state.focused = index.min(RESUME_BUTTON_COUNT - 1);
        }
    }

    /// Moves the overlay focus forward or backward with wrap-around.
    fn cycle_confirm_focus(&mut self, forward: bool) {
        if let Some(state) = self.pending_switch.as_mut() {
            state.focused = cycle_button_focus(state.focused, forward);
        }
    }

    /// Handles keyboard events, returning a `SessionsOutcome` when exiting the view.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        filtered_indices: &[usize],
    ) -> Option<SessionsOutcome> {
        if self.pending_switch.is_some() {
            return self.handle_confirm_key(key);
        }

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
            KeyCode::Enter => self.request_resume(filtered_indices),
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
        let chunks = table::layout_chunks(frame, self.show_detail);
        table::render_table(self, frame, &chunks, filtered_indices);
        let footer_idx = table::render_detail_pane(self, frame, &chunks, filtered_indices);

        let status_text = if self.pending_switch.is_some() {
            HELP_SESSIONS_CONFIRM.to_string()
        } else if self.searching {
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

        if let Some(state) = &self.pending_switch {
            if let Some(session) = self.sessions.get(state.session_idx) {
                confirm::render_confirm_overlay(frame, session, &Self::active_account(), state);
            }
        }
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
        SessionsOutcome::ResumeSession(cid) => launch_agy_conversation(&cid),
        SessionsOutcome::SwitchAndResume { account, cid } => {
            set_active_account(&account)?;
            launch_agy_conversation(&cid)
        }
        SessionsOutcome::CopyAndResume { account, cid } => {
            match import_conversation(&account, &cid) {
                Ok(new_cid) => {
                    println!("Copied conversation into active account as {new_cid}");
                    launch_agy_conversation(&new_cid)
                }
                Err(e) => {
                    eprintln!("Failed to copy conversation: {e}");
                    Ok(())
                }
            }
        }
    }
}

/// Executes `agy --conversation <cid>` in the restored terminal.
fn launch_agy_conversation(cid: &str) -> Result<()> {
    let status = std::process::Command::new("agy")
        .args(["--conversation", cid])
        .status()
        .with_context(|| format!("Failed to execute 'agy --conversation {cid}'"))?;
    if !status.success() {
        eprintln!("'agy' exited with status: {status}");
    }
    Ok(())
}
