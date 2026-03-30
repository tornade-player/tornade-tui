use ratatui::{Frame, layout::Rect, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, List, ListItem, ListState}};
use tornade_core::models::Track;
use crate::utils::{format_duration, truncate};

pub struct QueueState {
    pub list_state: ListState,
}

impl Default for QueueState {
    fn default() -> Self {
        Self { list_state: ListState::default() }
    }
}

impl QueueState {
    pub fn sync_selection(&mut self, queue_len: usize, active_index: usize) {
        if queue_len == 0 {
            self.list_state.select(None);
        } else if self.list_state.selected().is_none() {
            self.list_state.select(Some(active_index.min(queue_len - 1)));
        }
    }

    pub fn move_down(&mut self, len: usize) {
        if len == 0 { return; }
        let n = self.list_state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(n));
    }

    pub fn move_up(&mut self) {
        let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(p));
    }

    pub fn jump_top(&mut self) { self.list_state.select(Some(0)); }
    pub fn jump_bottom(&mut self, len: usize) { if len > 0 { self.list_state.select(Some(len - 1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, tracks: &[Track], active_index: usize, skipped_ids: &[i64]) {
        let items: Vec<ListItem> = tracks.iter().enumerate().map(|(i, t)| {
            let is_active = i == active_index;
            let is_skipped = skipped_ids.contains(&t.id);
            let dur = format_duration(t.duration.as_secs());
            let artist = t.artist_names.first().cloned().unwrap_or_default();
            let indicator = if is_active { "▶ " } else { "  " };
            let line = Line::from(vec![
                Span::styled(format!("{:>3}. {}", i + 1, indicator), Style::default().fg(if is_active { Color::Cyan } else { Color::DarkGray })),
                Span::raw(format!("{:<38} ", truncate(&t.title, 37))),
                Span::styled(format!("{:<22} ", truncate(&artist, 21)), Style::default().fg(Color::Gray)),
                Span::styled(format!("{:>5}", dur), Style::default().fg(Color::DarkGray)),
            ]);
            let style = if is_skipped {
                Style::default().fg(Color::Red)
            } else if is_active {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(" Queue ({} tracks)  [J/K reorder  x remove  X clear] ", tracks.len())))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}
