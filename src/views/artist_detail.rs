use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, List, ListItem, ListState, Paragraph}};
use tornade_core::{models::{Album, Artist}, services::LibraryService};
use crate::utils::truncate;

pub struct ArtistDetailState {
    pub artist: Artist,
    pub albums: Vec<Album>,
    pub list_state: ListState,
}

impl ArtistDetailState {
    pub fn new(artist: Artist, library: &LibraryService) -> Self {
        let albums = library.get_artist_albums(artist.id).unwrap_or_default();
        let mut list_state = ListState::default();
        if !albums.is_empty() { list_state.select(Some(0)); }
        Self { artist, albums, list_state }
    }

    pub fn selected_album(&self) -> Option<&Album> {
        self.list_state.selected().and_then(|i| self.albums.get(i))
    }

    pub fn move_down(&mut self) { let len = self.albums.len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn move_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn jump_top(&mut self) { if !self.albums.is_empty() { self.list_state.select(Some(0)); } }
    pub fn jump_bottom(&mut self) { if !self.albums.is_empty() { self.list_state.select(Some(self.albums.len()-1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(area);

        let header = Paragraph::new(vec![
            Line::from(Span::styled(truncate(&self.artist.name, 60), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("{} albums", self.albums.len()), Style::default().fg(Color::DarkGray))),
        ])
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, chunks[0]);

        let items: Vec<ListItem> = self.albums.iter().map(|a| {
            let year = a.year.map(|y| format!("  {}", y)).unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<45} ", truncate(&a.title, 44))),
                Span::styled(year, Style::default().fg(Color::DarkGray)),
            ]))
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Albums "))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut self.list_state);
    }
}
