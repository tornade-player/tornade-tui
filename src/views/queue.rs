use crate::utils::{format_duration, truncate};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use tornade_core::models::{RepeatMode, Track};

/// 1-row filter bar at the top of the right panel.
pub fn render_filter_bar(frame: &mut Frame, area: Rect, filter: &str, active: bool) {
    let cursor = if active { "_" } else { "" };
    let (text, style) = if filter.is_empty() && !active {
        (
            "\u{f002}  Filter queue...".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            format!("\u{f002}  {}{}", filter, cursor),
            Style::default().fg(Color::White),
        )
    };
    let bg = if active {
        Style::default().bg(Color::Rgb(40, 42, 54))
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, style))).style(bg),
        area,
    );
}

/// Hit zones for the 5 toolbar buttons (above the player bar).
#[derive(Default, Clone, Copy)]
pub struct ToolbarHitZones {
    pub random: Option<Rect>,
    pub repeat: Option<Rect>,
    pub shuffle: Option<Rect>,
    pub add: Option<Rect>,
    pub remove: Option<Rect>,
}

/// 3-row toolbar below the queue list: icons centered vertically for bigger click targets.
pub fn render_toolbar(
    frame: &mut Frame,
    area: Rect,
    shuffle: bool,
    repeat: &RepeatMode,
) -> ToolbarHitZones {
    let col_active = Color::Cyan;
    let col_dim = Color::DarkGray;

    let (repeat_icon, repeat_col) = match repeat {
        RepeatMode::Off => ("↺", col_dim),
        RepeatMode::All => ("↺", col_active),
        RepeatMode::One => ("↺1", col_active),
    };
    let shuffle_col = if shuffle { col_active } else { col_dim };

    let cells = Layout::horizontal([
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
    ])
    .split(area);

    let buttons: [(&str, Color); 5] = [
        ("\u{f522}", col_dim), // nf-fa-random
        (repeat_icon, repeat_col),
        ("⇄", shuffle_col),
        ("⊕", col_dim),
        ("✕", col_dim),
    ];

    for (cell, (icon, col)) in cells.iter().zip(buttons.iter()) {
        // Vertically center the icon in the toolbar area (split into top pad / icon / bottom pad)
        let vert = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(*cell);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                icon.to_string(),
                Style::default().fg(*col).add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            vert[1],
        );
    }

    ToolbarHitZones {
        random: Some(cells[0]),
        repeat: Some(cells[1]),
        shuffle: Some(cells[2]),
        add: Some(cells[3]),
        remove: Some(cells[4]),
    }
}

/// Compact read-only queue for the always-visible right panel, with optional filter.
#[allow(clippy::too_many_arguments)]
pub fn render_panel(
    frame: &mut Frame,
    area: Rect,
    tracks: &[Track],
    active_index: usize,
    skipped_ids: &[i64],
    panel_state: &mut ListState,
    focused: bool,
    filter: &str,
) {
    let filter_lower = filter.to_lowercase();
    let filtered: Vec<(usize, &Track)> = if filter.is_empty() {
        tracks.iter().enumerate().collect()
    } else {
        tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.title.to_lowercase().contains(&filter_lower)
                    || t.artist_names
                        .iter()
                        .any(|a| a.to_lowercase().contains(&filter_lower))
            })
            .collect()
    };

    let inner_w = (area.width as usize).saturating_sub(2);
    let title_w = inner_w.saturating_sub(13);

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(orig_idx, t)| {
            let is_active = *orig_idx == active_index;
            let is_skipped = skipped_ids.contains(&t.id);
            let dur = format_duration(t.duration.as_secs());
            let indicator = if is_active { "▶" } else { " " };
            let line = Line::from(vec![
                Span::styled(
                    format!("{:>3}. {}", orig_idx + 1, indicator),
                    Style::default().fg(if is_active {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(format!(
                    "{:<width$} ",
                    truncate(&t.title, title_w),
                    width = title_w
                )),
                Span::styled(format!("{:>5}", dur), Style::default().fg(Color::DarkGray)),
            ]);
            let style = if is_skipped {
                Style::default().fg(Color::Red)
            } else if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let hl_style = if focused {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let list = List::new(items)
        .block(Block::default())
        .highlight_style(hl_style);
    frame.render_stateful_widget(list, area, panel_state);
}

#[derive(Default)]
pub struct QueueState {
    pub list_state: ListState,
}

impl QueueState {
    pub fn sync_selection(&mut self, queue_len: usize, active_index: usize) {
        if queue_len == 0 {
            self.list_state.select(None);
        } else if self.list_state.selected().is_none() {
            self.list_state
                .select(Some(active_index.min(queue_len - 1)));
        }
    }

    pub fn move_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let n = self
            .list_state
            .selected()
            .map(|i| (i + 1).min(len - 1))
            .unwrap_or(0);
        self.list_state.select(Some(n));
    }

    pub fn move_up(&mut self) {
        let p = self
            .list_state
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.list_state.select(Some(p));
    }

    pub fn jump_top(&mut self) {
        self.list_state.select(Some(0));
    }
    pub fn jump_bottom(&mut self, len: usize) {
        if len > 0 {
            self.list_state.select(Some(len - 1));
        }
    }

    #[allow(clippy::too_many_arguments)] // render args are passed individually by design
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        tracks: &[Track],
        active_index: usize,
        skipped_ids: &[i64],
        focused: bool,
        selection: &crate::app::Selection,
    ) {
        let items: Vec<ListItem> = tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let is_active = i == active_index;
                let is_skipped = skipped_ids.contains(&t.id);
                let dur = format_duration(t.duration.as_secs());
                let artist = t.artist_names.first().cloned().unwrap_or_default();
                let indicator = if is_active { "▶ " } else { "  " };
                let mut spans = Vec::new();
                if let Some(marker) = crate::widgets::selection::marker_span(selection, t.id) {
                    spans.push(marker);
                }
                spans.extend([
                    Span::styled(
                        format!("{:>3}. {}", i + 1, indicator),
                        Style::default().fg(if is_active {
                            Color::Cyan
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::raw(format!("{:<38} ", truncate(&t.title, 37))),
                    Span::styled(
                        format!("{:<22} ", truncate(&artist, 21)),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(format!("{:>5}", dur), Style::default().fg(Color::DarkGray)),
                ]);
                let line = Line::from(spans);
                let style = if is_skipped {
                    Style::default().fg(Color::Red)
                } else if is_active {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(line).style(style)
            })
            .collect();

        let (hl_style, hl_sym) = if focused {
            (
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
                "> ",
            )
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        let list = List::new(items)
            .block(Block::default())
            .highlight_style(hl_style)
            .highlight_symbol(hl_sym);
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}
