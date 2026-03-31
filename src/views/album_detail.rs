use ratatui::{
    Frame, layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use tornade_core::{models::{Album, Track}, services::LibraryService};
use crate::utils::{format_duration, format_rating, truncate};

pub struct AlbumDetailState {
    pub album: Album,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub skipped_ids: Vec<i64>,
}

impl AlbumDetailState {
    pub fn new(album: Album, library: &LibraryService) -> Self {
        let tracks = library.get_album_tracks(album.id).unwrap_or_default();
        let mut list_state = ListState::default();
        if !tracks.is_empty() { list_state.select(Some(0)); }
        Self { album, tracks, list_state, skipped_ids: Vec::new() }
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
        let n = self.list_state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(n));
    }

    pub fn move_up(&mut self) {
        let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(p));
    }

    pub fn jump_top(&mut self) { if !self.tracks.is_empty() { self.list_state.select(Some(0)); } }
    pub fn jump_bottom(&mut self) { if !self.tracks.is_empty() { self.list_state.select(Some(self.tracks.len()-1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(0)])
            .split(area);

        // Header
        let year = self.album.year.map(|y| format!(" · {}", y)).unwrap_or_default();
        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                truncate(&self.album.title, 60),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("{}{}", truncate(&self.album.artist_name, 40), year),
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                format!("{} tracks", self.tracks.len()),
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, chunks[0]);

        // Track list
        let items: Vec<ListItem> = self.tracks.iter().map(|t| {
            let is_skipped = self.skipped_ids.contains(&t.id);
            let num = t.track_number.map(|n| format!("{:>2}. ", n)).unwrap_or_else(|| "    ".to_string());
            let dur = format_duration(t.duration.as_secs());
            let rating = format_rating(t.rating.0);
            let line = Line::from(vec![
                Span::styled(num, Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{:<40} ", truncate(&t.title, 39))),
                Span::styled(format!("{:>5} ", dur), Style::default().fg(Color::DarkGray)),
                Span::styled(rating, Style::default().fg(Color::Yellow)),
            ]);
            let style = if is_skipped { Style::default().fg(Color::Red) } else { Style::default() };
            ListItem::new(line).style(style)
        }).collect();

        let (hl_style, hl_sym) = if focused {
            (Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD), "> ")
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Tracks "))
            .highlight_style(hl_style)
            .highlight_symbol(hl_sym);
        frame.render_stateful_widget(list, chunks[1], &mut self.list_state);
    }
}
