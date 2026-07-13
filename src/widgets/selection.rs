//! Shared rendering helpers for multi-select mode (US2).
//!
//! When selection mode is active, track rows are prefixed with a checkbox
//! marker (`[x]` / `[ ]`) and a live "N selected" count is shown.

use crate::app::Selection;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Prefix span for a track row when selection mode is active.
///
/// Returns `None` when selection mode is off, so callers can conditionally
/// prepend the marker without altering the normal (non-selection) layout.
pub fn marker_span(selection: &Selection, track_id: i64) -> Option<Span<'static>> {
    if !selection.selection_mode {
        return None;
    }
    if selection.selected_ids.contains(&track_id) {
        Some(Span::styled(
            "[x] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
    } else {
        Some(Span::styled("[ ] ", Style::default().fg(Color::DarkGray)))
    }
}

/// A "N selected" label span, or `None` when selection mode is off.
pub fn count_span(selection: &Selection) -> Option<Span<'static>> {
    if !selection.selection_mode {
        return None;
    }
    Some(Span::styled(
        format!("  {} selected", selection.count()),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ))
}
