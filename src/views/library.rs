use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use tornade_core::{models::Track, services::LibraryService};
use crate::utils::{format_audio, format_duration, format_rating, truncate};

const PAGE_SIZE: usize = 50;

pub struct LibraryState {
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub page: usize,
    pub total_count: i64,
    pub filter: String,
    pub filter_active: bool,
    pub skipped_ids: Vec<i64>,
    scrollbar_state: ScrollbarState,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            list_state: ListState::default(),
            page: 0,
            total_count: 0,
            filter: String::new(),
            filter_active: false,
            skipped_ids: Vec::new(),
            scrollbar_state: ScrollbarState::default(),
        }
    }
}

impl LibraryState {
    pub fn load(&mut self, library: &LibraryService) {
        let all_tracks: Vec<Track> = library
            .list_sources()
            .unwrap_or_default()
            .iter()
            .flat_map(|s| library.get_source_tracks(s.id).unwrap_or_default())
            .collect();

        if self.filter.is_empty() {
            self.total_count = all_tracks.len() as i64;
            self.tracks = all_tracks;
        } else {
            let q = self.filter.to_lowercase();
            self.tracks = all_tracks
                .into_iter()
                .filter(|t| {
                    t.title.to_lowercase().contains(&q)
                        || t.artist_names.iter().any(|a| a.to_lowercase().contains(&q))
                })
                .collect();
            self.total_count = self.tracks.len() as i64;
        }

        if self.list_state.selected().is_none() && !self.tracks.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn selected_track(&self) -> Option<&Track> {
        self.list_state.selected().and_then(|i| self.tracks.get(i))
    }

    pub fn visible_track_ids(&self) -> Vec<i64> {
        self.tracks.iter().map(|t| t.id).collect()
    }

    pub fn move_down(&mut self) {
        let len = self.tracks.len();
        if len == 0 { return; }
        let next = self.list_state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(next));
    }

    pub fn move_up(&mut self) {
        let prev = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(prev));
    }

    pub fn page_down(&mut self) {
        let len = self.tracks.len();
        if len == 0 { return; }
        let next = self.list_state.selected().map(|i| (i + 10).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(next));
    }

    pub fn page_up(&mut self) {
        let prev = self.list_state.selected().map(|i| i.saturating_sub(10)).unwrap_or(0);
        self.list_state.select(Some(prev));
    }

    pub fn jump_top(&mut self) {
        if !self.tracks.is_empty() { self.list_state.select(Some(0)); }
    }

    pub fn jump_bottom(&mut self) {
        if !self.tracks.is_empty() {
            self.list_state.select(Some(self.tracks.len() - 1));
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        if self.total_count == 0 && self.filter.is_empty() {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Library is empty",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Press  :scan <path>  or  s  to scan a music folder."),
            ])
            .block(Block::default().borders(Borders::ALL).title(" Tracks "))
            .alignment(Alignment::Center);
            frame.render_widget(msg, area);
            return;
        }

        let title = if self.filter_active || !self.filter.is_empty() {
            format!(" Tracks [filter: {}] ", self.filter)
        } else {
            format!(" Tracks ({} total) ", self.total_count)
        };

        let items: Vec<ListItem> = self.tracks.iter().map(|t| {
            let is_skipped = self.skipped_ids.contains(&t.id);
            let dur = format_duration(t.duration.as_secs());
            let fmt = format_audio(t.file_type, t.sample_rate, t.bit_depth);
            let rating = format_rating(t.rating.0);
            let artist = t.artist_names.first().cloned().unwrap_or_default();
            let line = Line::from(vec![
                Span::raw(format!("{:<40} ", truncate(&t.title, 39))),
                Span::styled(
                    format!("{:<25} ", truncate(&artist, 24)),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("{:>5} ", dur),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:<12} ", fmt),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(rating, Style::default().fg(Color::Yellow)),
            ]);
            let style = if is_skipped {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        }).collect();

        let (hl_style, hl_sym) = if focused {
            (Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD), "> ")
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(hl_style)
            .highlight_symbol(hl_sym);

        frame.render_stateful_widget(list, area, &mut self.list_state);

        let visible_pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(self.tracks.len()).position(visible_pos);
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            area,
            &mut self.scrollbar_state,
        );
    }
}
