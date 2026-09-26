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

    assert_eq!(quota_progress_color(90), Color::Green);
    assert_eq!(quota_progress_color(75), Color::Green);
    assert_eq!(quota_progress_color(60), Color::Cyan);
    assert_eq!(quota_progress_color(50), Color::Cyan);
    assert_eq!(quota_progress_color(30), Color::Yellow);
    assert_eq!(quota_progress_color(25), Color::Yellow);
    assert_eq!(quota_progress_color(10), Color::Red);

    assert_eq!(quota_circle_glyph(100), "●");
    assert_eq!(quota_circle_glyph(88), "●");
    assert_eq!(quota_circle_glyph(87), "◕");
    assert_eq!(quota_circle_glyph(63), "◕");
    assert_eq!(quota_circle_glyph(62), "◑");
    assert_eq!(quota_circle_glyph(38), "◑");
    assert_eq!(quota_circle_glyph(37), "◔");
    assert_eq!(quota_circle_glyph(13), "◔");
    assert_eq!(quota_circle_glyph(12), "○");
    assert_eq!(quota_circle_glyph(0), "○");

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
        gemini_week_percent: None,
        gemini_window_percent: None,
        gemini_percent: Some(85),
        claude_week_percent: None,
        claude_window_percent: None,
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

    let quota_multi = AccountQuotaInfo {
        plan_type: Some("Pro".to_string()),
        gemini_week_percent: Some(88),
        gemini_window_percent: Some(49),
        gemini_percent: Some(49),
        claude_week_percent: Some(76),
        claude_window_percent: Some(100),
        claude_percent: Some(100),
        top_model_name: Some("Gemini".to_string()),
        top_model_percent: Some(49),
        fetched_at: 0,
        is_fresh: false,
    };
    assert_eq!(
        format_quota_badge(Some(&quota_multi)),
        "[Gemini: 88%(W) 49%(5h) | Claude: 76%(W) 100%(5h)]"
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
fn test_format_quota_cell_line() {
    let unavail = format_quota_cell_line(None, false);
    assert_eq!(unavail.spans[0].content, "[quota unavailable]");

    let quota_multi = AccountQuotaInfo {
        plan_type: Some("Pro".to_string()),
        gemini_week_percent: Some(88),
        gemini_window_percent: Some(49),
        gemini_percent: Some(49),
        claude_week_percent: Some(76),
        claude_window_percent: Some(100),
        claude_percent: Some(100),
        top_model_name: Some("Gemini".to_string()),
        top_model_percent: Some(49),
        fetched_at: 0,
        is_fresh: false,
    };
    let line = format_quota_cell_line(Some(&quota_multi), false);
    let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(full_text.contains("Gemini"));
    assert!(full_text.contains("W: ●  88%"));
    assert!(full_text.contains("5h: ◑  49%"));
    assert!(full_text.contains("Claude"));
    assert!(full_text.contains("W: ◕  76%"));
    assert!(full_text.contains("5h: ● 100%"));

    // Verify vertical divider alignment with 5h-only quota
    let quota_5h = AccountQuotaInfo {
        plan_type: None,
        gemini_week_percent: None,
        gemini_window_percent: Some(100),
        gemini_percent: Some(100),
        claude_week_percent: None,
        claude_window_percent: Some(100),
        claude_percent: Some(100),
        top_model_name: None,
        top_model_percent: None,
        fetched_at: 0,
        is_fresh: false,
    };
    let line_5h = format_quota_cell_line(Some(&quota_5h), false);
    let full_text_5h: String = line_5h.spans.iter().map(|s| s.content.as_ref()).collect();
    let col_multi = full_text.chars().take_while(|&c| c != '│').count();
    let col_5h = full_text_5h.chars().take_while(|&c| c != '│').count();
    assert_eq!(
        col_multi, col_5h,
        "│ divider must align at the exact same visual column"
    );

    let selected_line = format_quota_cell_line(Some(&quota_multi), true);
    assert!(!selected_line.spans.is_empty());
}

#[test]
fn test_styles_instantiation() {
    assert_eq!(style_header().fg, Some(Color::Cyan));
    assert_eq!(style_selected().fg, Some(Color::Black));
    assert_eq!(style_selected().bg, Some(Color::Cyan));
    assert_eq!(style_dimmed().fg, Some(Color::DarkGray));
    assert_eq!(style_warning().fg, Some(Color::Yellow));
}
