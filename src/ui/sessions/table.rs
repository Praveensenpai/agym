//! Session list table rendering and detail preview pane.

use super::SessionsApp;
use crate::session::SessionInfo;
use crate::ui::widgets::{
    format_bytes, format_session_detail, format_sessions_header, style_header, style_selected,
    style_warning, TITLE_SESSION_DETAIL,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
    Frame,
};

/// Renders the header banner and the scrollable session table into `chunks`.
pub fn render_table(
    app: &mut SessionsApp,
    frame: &mut Frame,
    chunks: &[ratatui::layout::Rect],
    filtered_indices: &[usize],
) {
    let header = Paragraph::new(format_sessions_header(filtered_indices.len(), &app.filter))
        .style(style_header())
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    let rows: Vec<Row> = filtered_indices
        .iter()
        .filter_map(|&idx| app.sessions.get(idx))
        .map(row_for)
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(18),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(30),
        ],
    )
    .header(
        Row::new(vec![
            "CID",
            "Account",
            "Date/Time",
            "Size",
            "Lines",
            "Prompt Summary",
        ])
        .style(style_header()),
    )
    .block(Block::default().borders(Borders::ALL))
    .row_highlight_style(style_selected());

    frame.render_stateful_widget(table, chunks[1], &mut app.state);
}

/// Builds a table row for a single session.
fn row_for(s: &SessionInfo) -> Row<'static> {
    Row::new(vec![
        s.short_cid.clone(),
        s.account.clone(),
        s.datetime.clone(),
        format_bytes(s.size_bytes),
        format!("{} lines", s.line_count),
        s.summary.clone(),
    ])
}

/// Renders the optional detail preview pane, returning the footer chunk index.
pub fn render_detail_pane(
    app: &SessionsApp,
    frame: &mut Frame,
    chunks: &[ratatui::layout::Rect],
    filtered_indices: &[usize],
) -> usize {
    if !app.show_detail {
        return 2;
    }
    let selected_session = app
        .state
        .selected()
        .and_then(|i| filtered_indices.get(i))
        .and_then(|&real_idx| app.sessions.get(real_idx));
    let detail_block = Paragraph::new(format_session_detail(selected_session))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(TITLE_SESSION_DETAIL)
                .borders(Borders::ALL)
                .border_style(style_warning()),
        );
    frame.render_widget(detail_block, chunks[2]);
    3
}

/// Computes the vertical layout split for the given detail-pane visibility.
pub fn layout_chunks(frame: &Frame, show_detail: bool) -> std::rc::Rc<[ratatui::layout::Rect]> {
    let constraints = if show_detail {
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
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area())
}
