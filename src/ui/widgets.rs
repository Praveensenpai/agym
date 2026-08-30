//! Shared UI styling, layout constants, pure navigation math, query filtering, and formatting helpers.
//!
//! Isolates all deterministic visual calculations and search algorithms from crossterm/terminal I/O,
//! enabling 100% automated headless unit testing.

use crate::account::AccountInfo;
use crate::quota::AccountQuotaInfo;
use crate::session::SessionInfo;
use ratatui::style::{Color, Modifier, Style};
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
/// Title displayed on the Session Detail Preview pane border.
pub const TITLE_SESSION_DETAIL: &str = " 🔍 Session Detail Preview ";

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

/// Returns the color indicator corresponding to a quota percentage (>=50% Green, 20-49% Yellow, <20% Red).
#[must_use]
pub fn quota_color(percent: u32) -> Color {
    match percent {
        p if p >= 50 => Color::Green,
        p if p >= 20 => Color::Yellow,
        _ => Color::Red,
    }
}

/// Formats the quota badge text with a safe fallback when quota info is missing.
#[must_use]
pub fn format_quota_badge(quota: Option<&AccountQuotaInfo>) -> String {
    quota
        .map(|q| q.display_badge())
        .unwrap_or_else(|| "[quota unavailable]".to_string())
}

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
            "ID: {}\nDate: {}\nSize: {} | Lines: {}\n\n{}",
            s.cid,
            s.datetime,
            format_bytes(s.size_bytes),
            s.line_count,
            s.full_prompt
        ),
        None => "No session selected.".to_string(),
    }
}

/// Formats the 15-second refresh cooldown remaining notification message.
#[must_use]
pub fn format_refresh_cooldown(remaining_secs: u64) -> String {
    format!(" ⏳ Refresh on 15s cooldown. Please wait {remaining_secs}s before refreshing again.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountInfo;
    use crate::quota::AccountQuotaInfo;
    use crate::session::SessionInfo;
    use std::path::PathBuf;

    fn make_acc(email: &str, is_active: bool) -> AccountInfo {
        AccountInfo {
            email: email.to_string(),
            is_active,
            quota: None,
            file_path: PathBuf::from(format!("{email}.json")),
        }
    }

    fn make_sess(cid: &str, short_cid: &str, summary: &str) -> SessionInfo {
        SessionInfo {
            cid: cid.to_string(),
            short_cid: short_cid.to_string(),
            datetime: "2026-08-30 10:00".to_string(),
            timestamp: 1700000000,
            size_bytes: 1024,
            line_count: 50,
            summary: summary.to_string(),
            full_prompt: format!("Prompt for {cid}"),
        }
    }

    #[test]
    fn test_navigation_math() {
        assert_eq!(next_index(None, 0), 0);
        assert_eq!(next_index(Some(0), 0), 0);
        assert_eq!(next_index(None, 1), 0);
        assert_eq!(next_index(Some(0), 1), 0);
        assert_eq!(next_index(Some(0), 3), 1);
        assert_eq!(next_index(Some(2), 3), 0);
        assert_eq!(next_index(Some(10), 3), 0);

        assert_eq!(prev_index(None, 0), 0);
        assert_eq!(prev_index(Some(0), 0), 0);
        assert_eq!(prev_index(None, 1), 0);
        assert_eq!(prev_index(Some(0), 1), 0);
        assert_eq!(prev_index(None, 3), 2);
        assert_eq!(prev_index(Some(0), 3), 2);
        assert_eq!(prev_index(Some(2), 3), 1);
        assert_eq!(prev_index(Some(10), 3), 2);
    }

    #[test]
    fn test_matches_filter() {
        assert!(matches_filter("", "alice@gmail.com"));
        assert!(matches_filter("   ", "alice@gmail.com"));
        assert!(matches_filter("ALICE", "alice@gmail.com"));
        assert!(matches_filter("alice", "ALICE@GMAIL.COM"));
        assert!(!matches_filter("bob", "alice@gmail.com"));
        assert!(!matches_filter("query", ""));
    }

    #[test]
    fn test_filter_account_indices() {
        let accounts = vec![
            make_acc("alice@gmail.com", true),
            make_acc("bob@company.org", false),
            make_acc("charlie@gmail.com", false),
        ];
        assert_eq!(filter_account_indices(&accounts, "gmail"), vec![0, 2]);
        assert_eq!(filter_account_indices(&accounts, "BOB"), vec![1]);
        assert_eq!(filter_account_indices(&accounts, ""), vec![0, 1, 2]);
        assert_eq!(
            filter_account_indices(&accounts, "xyz"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn test_filter_session_indices() {
        let sessions = vec![
            make_sess("cid-alpha-001", "alpha-001", "Refactor auth token parser"),
            make_sess("cid-beta-002", "beta-002", "Implement UI widgets tests"),
        ];
        assert_eq!(filter_session_indices(&sessions, "alpha"), vec![0]);
        assert_eq!(filter_session_indices(&sessions, "WIDGETS"), vec![1]);
        assert_eq!(filter_session_indices(&sessions, "002"), vec![1]);
        assert_eq!(filter_session_indices(&sessions, ""), vec![0, 1]);
        assert_eq!(
            filter_session_indices(&sessions, "notfound"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn test_quota_color_and_labels() {
        assert_eq!(quota_color(100), Color::Green);
        assert_eq!(quota_color(50), Color::Green);
        assert_eq!(quota_color(49), Color::Yellow);
        assert_eq!(quota_color(20), Color::Yellow);
        assert_eq!(quota_color(19), Color::Red);

        assert_eq!(account_status_label(true), "* ACTIVE");
        assert_eq!(account_status_label(false), "  INACTIVE");
    }

    #[test]
    fn test_format_helpers() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(1024), "1.0KB");
        assert_eq!(format_bytes(1048576), "1.0MB");

        assert_eq!(format_quota_badge(None), "[quota unavailable]");
        let quota = AccountQuotaInfo {
            plan_type: None,
            gemini_percent: Some(85),
            claude_percent: Some(40),
            top_model_name: None,
            top_model_percent: None,
            fetched_at: 0,
            is_fresh: false,
        };
        assert_eq!(
            format_quota_badge(Some(&quota)),
            "[Gemini: 85% | Claude: 40%]"
        );

        let acc_hdr = format_accounts_header(3, "user@test.com", false);
        assert_eq!(
            acc_hdr,
            " 🤖 AGYM — Antigravity Accounts (3) | Active: user@test.com"
        );
        let acc_hdr_ref = format_accounts_header(3, "user@test.com", true);
        assert!(acc_hdr_ref.ends_with("| Refreshing... ⏳"));

        let sess_hdr = format_sessions_header(10, "");
        assert_eq!(sess_hdr, " 💬 AGYM — Session Explorer (10) | Filter: None");

        let sess = make_sess("cid-123", "123", "Test summary");
        let detail = format_session_detail(Some(&sess));
        assert!(detail.contains("ID: cid-123"));
        assert_eq!(format_session_detail(None), "No session selected.");

        let msg = format_refresh_cooldown(12);
        assert!(msg.contains("12s"));
    }

    #[test]
    fn test_styles_instantiation() {
        assert_eq!(style_header().fg, Some(Color::Cyan));
        assert_eq!(style_selected().fg, Some(Color::Black));
        assert_eq!(style_selected().bg, Some(Color::Cyan));
        assert_eq!(style_dimmed().fg, Some(Color::DarkGray));
        assert_eq!(style_warning().fg, Some(Color::Yellow));
    }
}
