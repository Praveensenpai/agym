//! Interactive Accounts table management view and background quota worker.
//!
//! Provides the primary TUI table listing saved Antigravity profiles, active status badges,
//! live quota meters, and asynchronous background quota refresh worker thread communication.

use crate::account::{
    interactive_remove_account, list_account_infos, list_account_infos_cached, prepare_new_session,
    set_active_account, AccountInfo,
};
use crate::ui::widgets::{
    account_status_label, filter_account_indices, format_accounts_header, format_quota_badge,
    format_refresh_cooldown, next_index, prev_index, quota_color, style_dimmed, style_header,
    style_selected, style_warning, COOLDOWN_DISPLAY_DURATION, EVENT_POLL_TIMEOUT,
    HELP_ACCOUNTS_ALREADY_RUNNING, HELP_ACCOUNTS_DEFAULT, HELP_ACCOUNTS_REFRESHING,
    REFRESH_COOLDOWN, REFRESH_COOLDOWN_SECS,
};
use crate::ui::{run_sessions_tui, TerminalGuard};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;

/// Result payload from background quota refreshes: `(accounts_list, is_fresh_data)`.
pub type AccountsRefreshResult = (Vec<AccountInfo>, bool);

/// Sender half for background quota refresh worker updates.
pub type QuotaSender = Sender<AccountsRefreshResult>;

/// Receiver half for background quota refresh worker updates.
pub type QuotaReceiver = Receiver<AccountsRefreshResult>;

/// Action to execute after exiting the Accounts TUI event loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountsOutcome {
    /// Exit the application cleanly.
    Quit,
    /// Transition into the Session Explorer TUI.
    SwitchToSessions,
    /// Prepare a new login session by wiping current keyring token.
    PrepareNewSession,
    /// Prompt interactively to remove a saved account.
    InteractiveRemove,
    /// Switch active Antigravity profile to the specified account email.
    SwitchAccount(String),
}

/// State machine for the interactive Accounts TUI view.
pub struct AccountsApp {
    /// Selection state for the Ratatui Table widget.
    pub state: TableState,
    /// List of saved Antigravity accounts.
    pub accounts: Vec<AccountInfo>,
    /// Search filter query string.
    pub filter: String,
    /// Whether user is currently typing into search filter mode.
    pub searching: bool,
    /// Whether a background quota refresh thread is actively running.
    pub is_refreshing: bool,
    /// Timestamp of last initiated quota refresh for cooldown enforcement.
    pub last_refresh_time: Option<Instant>,
    /// Transient message banner with expiration timestamp.
    pub cooldown_msg: Option<(String, Instant)>,
    /// Channel sender to dispatch fresh account quota results.
    pub tx: QuotaSender,
    /// Channel receiver to collect fresh account quota results from worker thread.
    pub rx: QuotaReceiver,
}

impl AccountsApp {
    /// Initializes application state, loading cached accounts and initiating a background
    /// refresh worker thread if the local cache is absent or expired.
    pub fn new() -> Self {
        let mut state = TableState::default();
        let accounts = list_account_infos_cached();
        let has_valid_cache = !accounts.is_empty()
            && accounts
                .iter()
                .all(|a| a.quota.as_ref().is_some_and(|q| !q.is_expired()));

        let (tx, rx): (QuotaSender, QuotaReceiver) = channel();
        if !accounts.is_empty() {
            state.select(Some(0));
        }

        if !has_valid_cache {
            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                let _ = tx_clone.send((list_account_infos(false), true));
            });
        }

        Self {
            state,
            accounts,
            filter: String::new(),
            searching: false,
            is_refreshing: !has_valid_cache,
            last_refresh_time: None,
            cooldown_msg: None,
            tx,
            rx,
        }
    }

    /// Polls background channel receiver for refreshed account quota payloads.
    pub fn poll_updates(&mut self) {
        if let Ok((fresh_accounts, is_fresh)) = self.rx.try_recv() {
            self.accounts = fresh_accounts;
            if is_fresh {
                self.is_refreshing = false;
            }
            if self.state.selected().is_none() && !self.accounts.is_empty() {
                self.state.select(Some(0));
            }
        }
    }

    /// Spawns a background thread to fetch live quotas from the API.
    fn trigger_refresh_worker(&mut self) {
        self.is_refreshing = true;
        self.last_refresh_time = Some(Instant::now());
        let tx_clone = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx_clone.send((list_account_infos(true), true));
        });
    }

    /// Requests a quota refresh, enforcing a 15-second cooldown between API requests.
    pub fn request_refresh(&mut self) {
        let now = Instant::now();
        if self.is_refreshing {
            self.cooldown_msg = Some((
                HELP_ACCOUNTS_ALREADY_RUNNING.to_string(),
                now + COOLDOWN_DISPLAY_DURATION,
            ));
        } else if let Some(last_time) = self.last_refresh_time {
            let elapsed = last_time.elapsed();
            if elapsed < REFRESH_COOLDOWN {
                let remaining = REFRESH_COOLDOWN_SECS.saturating_sub(elapsed.as_secs());
                self.cooldown_msg = Some((
                    format_refresh_cooldown(remaining),
                    now + COOLDOWN_DISPLAY_DURATION,
                ));
            } else {
                self.trigger_refresh_worker();
            }
        } else {
            self.trigger_refresh_worker();
        }
    }

    /// Handles keyboard events, returning an `AccountsOutcome` when exiting the view.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        filtered_indices: &[usize],
    ) -> Option<AccountsOutcome> {
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
            KeyCode::Char('q') | KeyCode::Esc => Some(AccountsOutcome::Quit),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(AccountsOutcome::Quit)
            }
            KeyCode::Char('/') => {
                self.searching = true;
                None
            }
            KeyCode::Char('r') => {
                self.request_refresh();
                None
            }
            KeyCode::Char('n') => Some(AccountsOutcome::PrepareNewSession),
            KeyCode::Char('d') | KeyCode::Delete => Some(AccountsOutcome::InteractiveRemove),
            KeyCode::Char('s') | KeyCode::Tab => Some(AccountsOutcome::SwitchToSessions),
            KeyCode::Enter => self
                .state
                .selected()
                .and_then(|i| filtered_indices.get(i))
                .and_then(|&idx| self.accounts.get(idx))
                .map(|acc| AccountsOutcome::SwitchAccount(acc.email.clone())),
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

    /// Renders header, accounts table with quota badges, and footer banner into the frame.
    pub fn render(&mut self, frame: &mut Frame, filtered_indices: &[usize]) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let active_acc = self
            .accounts
            .iter()
            .find(|a| a.is_active)
            .map(|a| a.email.as_str())
            .unwrap_or("None");

        let header = Paragraph::new(format_accounts_header(
            self.accounts.len(),
            active_acc,
            self.is_refreshing,
        ))
        .style(style_header())
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, chunks[0]);

        let rows: Vec<Row> = if self.accounts.is_empty() {
            vec![Row::new(vec![
                Cell::from(""),
                Cell::from("⏳ Loading accounts…"),
                Cell::from("Please wait"),
            ])]
        } else {
            filtered_indices
                .iter()
                .filter_map(|&idx| self.accounts.get(idx))
                .map(|acc| {
                    let status_cell = Cell::from(account_status_label(acc.is_active));
                    let email_cell = Cell::from(acc.email.clone());
                    let quota_str = format_quota_badge(acc.quota.as_ref());
                    let quota_cell = if let Some(q) = &acc.quota {
                        let pct = q.gemini_percent.or(q.top_model_percent).unwrap_or(0);
                        Cell::from(quota_str).style(Style::default().fg(quota_color(pct)))
                    } else {
                        Cell::from(quota_str).style(style_dimmed())
                    };
                    Row::new(vec![status_cell, email_cell, quota_cell])
                })
                .collect()
        };

        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Length(30),
                Constraint::Min(30),
            ],
        )
        .header(Row::new(vec!["Status", "Account Email", "Quota Metrics"]).style(style_header()))
        .block(Block::default().borders(Borders::ALL))
        .row_highlight_style(style_selected());

        frame.render_stateful_widget(table, chunks[1], &mut self.state);

        let (status_text, footer_style) = if let Some((ref msg, show_until)) = self.cooldown_msg {
            if Instant::now() < show_until {
                (msg.clone(), style_warning())
            } else {
                self.cooldown_msg = None;
                (HELP_ACCOUNTS_DEFAULT.to_string(), style_dimmed())
            }
        } else if self.is_refreshing {
            (HELP_ACCOUNTS_REFRESHING.to_string(), style_warning())
        } else if self.searching {
            (
                format!(
                    " Search: {} (Press Enter to confirm, Esc to clear)",
                    self.filter
                ),
                style_header(),
            )
        } else {
            (HELP_ACCOUNTS_DEFAULT.to_string(), style_dimmed())
        };

        let footer = Paragraph::new(status_text)
            .style(footer_style)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[2]);
    }
}

impl Default for AccountsApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs the interactive Accounts management TUI event loop.
pub fn run_accounts_tui() -> Result<()> {
    let mut guard = TerminalGuard::new()?;
    let mut app = AccountsApp::new();

    let outcome = loop {
        app.poll_updates();
        let filtered_indices = filter_account_indices(&app.accounts, &app.filter);

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
        AccountsOutcome::Quit => Ok(()),
        AccountsOutcome::SwitchToSessions => run_sessions_tui(),
        AccountsOutcome::PrepareNewSession => prepare_new_session(),
        AccountsOutcome::InteractiveRemove => interactive_remove_account(),
        AccountsOutcome::SwitchAccount(email) => set_active_account(&email),
    }
}
