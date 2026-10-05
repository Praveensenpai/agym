//! Quota meter rendering helpers: color coding, circular glyphs, and styled meter lines.

use super::style_dimmed;
use crate::quota::AccountQuotaInfo;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Returns the color indicator corresponding to a quota percentage (>=50% Green, 20-49% Yellow, <20% Red).
#[must_use]
pub fn quota_color(percent: u32) -> Color {
    match percent {
        p if p >= 50 => Color::Green,
        p if p >= 20 => Color::Yellow,
        _ => Color::Red,
    }
}

/// Returns the circular progress meter glyph matching the quota percentage.
#[must_use]
pub fn quota_circle_glyph(percent: u32) -> &'static str {
    match percent {
        88..=u32::MAX => "●",
        63..=87 => "◕",
        38..=62 => "◑",
        13..=37 => "◔",
        _ => "○",
    }
}

/// Returns the color indicator corresponding to a quota percentage.
#[must_use]
pub fn quota_progress_color(percent: u32) -> Color {
    if (50..75).contains(&percent) {
        Color::Cyan
    } else {
        quota_color(percent)
    }
}

/// Configuration options for rendering a model's quota meters.
pub struct ModelQuotaRenderConfig<'a> {
    pub name: &'a str,
    pub name_color: Color,
    pub is_selected: bool,
    pub week_pct: Option<u32>,
    pub win_pct: Option<u32>,
}

/// Standardized fixed column width for the primary (Gemini) model block to ensure vertical divider alignment.
pub const TARGET_MODEL_BLOCK_WIDTH: usize = 31;

/// Appends styled Spans for an individual model's quota metrics into the line buffer.
pub fn append_model_quota_spans<'a>(spans: &mut Vec<Span<'a>>, cfg: ModelQuotaRenderConfig<'a>) {
    let name_style = if cfg.is_selected {
        Style::default()
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(cfg.name_color)
            .add_modifier(Modifier::BOLD)
    };
    let dim_style = if cfg.is_selected {
        Style::default().fg(Color::DarkGray)
    } else {
        style_dimmed()
    };
    let val_style = |pct: u32| {
        if cfg.is_selected {
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(quota_progress_color(pct))
        }
    };

    spans.push(Span::styled(cfg.name, name_style));
    spans.push(Span::styled(" [", dim_style));

    match (cfg.week_pct, cfg.win_pct) {
        (Some(w), Some(h)) => {
            spans.push(Span::styled("W: ", dim_style));
            spans.push(Span::styled(quota_circle_glyph(w), val_style(w)));
            spans.push(Span::styled(format!(" {w:>3}%"), val_style(w)));
            spans.push(Span::styled(" · 5h: ", dim_style));
            spans.push(Span::styled(quota_circle_glyph(h), val_style(h)));
            spans.push(Span::styled(format!(" {h:>3}%"), val_style(h)));
        }
        (Some(w), None) => {
            spans.push(Span::styled("W: ", dim_style));
            spans.push(Span::styled(quota_circle_glyph(w), val_style(w)));
            spans.push(Span::styled(format!(" {w:>3}%"), val_style(w)));
        }
        (None, Some(h)) => {
            spans.push(Span::styled("5h: ", dim_style));
            spans.push(Span::styled(quota_circle_glyph(h), val_style(h)));
            spans.push(Span::styled(format!(" {h:>3}%"), val_style(h)));
        }
        (None, None) => {
            spans.push(Span::styled("—", dim_style));
        }
    }
    spans.push(Span::styled("]", dim_style));
}

/// Formats a complete multi-window, circular styled Line for an account's quota table cell.
#[must_use]
pub fn format_quota_cell_line<'a>(quota: Option<&AccountQuotaInfo>, is_selected: bool) -> Line<'a> {
    let q = match quota {
        Some(q) => q,
        None => return Line::from(vec![Span::styled(format_quota_badge(None), style_dimmed())]),
    };

    let dim_style = if is_selected {
        Style::default().fg(Color::DarkGray)
    } else {
        style_dimmed()
    };

    let mut spans = Vec::with_capacity(16);
    append_model_quota_spans(
        &mut spans,
        ModelQuotaRenderConfig {
            name: "Gemini",
            name_color: Color::Cyan,
            is_selected,
            week_pct: q.gemini_week_percent,
            win_pct: q.gemini_window_percent.or(q.gemini_percent),
        },
    );

    let gemini_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    if gemini_width < TARGET_MODEL_BLOCK_WIDTH {
        spans.push(Span::raw(
            " ".repeat(TARGET_MODEL_BLOCK_WIDTH - gemini_width),
        ));
    }

    spans.push(Span::styled("  │  ", dim_style));

    append_model_quota_spans(
        &mut spans,
        ModelQuotaRenderConfig {
            name: "Claude",
            name_color: Color::Magenta,
            is_selected,
            week_pct: q.claude_week_percent,
            win_pct: q.claude_window_percent.or(q.claude_percent),
        },
    );

    Line::from(spans)
}

/// Formats the quota badge text with a safe fallback when quota info is missing.
#[must_use]
pub fn format_quota_badge(quota: Option<&AccountQuotaInfo>) -> String {
    quota
        .map(|q| q.display_badge())
        .unwrap_or_else(|| "[quota unavailable]".to_string())
}
