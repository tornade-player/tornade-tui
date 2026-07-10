use crate::utils::{format_duration, format_rating, truncate};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use tornade_core::{
    models::{Playlist, Track},
    services::LibraryService,
};

pub struct PlaylistDetailState {
    pub playlist: Playlist,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub skipped_ids: Vec<i64>,
}

impl PlaylistDetailState {
    pub fn new(playlist: Playlist, library: &LibraryService) -> Self {
        let tracks: Vec<Track> = playlist
            .tracks
            .iter()
            .filter_map(|pt| library.get_track(pt.track_id).ok().flatten())
            .collect();
        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            playlist,
            tracks,
            list_state,
            skipped_ids: Vec::new(),
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
        if !self.tracks.is_empty() {
            self.list_state.select(Some(0));
        }
    }
    pub fn jump_bottom(&mut self) {
        if !self.tracks.is_empty() {
            self.list_state.select(Some(self.tracks.len() - 1));
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        selection: &crate::app::Selection,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let mut header_spans = vec![Span::styled(
            format!(
                "{} · {} tracks",
                truncate(&self.playlist.name, 40),
                self.tracks.len()
            ),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )];
        if let Some(count) = crate::widgets::selection::count_span(selection) {
            header_spans.push(count);
        }
        let header = Paragraph::new(Line::from(header_spans)).block(Block::default());
        frame.render_widget(header, chunks[0]);

        let items: Vec<ListItem> = self
            .tracks
            .iter()
            .enumerate()
            .map(|(pos, t)| {
                let is_skipped = self.skipped_ids.contains(&t.id);
                let dur = format_duration(t.duration.as_secs());
                let artist = t.artist_names.first().cloned().unwrap_or_default();
                let rating = format_rating(t.rating.0);
                let mut spans = Vec::new();
                if let Some(marker) = crate::widgets::selection::marker_span(selection, t.id) {
                    spans.push(marker);
                }
                spans.extend([
                    Span::styled(
                        format!("{:>3}. ", pos + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(format!("{:<38} ", truncate(&t.title, 37))),
                    Span::styled(
                        format!("{:<22} ", truncate(&artist, 21)),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(format!("{:>5} ", dur), Style::default().fg(Color::DarkGray)),
                    Span::styled(rating, Style::default().fg(Color::Yellow)),
                ]);
                let line = Line::from(spans);
                let style = if is_skipped {
                    Style::default().fg(Color::Red)
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
        frame.render_stateful_widget(list, chunks[1], &mut self.list_state);
    }
}
