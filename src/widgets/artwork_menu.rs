//! Small overlay menu for track artwork actions (feature 012, US3).
//!
//! Opened from the tag editor with `i`. Offers three actions:
//!   1. Set from file  — prompts for a local image path (via `input_dialog`).
//!   2. Fetch online   — launches a `FetchAlbumArtwork` job.
//!   3. Remove         — clears the album artwork for the track.
//!
//! Follows the [`crate::widgets::input_dialog`] overlay pattern.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// One selectable artwork action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkAction {
    SetFromFile,
    FetchOnline,
    Remove,
}

impl ArtworkAction {
    pub const ALL: [ArtworkAction; 3] = [
        ArtworkAction::SetFromFile,
        ArtworkAction::FetchOnline,
        ArtworkAction::Remove,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ArtworkAction::SetFromFile => "Set from file…",
            ArtworkAction::FetchOnline => "Fetch online",
            ArtworkAction::Remove => "Remove artwork",
        }
    }
}

/// State for the artwork menu overlay.
pub struct ArtworkMenuState {
    /// Track ids the action applies to (single or multi-selection).
    pub targets: Vec<i64>,
    /// Highlighted action index.
    pub cursor: usize,
}

impl ArtworkMenuState {
    pub fn new(targets: Vec<i64>) -> Self {
        Self { targets, cursor: 0 }
    }

    pub fn next(&mut self) {
        self.cursor = (self.cursor + 1) % ArtworkAction::ALL.len();
    }

    pub fn prev(&mut self) {
        let len = ArtworkAction::ALL.len();
        self.cursor = (self.cursor + len - 1) % len;
    }

    pub fn selected(&self) -> ArtworkAction {
        ArtworkAction::ALL[self.cursor.min(ArtworkAction::ALL.len() - 1)]
    }
}

/// Render the artwork menu overlay.
pub fn render(frame: &mut Frame, state: &ArtworkMenuState) {
    let area = centered_rect(40, 8, frame.area());
    frame.render_widget(Clear, area);

    let title = if state.targets.len() > 1 {
        format!(" Artwork — {} tracks ", state.targets.len())
    } else {
        " Artwork ".to_string()
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for (i, action) in ArtworkAction::ALL.iter().enumerate() {
        let is_cursor = i == state.cursor;
        let marker = if is_cursor { "▶ " } else { "  " };
        let style = if is_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(Span::styled(
            format!("{marker}{}", action.label()),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter: select    Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}
