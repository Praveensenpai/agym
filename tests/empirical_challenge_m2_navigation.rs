//! Adversarial Empirical Challenge Suite for Milestone 2: UI Subsystem & Pure Navigation Math.
//!
//! Empirically challenges:
//! 1. `next_index` and `prev_index` against: empty lists, single-element lists, large lists,
//!    out-of-bounds start indices, `usize::MAX` overflow scenarios, and automated fuzzing invariants.
//! 2. `matches_filter`, `filter_account_indices`, and `filter_session_indices` against:
//!    empty queries, whitespace queries, regex meta-characters, unicode emojis, non-ASCII/CJK/Cyrillic/accents,
//!    SQL injection payloads, null/control characters, no matches, all matches, and high-volume lists.
//! 3. Presentation helpers & constants: `format_bytes`, `quota_color`, `format_quota_badge`,
//!    `format_accounts_header`, `format_sessions_header`, `format_session_detail`, `format_refresh_cooldown`,
//!    and UI constants.

use std::path::PathBuf;
use std::time::Duration;

pub mod quota {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AccountQuotaInfo {
        pub plan_type: Option<String>,
        pub gemini_percent: Option<u32>,
        pub claude_percent: Option<u32>,
        pub top_model_name: Option<String>,
        pub top_model_percent: Option<u32>,
        pub fetched_at: u64,
        pub is_fresh: bool,
    }

    impl AccountQuotaInfo {
        pub fn display_badge(&self) -> String {
            match (self.gemini_percent, self.claude_percent) {
                (Some(g), Some(c)) => format!("[Gemini: {g}% | Claude: {c}%]"),
                (Some(g), None) => format!("[Gemini: {g}%]"),
                (None, Some(c)) => format!("[Claude: {c}%]"),
                _ => "[quota active]".to_string(),
            }
        }
    }
}

pub mod account {
    use super::quota::AccountQuotaInfo;
    use std::path::PathBuf;

    #[derive(Debug, Clone, PartialEq)]
    pub struct AccountInfo {
        pub email: String,
        pub is_active: bool,
        pub quota: Option<AccountQuotaInfo>,
        pub file_path: PathBuf,
    }
}

pub mod session {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SessionInfo {
        pub cid: String,
        pub short_cid: String,
        pub datetime: String,
        pub timestamp: u64,
        pub size_bytes: u64,
        pub line_count: usize,
        pub summary: String,
        pub full_prompt: String,
    }

    pub fn format_bytes(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{bytes}B")
        } else if bytes < 1024 * 1024 {
            format!("{:.1}KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }
}

#[path = "../src/ui/widgets.rs"]
mod widgets;

use account::AccountInfo;
use quota::AccountQuotaInfo;
use session::SessionInfo;
use widgets::*;

fn make_test_account(email: &str, is_active: bool) -> AccountInfo {
    AccountInfo {
        email: email.to_string(),
        is_active,
        quota: None,
        file_path: PathBuf::from(format!("/fake/path/{email}.json")),
    }
}

fn make_test_session(cid: &str, short_cid: &str, summary: &str) -> SessionInfo {
    SessionInfo {
        cid: cid.to_string(),
        short_cid: short_cid.to_string(),
        datetime: "2026-08-30 12:00".to_string(),
        timestamp: 1772366400,
        size_bytes: 4096,
        line_count: 120,
        summary: summary.to_string(),
        full_prompt: format!("Adversarial prompt for session {cid}"),
    }
}

// =========================================================================
// 1. ADVERSARIAL NAVIGATION TESTS (`next_index`, `prev_index`)
// =========================================================================

#[test]
fn test_adv_navigation_empty_lists() {
    let test_indices = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(100),
        Some(1_000_000),
        Some(usize::MAX - 1),
        Some(usize::MAX),
    ];

    for idx in test_indices {
        assert_eq!(
            next_index(idx, 0),
            0,
            "next_index failed on total=0 with index {:?}",
            idx
        );
        assert_eq!(
            prev_index(idx, 0),
            0,
            "prev_index failed on total=0 with index {:?}",
            idx
        );
    }
}

#[test]
fn test_adv_navigation_single_element_list() {
    assert_eq!(next_index(None, 1), 0);
    assert_eq!(next_index(Some(0), 1), 0);
    assert_eq!(next_index(Some(1), 1), 0);
    assert_eq!(next_index(Some(999), 1), 0);
    assert_eq!(next_index(Some(usize::MAX), 1), 0);

    assert_eq!(prev_index(None, 1), 0);
    assert_eq!(prev_index(Some(0), 1), 0);
    assert_eq!(prev_index(Some(1), 1), 0);
    assert_eq!(prev_index(Some(999), 1), 0);
    assert_eq!(prev_index(Some(usize::MAX), 1), 0);
}

#[test]
fn test_adv_navigation_out_of_bounds_recovery() {
    let total = 3;

    assert_eq!(next_index(Some(3), total), 0);
    assert_eq!(next_index(Some(4), total), 0);
    assert_eq!(next_index(Some(100), total), 0);
    assert_eq!(next_index(Some(usize::MAX), total), 0);

    assert_eq!(prev_index(Some(3), total), 2);
    assert_eq!(prev_index(Some(4), total), 2);
    assert_eq!(prev_index(Some(100), total), 2);
    assert_eq!(prev_index(Some(usize::MAX), total), 2);
}

#[test]
fn test_adv_navigation_full_bidirectional_cycles() {
    let total = 7;

    let mut current = Some(0);
    for expected in [1, 2, 3, 4, 5, 6, 0, 1] {
        current = Some(next_index(current, total));
        assert_eq!(current, Some(expected));
    }

    current = Some(0);
    for expected in [6, 5, 4, 3, 2, 1, 0, 6, 5] {
        current = Some(prev_index(current, total));
        assert_eq!(current, Some(expected));
    }

    assert_eq!(next_index(None, total), 0);
    assert_eq!(prev_index(None, total), 6);
}

#[test]
fn test_adv_navigation_large_lists_and_overflow_safety() {
    let large_total = 1_000_000;
    assert_eq!(next_index(Some(999_998), large_total), 999_999);
    assert_eq!(next_index(Some(999_999), large_total), 0);
    assert_eq!(prev_index(Some(0), large_total), 999_999);
    assert_eq!(prev_index(Some(999_999), large_total), 999_998);

    let max_total = usize::MAX;
    let max_idx = max_total - 1;

    assert_eq!(next_index(Some(max_idx - 1), max_total), max_idx);
    assert_eq!(next_index(Some(max_idx), max_total), 0);
    assert_eq!(next_index(Some(usize::MAX), max_total), 0);

    assert_eq!(prev_index(Some(0), max_total), max_idx);
    assert_eq!(prev_index(Some(max_idx), max_total), max_idx - 1);
    assert_eq!(prev_index(Some(usize::MAX), max_total), max_idx);
    assert_eq!(prev_index(None, max_total), max_idx);
}

#[test]
fn test_adv_navigation_fuzzing_invariants() {
    let test_totals = [
        0,
        1,
        2,
        3,
        5,
        10,
        100,
        1000,
        65535,
        usize::MAX - 1,
        usize::MAX,
    ];
    let test_indices = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(99),
        Some(100),
        Some(101),
        Some(65534),
        Some(65535),
        Some(65536),
        Some(usize::MAX / 2),
        Some(usize::MAX - 2),
        Some(usize::MAX - 1),
        Some(usize::MAX),
    ];

    for &tot in &test_totals {
        for &idx in &test_indices {
            let next = next_index(idx, tot);
            let prev = prev_index(idx, tot);

            if tot == 0 {
                assert_eq!(next, 0);
                assert_eq!(prev, 0);
            } else {
                assert!(
                    next < tot,
                    "Invariant violated: next_index({:?}, {}) = {} >= total",
                    idx,
                    tot,
                    next
                );
                assert!(
                    prev < tot,
                    "Invariant violated: prev_index({:?}, {}) = {} >= total",
                    idx,
                    tot,
                    prev
                );
            }
        }
    }
}

// =========================================================================
// 2. ADVERSARIAL FILTERING TESTS (`matches_filter`, `filter_*_indices`)
// =========================================================================

#[test]
fn test_adv_matches_filter_empty_and_whitespace() {
    assert!(matches_filter("", ""));
    assert!(matches_filter("", "alice@gmail.com"));
    assert!(matches_filter("   ", "alice@gmail.com"));
    assert!(matches_filter("\t\r\n  \t", "alice@gmail.com"));
    assert!(!matches_filter("something", ""));
}

#[test]
fn test_adv_matches_filter_regex_metacharacters() {
    assert!(matches_filter(".*", "user.*@company.org"));
    assert!(matches_filter("(1)+2", "file(1)+2[3]?^$4.json"));
    assert!(matches_filter("[3]?", "file(1)+2[3]?^$4.json"));
    assert!(matches_filter("^$4", "file(1)+2[3]?^$4.json"));
    assert!(matches_filter("\\to\\", "path\\to\\file"));
    assert!(matches_filter("|claude|", "model|claude|gemini"));
    assert!(matches_filter("{placeholder}", "{placeholder}"));

    let adversarial_regex_queries = [
        "[a-z",
        "(unclosed group",
        "*quantifier without target",
        "+plus start",
        "?question start",
        "\\",
        "((((((((",
        "|",
        "^",
        "$",
    ];

    for q in adversarial_regex_queries {
        let _ = matches_filter(q, "target_string_with_symbols_*+?^$|()[]\\");
    }
}

#[test]
fn test_adv_matches_filter_unicode_and_emojis() {
    assert!(matches_filter("🚀", "Project 🚀 Launch session"));
    assert!(matches_filter("🤖", "🤖 AGYM account"));
    assert!(matches_filter("🎉", "Celebration 🎉 party"));
    assert!(!matches_filter("🔥", "Project 🚀 Launch session"));

    assert!(matches_filter("MÜLLER", "müller@firma.de"));
    assert!(matches_filter("müller", "MÜLLER@FIRMA.DE"));
    assert!(matches_filter("café", "Visit CAFÉ in Paris"));
    assert!(matches_filter("CAFÉ", "visit café in paris"));
    assert!(matches_filter("señor", "SEÑOR DEV"));

    assert!(matches_filter("привет", "Сообщение: ПРИВЕТ МИР"));
    assert!(matches_filter("ПРИВЕТ", "сообщение: привет мир"));

    assert!(matches_filter("日本語", "これは日本語のテストです"));
    assert!(matches_filter("北京", "欢迎来到北京"));
    assert!(matches_filter("한국어", "안녕하세요 한국어 지원"));

    assert!(matches_filter("مرحبا", "رسالة مرحبا بالعالم"));
    assert!(matches_filter("שלום", "טקסט שלום עולם"));
}

#[test]
fn test_adv_matches_filter_injections_and_control_chars() {
    let target = "admin@database.internal.corp";

    assert!(!matches_filter("'; DROP TABLE accounts; --", target));
    assert!(matches_filter("admin", "admin'; DROP TABLE accounts; --"));

    assert!(matches_filter("\u{200B}", "zero\u{200B}width"));
    assert!(!matches_filter("\u{0000}", target));
}

#[test]
fn test_adv_filter_account_indices_comprehensive() {
    let accounts = vec![
        make_test_account("alice.developer@gemini.corp", true),
        make_test_account("bob.tester@antigravity.io", false),
        make_test_account("charlie.manager@gemini.corp", false),
        make_test_account("dave.secops@antigravity.io", false),
        make_test_account("eve.researcher@gemini.corp", false),
        make_test_account("francis.cjk.日本語@tokyo.jp", false),
        make_test_account("george.emoji.🚀@orbit.space", false),
    ];

    assert_eq!(
        filter_account_indices(&accounts, ""),
        (0..7).collect::<Vec<_>>()
    );
    assert_eq!(
        filter_account_indices(&accounts, "   "),
        (0..7).collect::<Vec<_>>()
    );

    assert_eq!(
        filter_account_indices(&accounts, "gemini.corp"),
        vec![0, 2, 4]
    );
    assert_eq!(
        filter_account_indices(&accounts, "ANTIGRAVITY.IO"),
        vec![1, 3]
    );

    assert_eq!(filter_account_indices(&accounts, "alice"), vec![0]);
    assert_eq!(filter_account_indices(&accounts, "BOB"), vec![1]);

    assert_eq!(filter_account_indices(&accounts, "日本語"), vec![5]);
    assert_eq!(filter_account_indices(&accounts, "🚀"), vec![6]);

    assert_eq!(
        filter_account_indices(&accounts, "nonexistent@nowhere.com"),
        Vec::<usize>::new()
    );
    assert_eq!(
        filter_account_indices(&accounts, "xyz12345"),
        Vec::<usize>::new()
    );

    let empty_accounts: Vec<AccountInfo> = Vec::new();
    assert_eq!(
        filter_account_indices(&empty_accounts, ""),
        Vec::<usize>::new()
    );
    assert_eq!(
        filter_account_indices(&empty_accounts, "alice"),
        Vec::<usize>::new()
    );
}

#[test]
fn test_adv_filter_session_indices_multi_field_matching() {
    let sessions = vec![
        make_test_session(
            "cid-01-auth-refactor",
            "01-auth",
            "Refactor OAuth keyring token extraction",
        ),
        make_test_session(
            "cid-02-ui-modularization",
            "02-ui",
            "Split monolithic ui.rs into submodules",
        ),
        make_test_session(
            "cid-03-sqlite-migration",
            "03-sqlite",
            "Synchronize active profile to sqlite DB",
        ),
        make_test_session(
            "cid-04-cjk-support",
            "04-cjk",
            "Support multilingual transcripts: 日本語と한국어",
        ),
        make_test_session(
            "cid-05-emoji-rocket",
            "05-🚀",
            "Add rocket 🚀 emoji to release pipeline",
        ),
    ];

    assert_eq!(filter_session_indices(&sessions, "cid-01"), vec![0]);
    assert_eq!(filter_session_indices(&sessions, "CID-03"), vec![2]);

    assert_eq!(filter_session_indices(&sessions, "02-ui"), vec![1]);
    assert_eq!(filter_session_indices(&sessions, "05-🚀"), vec![4]);

    assert_eq!(filter_session_indices(&sessions, "monolithic"), vec![1]);
    assert_eq!(filter_session_indices(&sessions, "KEYRING"), vec![0]);
    assert_eq!(filter_session_indices(&sessions, "日本語"), vec![3]);
    assert_eq!(filter_session_indices(&sessions, "rocket"), vec![4]);

    assert_eq!(filter_session_indices(&sessions, "refactor"), vec![0]);

    assert_eq!(
        filter_session_indices(&sessions, ""),
        (0..5).collect::<Vec<_>>()
    );

    assert_eq!(
        filter_session_indices(&sessions, "nonexistent"),
        Vec::<usize>::new()
    );

    let empty_sessions: Vec<SessionInfo> = Vec::new();
    assert_eq!(
        filter_session_indices(&empty_sessions, "auth"),
        Vec::<usize>::new()
    );
}

#[test]
fn test_adv_high_volume_filtering_stress() {
    let accounts: Vec<AccountInfo> = (0..10_000)
        .map(|i| {
            let domain = if i % 3 == 0 {
                "gemini.corp"
            } else if i % 3 == 1 {
                "antigravity.io"
            } else {
                "external.net"
            };
            make_test_account(&format!("user_{i:05}@{domain}"), i == 0)
        })
        .collect();

    let gemini_matches = filter_account_indices(&accounts, "gemini.corp");
    assert_eq!(gemini_matches.len(), 3334);
    assert_eq!(gemini_matches[0], 0);
    assert_eq!(gemini_matches[1], 3);

    let specific_match = filter_account_indices(&accounts, "user_09999");
    assert_eq!(specific_match, vec![9999]);

    assert_eq!(next_index(Some(3333), gemini_matches.len()), 0);
    assert_eq!(prev_index(Some(0), gemini_matches.len()), 3333);
}

// =========================================================================
// 3. ADVERSARIAL PRESENTATION & FORMATTING TESTS
// =========================================================================

#[test]
fn test_adv_presentation_formatting_bounds() {
    assert_eq!(format_bytes(0), "0B");
    assert_eq!(format_bytes(1), "1B");
    assert_eq!(format_bytes(1023), "1023B");
    assert_eq!(format_bytes(1024), "1.0KB");
    assert_eq!(format_bytes(1536), "1.5KB");
    assert_eq!(format_bytes(1024 * 1024 - 1), "1024.0KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0MB");
    assert_eq!(format_bytes(100 * 1024 * 1024), "100.0MB");
    assert_eq!(format_bytes(u64::MAX), "17592186044416.0MB");

    assert_eq!(quota_color(100), ratatui::style::Color::Green);
    assert_eq!(quota_color(51), ratatui::style::Color::Green);
    assert_eq!(quota_color(50), ratatui::style::Color::Green);
    assert_eq!(quota_color(49), ratatui::style::Color::Yellow);
    assert_eq!(quota_color(21), ratatui::style::Color::Yellow);
    assert_eq!(quota_color(20), ratatui::style::Color::Yellow);
    assert_eq!(quota_color(19), ratatui::style::Color::Red);
    assert_eq!(quota_color(1), ratatui::style::Color::Red);
    assert_eq!(quota_color(0), ratatui::style::Color::Red);

    assert_eq!(account_status_label(true), STATUS_ACTIVE_BADGE);
    assert_eq!(account_status_label(false), STATUS_INACTIVE_BADGE);

    assert_eq!(format_quota_badge(None), "[quota unavailable]");
    let quota_full = AccountQuotaInfo {
        plan_type: Some("Pro".to_string()),
        gemini_percent: Some(90),
        claude_percent: Some(75),
        top_model_name: Some("gemini-1.5-pro".to_string()),
        top_model_percent: Some(90),
        fetched_at: 1700000000,
        is_fresh: true,
    };
    assert_eq!(
        format_quota_badge(Some(&quota_full)),
        "[Gemini: 90% | Claude: 75%]"
    );

    assert_eq!(
        format_accounts_header(0, "none", false),
        " 🤖 AGYM — Antigravity Accounts (0) | Active: none"
    );
    assert_eq!(
        format_accounts_header(5, "alice@gemini.corp", true),
        " 🤖 AGYM — Antigravity Accounts (5) | Active: alice@gemini.corp | Refreshing... ⏳"
    );

    assert_eq!(
        format_sessions_header(0, ""),
        " 💬 AGYM — Session Explorer (0) | Filter: None"
    );
    assert_eq!(
        format_sessions_header(0, "   "),
        " 💬 AGYM — Session Explorer (0) | Filter: None"
    );
    assert_eq!(
        format_sessions_header(12, "OAuth"),
        " 💬 AGYM — Session Explorer (12) | Filter: OAuth"
    );

    assert_eq!(format_session_detail(None), "No session selected.");
    let sess = make_test_session("cid-999", "999", "Summary text");
    let detail = format_session_detail(Some(&sess));
    assert!(detail.contains("ID: cid-999"));
    assert!(detail.contains("Lines: 120"));
    assert!(detail.contains("Adversarial prompt for session cid-999"));

    assert_eq!(
        format_refresh_cooldown(15),
        " ⏳ Refresh on 15s cooldown. Please wait 15s before refreshing again."
    );
    assert_eq!(
        format_refresh_cooldown(0),
        " ⏳ Refresh on 15s cooldown. Please wait 0s before refreshing again."
    );

    assert_eq!(REFRESH_COOLDOWN_SECS, 15);
    assert_eq!(REFRESH_COOLDOWN, Duration::from_secs(15));
    assert_eq!(COOLDOWN_DISPLAY_DURATION, Duration::from_secs(3));
    assert_eq!(EVENT_POLL_TIMEOUT, Duration::from_millis(50));
    assert!(!HELP_ACCOUNTS_DEFAULT.is_empty());
    assert!(!HELP_ACCOUNTS_REFRESHING.is_empty());
    assert!(!HELP_ACCOUNTS_ALREADY_RUNNING.is_empty());
    assert!(!HELP_SESSIONS_DEFAULT.is_empty());
    assert_eq!(TITLE_SESSION_DETAIL, " 🔍 Session Detail Preview ");

    let _ = style_header();
    let _ = style_selected();
    let _ = style_dimmed();
    let _ = style_warning();
}
