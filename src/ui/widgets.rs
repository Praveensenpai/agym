//! Shared UI styling, layout constants, pure navigation math, query filtering, and formatting helpers.
//!
//! Isolates all deterministic visual calculations and search algorithms from crossterm/terminal I/O,
//! enabling 100% automated headless unit testing.

use crate::account::AccountInfo;
use crate::quota::AccountQuotaInfo;
use crate::session::SessionInfo;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::time::Duration;

// --- Timing & Layout Constants ---

/// Minimum duration in seconds between manual background quota refreshes to prevent rate-limiting.
pub const REFRESH_COOLDOWN_SECS: u64 = 15;
/// Duration for the refresh cooldown interval.
pub const REFRESH_COOLDOWN: Duration = Duration::from_secs(REFRESH_COOLDOWN_SECS);
/// Duration for transient cooldown alert display.
pub const COOLDOWN_DISPLAY_DURATION: Duration = Duration::from_secs(3);
/// Polling timeout for crossterm keyboard event reading (yielding 20 FPS refresh rate).
pub const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(50);

/// Status badge text for active account.
pub const STATUS_ACTIVE_BADGE: &str = "* ACTIVE";
/// Status badge text for inactive account.
pub const STATUS_INACTIVE_BADGE: &str = "  INACTIVE";
/// Default shortcuts help text for the Accounts management view.
pub const HELP_ACCOUNTS_DEFAULT: &str =
    " [Enter] Switch | [s] Sessions | [n] New Log | [d] Delete | [r] Refresh | [/] Filter | [q] Quit";
/// Help text displayed in the Accounts footer while background quota refresh is executing.
pub const HELP_ACCOUNTS_REFRESHING: &str =
    " ⏳ Fetching live account quotas in background... Navigate freely with [↑/↓]";
/// Help text displayed when user attempts to refresh while a background refresh is already running.
pub const HELP_ACCOUNTS_ALREADY_RUNNING: &str = " ⏳ Refresh is already running in background...";
/// Default shortcuts help text for the Session Explorer view.
pub const HELP_SESSIONS_DEFAULT: &str =
    " [Enter] Resume | [Space/v] Toggle Preview | [a] Accounts | [/] Filter | [q] Quit";
/// Help text shown while the switch-account confirmation overlay is active.
pub const HELP_SESSIONS_CONFIRM: &str =
    " [←/→ or Tab] Select | [Enter] Confirm | [s] Switch | [c] Copy | [Esc] Cancel";
/// Title displayed on the Session Detail Preview pane border.
pub const TITLE_SESSION_DETAIL: &str = " 🔍 Session Detail Preview ";
/// Title displayed on the switch-account confirmation overlay border.
pub const TITLE_SESSION_CONFIRM: &str = " ⚠ Resume Different Account ";
/// Label for the switch-account button.
pub const BUTTON_SWITCH_LABEL: &str = " Switch ";
/// Label for the copy-and-continue button.
pub const BUTTON_COPY_LABEL: &str = " Copy & continue ";
/// Label for the cancel button.
pub const BUTTON_CANCEL_LABEL: &str = " Cancel ";
/// Number of buttons in the resume confirmation overlay.
pub const RESUME_BUTTON_COUNT: usize = 3;

// --- Styling Helpers ---

/// Returns the standard style for header titles and active table column headers (Cyan, Bold).
#[must_use]
pub fn style_header() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Returns the highlight style for the currently selected table row (Black on Cyan, Bold).
#[must_use]
pub fn style_selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Returns the dimmed style for secondary footer help text and inactive metadata (Dark Gray).
#[must_use]
pub fn style_dimmed() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Returns the style for warning banners, refresh alerts, and cooldown notifications (Yellow, Bold).
#[must_use]
pub fn style_warning() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

// --- Pure Navigation Math ---

/// Computes the next selected row index with wrap-around and empty-list protection.
#[must_use]
pub fn next_index(current: Option<usize>, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    match current {
        Some(i) if i < total.saturating_sub(1) => i + 1,
        _ => 0,
    }
}

/// Computes the previous selected row index with wrap-around and empty-list protection.
#[must_use]
pub fn prev_index(current: Option<usize>, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let last_idx = total.saturating_sub(1);
    match current {
        Some(0) | None => last_idx,
        Some(i) if i > last_idx => last_idx,
        Some(i) => i - 1,
    }
}

// --- Pure Search Filtering ---

/// Performs case-insensitive substring matching between a query and a target string.
#[must_use]
pub fn matches_filter(query: &str, target: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return true;
    }
    target.to_lowercase().contains(&trimmed.to_lowercase())
}

/// Filters a list of accounts returning the indices of accounts matching the email search query.
#[must_use]
pub fn filter_account_indices(accounts: &[AccountInfo], query: &str) -> Vec<usize> {
    accounts
        .iter()
        .enumerate()
        .filter_map(|(idx, acc)| {
            if matches_filter(query, &acc.email) {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

/// Filters a list of sessions returning the indices of sessions matching CID, short CID, or summary.
#[must_use]
pub fn filter_session_indices(sessions: &[SessionInfo], query: &str) -> Vec<usize> {
    sessions
        .iter()
        .enumerate()
        .filter_map(|(idx, s)| {
            if matches_filter(query, &s.cid)
                || matches_filter(query, &s.short_cid)
                || matches_filter(query, &s.summary)
            {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

// --- Pure Presentation Helpers ---

/// Returns the status string representation for active/inactive accounts.
#[must_use]
pub fn account_status_label(is_active: bool) -> &'static str {
    if is_active {
        STATUS_ACTIVE_BADGE
    } else {
        STATUS_INACTIVE_BADGE
    }
}

#[path = "widgets/quota.rs"]
pub mod quota;

pub use quota::{format_quota_badge, format_quota_cell_line};

pub use crate::session::format_bytes;

/// Formats the header banner text for the Accounts management view.
#[must_use]
pub fn format_accounts_header(total: usize, active_email: &str, is_refreshing: bool) -> String {
    let refresh_tag = if is_refreshing {
        " | Refreshing... ⏳"
    } else {
        ""
    };
    format!(" 🤖 AGYM — Antigravity Accounts ({total}) | Active: {active_email}{refresh_tag}")
}

/// Formats the header banner text for the Session Explorer view.
#[must_use]
pub fn format_sessions_header(total: usize, filter: &str) -> String {
    let filter_display = if filter.trim().is_empty() {
        "None"
    } else {
        filter.trim()
    };
    format!(" 💬 AGYM — Session Explorer ({total}) | Filter: {filter_display}")
}

/// Formats the detailed preview text for a selected session.
#[must_use]
pub fn format_session_detail(session: Option<&SessionInfo>) -> String {
    match session {
        Some(s) => format!(
            "ID: {}\nAccount: {}\nDate: {}\nSize: {} | Lines: {}\n\n{}",
            s.cid,
            s.account,
            s.datetime,
            format_bytes(s.size_bytes),
            s.line_count,
            s.full_prompt
        ),
        None => "No session selected.".to_string(),
    }
}

/// Formats the confirmation overlay text shown when a session belongs to a
/// different account than the currently active one.
#[must_use]
pub fn format_resume_prompt(
    session: &SessionInfo,
    active_account: &str,
    quota: Option<&AccountQuotaInfo>,
) -> String {
    let quota_line = match quota {
        Some(q) => format!("\nRemaining quota: {}", q.display_badge()),
        None => "\nRemaining quota: unavailable".to_string(),
    };
    let exhausted = quota.is_some_and(|q| {
        q.gemini_percent.unwrap_or(100) == 0 && q.claude_percent.unwrap_or(100) == 0
    });
    let exhausted_line = if exhausted {
        "\n\n⚠ Warning: this account appears to have no quota left."
    } else {
        ""
    };

    format!(
        "This conversation belongs to account '{}'.\n\nActive account: '{}'.\n{}{}\n\nSwitch to '{}', or copy it here and continue?",
        session.account, active_account, quota_line, exhausted_line, session.account
    )
}

/// Builds the styled three-button line (`Switch`, `Copy & continue`, `Cancel`),
/// highlighting the focused button by index.
#[must_use]
pub fn format_resume_buttons(focused: usize) -> Line<'static> {
    let buttons = [
        (BUTTON_SWITCH_LABEL, Color::Green),
        (BUTTON_COPY_LABEL, Color::Cyan),
        (BUTTON_CANCEL_LABEL, Color::Red),
    ];
    let mut spans = Vec::with_capacity(buttons.len() * 2);
    for (i, (label, color)) in buttons.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(
            format!("  {label}  "),
            button_style(i == focused, *color),
        ));
    }
    Line::from(spans)
}

/// Cycles the button focus index left or right within the overlay button count.
#[must_use]
pub fn cycle_button_focus(current: usize, forward: bool) -> usize {
    if forward {
        (current + 1) % RESUME_BUTTON_COUNT
    } else {
        (current + RESUME_BUTTON_COUNT - 1) % RESUME_BUTTON_COUNT
    }
}

/// Returns the button style for the given focus state and base color.
#[must_use]
fn button_style(focused: bool, color: Color) -> Style {
    if focused {
        Style::default()
            .fg(Color::Black)
            .bg(color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }
}

/// Formats the 15-second refresh cooldown remaining notification message.
#[must_use]
pub fn format_refresh_cooldown(remaining_secs: u64) -> String {
    format!(" ⏳ Refresh on 15s cooldown. Please wait {remaining_secs}s before refreshing again.")
}

#[cfg(test)]
#[path = "widgets_tests.rs"]
mod tests;
