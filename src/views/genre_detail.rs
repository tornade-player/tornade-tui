use crate::utils::{format_duration, truncate};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use tornade_core::{
    models::{Genre, Track},
    services::LibraryService,
};

pub struct GenreDetailState {
    pub genre: Genre,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub skipped_ids: Vec<i64>,
}

impl GenreDetailState {
    pub fn new(genre: Genre, library: &LibraryService) -> Self {
        let tracks = library.get_genre_tracks(genre.id).unwrap_or_default();
        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            genre,
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let header = Paragraph::new(Line::from(Span::styled(
            format!(
                "{} · {} tracks",
                truncate(&self.genre.name, 40),
                self.tracks.len()
            ),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )))
        .block(Block::default());
        frame.render_widget(header, chunks[0]);

        let items: Vec<ListItem> = self
            .tracks
            .iter()
            .map(|t| {
                let is_skipped = self.skipped_ids.contains(&t.id);
                let dur = format_duration(t.duration.as_secs());
                let artist = t.artist_names.first().cloned().unwrap_or_default();
                let line = Line::from(vec![
                    Span::raw(format!("{:<40} ", truncate(&t.title, 39))),
                    Span::styled(
                        format!("{:<25} ", truncate(&artist, 24)),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(format!("{:>5}", dur), Style::default().fg(Color::DarkGray)),
                ]);
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
