//! Account-switch confirmation overlay rendering.
//!
//! Draws the centered modal that shows the owning account, its remaining quota,
//! and the colored `Switch` / `Copy & continue` / `Cancel` button bar.

use super::ConfirmState;
use crate::account::{email_prefix, list_account_infos_cached};
use crate::session::SessionInfo;
use crate::ui::widgets::{
    format_resume_buttons, format_resume_prompt, style_warning, TITLE_SESSION_CONFIRM,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Renders the centered confirmation overlay for the given session and button focus.
pub fn render_confirm_overlay(
    frame: &mut Frame,
    session: &SessionInfo,
    active_account: &str,
    state: &ConfirmState,
) {
    let quota = list_account_infos_cached()
        .into_iter()
        .find(|a| email_prefix(&a.email) == session.account)
        .and_then(|a| a.quota);

    let body = format_resume_prompt(session, active_account, quota.as_ref());
    let buttons = format_resume_buttons(state.focused);
    let area = centered_rect(70, 50, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let overlay = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .style(style_warning())
        .block(
            Block::default()
                .title(TITLE_SESSION_CONFIRM)
                .borders(Borders::ALL)
                .border_style(style_warning()),
        );
    frame.render_widget(overlay, chunks[0]);

    let button_bar = Paragraph::new(buttons)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(button_bar, chunks[1]);
}

/// Computes a centered rectangle of the given percentage width/height.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
