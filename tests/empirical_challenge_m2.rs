use ratatui::{
    backend::TestBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Terminal,
};
use std::process::Command;

fn get_bin_path() -> String {
    let mut path = std::env::current_exe().expect("failed to get current test exe path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("agym");
    path.to_string_lossy().to_string()
}

#[test]
fn test_cli_help_flag_displays_usage_and_subcommands() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to execute agym --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Unified Antigravity CLI & Account Manager"));
    assert!(stdout.contains("save"));
    assert!(stdout.contains("completions"));
    assert!(stdout.contains("--version"));
}

#[test]
fn test_cli_save_subcommand_without_token_exits_gracefully() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("save")
        .output()
        .expect("failed to execute agym save");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);
    assert!(
        !combined.contains("panicked at"),
        "agym save panicked: {}",
        combined
    );
}

#[test]
fn test_cli_invalid_subcommand_error_handling() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("nonexistent_subcommand_xyz")
        .output()
        .expect("failed to execute agym");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}\n{}", stdout, stderr);
    assert!(!combined.contains("panicked at"));
    assert!(combined.contains("not found"));
}

#[test]
fn test_ratatui_accounts_table_rendering_in_test_backend() {
    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).expect("failed to construct TestBackend terminal");

    terminal
        .draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            let header = Paragraph::new(
                " 🤖 AGYM — Antigravity Accounts (2) | Active: active@test.com",
            )
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(header, chunks[0]);

            let rows = vec![
                Row::new(vec![
                    Cell::from("* ACTIVE"),
                    Cell::from("active@test.com"),
                    Cell::from("[Gemini: 90% | Claude: 80%]")
                        .style(Style::default().fg(Color::Green)),
                ]),
                Row::new(vec![
                    Cell::from("  INACTIVE"),
                    Cell::from("other@test.com"),
                    Cell::from("[quota unavailable]")
                        .style(Style::default().fg(Color::DarkGray)),
                ]),
            ];

            let table = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(30),
                    Constraint::Min(30),
                ],
            )
            .header(
                Row::new(vec!["Status", "Account Email", "Quota Metrics"]).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .block(Block::default().borders(Borders::ALL))
            .row_highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );

            let mut state = TableState::default();
            state.select(Some(0));
            frame.render_stateful_widget(table, chunks[1], &mut state);

            let footer = Paragraph::new(
                " [Enter] Switch | [s] Sessions | [n] New Log | [d] Delete | [r] Refresh | [/] Filter | [q] Quit",
            )
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(footer, chunks[2]);
        })
        .expect("draw failed");

    let buffer = terminal.backend().buffer();
    let rendered_text = buffer_to_string(buffer);

    assert!(rendered_text.contains("AGYM — Antigravity Accounts"));
    assert!(rendered_text.contains("active@test.com"));
    assert!(rendered_text.contains("* ACTIVE"));
    assert!(rendered_text.contains("other@test.com"));
    assert!(rendered_text.contains("INACTIVE"));
    assert!(rendered_text.contains("Quota Metrics"));
    assert!(rendered_text.contains("[q] Quit"));
}

#[test]
fn test_ratatui_sessions_explorer_rendering_in_test_backend() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("failed to construct TestBackend terminal");

    terminal
        .draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Percentage(50),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            let header = Paragraph::new(
                " 💬 AGYM — Session Explorer (1) | Filter: None",
            )
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(header, chunks[0]);

            let rows = vec![Row::new(vec![
                "abc-123".to_string(),
                "2026-08-30 12:00".to_string(),
                "2.5KB".to_string(),
                "42 lines".to_string(),
                "Refactor authentication flow".to_string(),
            ])];

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
                Row::new(vec!["CID", "Date/Time", "Size", "Lines", "Prompt Summary"]).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .block(Block::default().borders(Borders::ALL));

            let mut state = TableState::default();
            state.select(Some(0));
            frame.render_stateful_widget(table, chunks[1], &mut state);

            let detail_text = "ID: cid-full-uuid-1234\nDate: 2026-08-30 12:00\nSize: 2.5KB | Lines: 42\n\nFull prompt description";
            let detail_block = Paragraph::new(detail_text)
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(" 🔍 Session Detail Preview ")
                        .borders(Borders::ALL)
                        .border_style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                );
            frame.render_widget(detail_block, chunks[2]);

            let footer = Paragraph::new(
                " [Enter] Resume | [Space/v] Toggle Preview | [a] Accounts | [/] Filter | [q] Quit",
            )
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(footer, chunks[3]);
        })
        .expect("draw failed");

    let buffer = terminal.backend().buffer();
    let rendered_text = buffer_to_string(buffer);

    assert!(rendered_text.contains("Session Explorer"));
    assert!(rendered_text.contains("abc-123"));
    assert!(rendered_text.contains("Refactor authentication flow"));
    assert!(rendered_text.contains("Session Detail Preview"));
    assert!(rendered_text.contains("Full prompt description"));
    assert!(rendered_text.contains("[Enter] Resume"));
}

#[test]
fn test_ratatui_empty_accounts_loading_state_rendering() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).expect("failed to construct terminal");

    terminal
        .draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            let header = Paragraph::new(
                " 🤖 AGYM — Antigravity Accounts (0) | Active: None | Refreshing... ⏳",
            )
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(header, chunks[0]);

            let rows = vec![Row::new(vec![
                Cell::from(""),
                Cell::from("⏳ Loading accounts…"),
                Cell::from("Please wait"),
            ])];

            let table = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(30),
                    Constraint::Min(30),
                ],
            )
            .header(Row::new(vec!["Status", "Account Email", "Quota Metrics"]))
            .block(Block::default().borders(Borders::ALL));

            let mut state = TableState::default();
            frame.render_stateful_widget(table, chunks[1], &mut state);

            let footer = Paragraph::new(
                " ⏳ Fetching live account quotas in background... Navigate freely with [↑/↓]",
            )
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(footer, chunks[2]);
        })
        .expect("draw failed");

    let buffer = terminal.backend().buffer();
    let text = buffer_to_string(buffer);
    assert!(text.contains("Loading accounts"));
    assert!(text.contains("Refreshing... ⏳"));
    assert!(text.contains("Fetching live account quotas"));
}

fn buffer_to_string(buffer: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
