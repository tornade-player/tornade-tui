use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, List, ListItem, ListState, Paragraph}};
use tornade_core::{models::{Album, Artist, Track}, services::SearchService};
use crate::utils::{format_duration, truncate};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SearchSection { #[default] Tracks, Albums, Artists }

pub struct SearchState {
    pub query: String,
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub section: SearchSection,
    pub tracks_state: ListState,
    pub albums_state: ListState,
    pub artists_state: ListState,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            tracks: Vec::new(), albums: Vec::new(), artists: Vec::new(),
            section: SearchSection::Tracks,
            tracks_state: ListState::default(),
            albums_state: ListState::default(),
            artists_state: ListState::default(),
        }
    }
}

impl SearchState {
    pub fn run_search_with_query(&mut self, query: &str, svc: &SearchService) {
        if query.is_empty() {
            self.tracks.clear(); self.albums.clear(); self.artists.clear();
            return;
        }
        if let Ok(results) = svc.search(query) {
            self.tracks = results.tracks;
            self.albums = results.albums;
            self.artists = results.artists;
            if !self.tracks.is_empty() { self.tracks_state.select(Some(0)); }
            if !self.albums.is_empty() { self.albums_state.select(Some(0)); }
            if !self.artists.is_empty() { self.artists_state.select(Some(0)); }
        }
    }

    pub fn next_section(&mut self) {
        self.section = match self.section {
            SearchSection::Tracks => SearchSection::Albums,
            SearchSection::Albums => SearchSection::Artists,
            SearchSection::Artists => SearchSection::Tracks,
        };
    }

    pub fn move_down(&mut self) {
        match self.section {
            SearchSection::Tracks => { let len = self.tracks.len(); if len > 0 { let n = self.tracks_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.tracks_state.select(Some(n)); } }
            SearchSection::Albums => { let len = self.albums.len(); if len > 0 { let n = self.albums_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.albums_state.select(Some(n)); } }
            SearchSection::Artists => { let len = self.artists.len(); if len > 0 { let n = self.artists_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.artists_state.select(Some(n)); } }
        }
    }

    pub fn move_up(&mut self) {
        match self.section {
            SearchSection::Tracks => { let p = self.tracks_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.tracks_state.select(Some(p)); }
            SearchSection::Albums => { let p = self.albums_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.albums_state.select(Some(p)); }
            SearchSection::Artists => { let p = self.artists_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.artists_state.select(Some(p)); }
        }
    }

    pub fn selected_track(&self) -> Option<&Track> { self.tracks_state.selected().and_then(|i| self.tracks.get(i)) }
    pub fn selected_album(&self) -> Option<&Album> { self.albums_state.selected().and_then(|i| self.albums.get(i)) }
    pub fn selected_artist(&self) -> Option<&Artist> { self.artists_state.selected().and_then(|i| self.artists.get(i)) }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let chunks = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Ratio(1,3), Constraint::Ratio(1,3), Constraint::Ratio(1,3)])
            .split(area);

        let query_bar = Paragraph::new(Line::from(vec![
            Span::styled("Search: ", Style::default().fg(Color::Cyan)),
            Span::raw(&self.query),
        ])).block(Block::default());
        frame.render_widget(query_bar, chunks[0]);

        self.render_section(frame, chunks[1], SearchSection::Tracks, focused);
        self.render_section(frame, chunks[2], SearchSection::Albums, focused);
        self.render_section(frame, chunks[3], SearchSection::Artists, focused);
    }

    fn render_section(&mut self, frame: &mut Frame, area: Rect, section: SearchSection, focused: bool) {
        let is_active = self.section == section;
        let (hl_style, hl_sym) = if focused && is_active {
            (Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD), "> ")
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        match section {
            SearchSection::Tracks => {
                let items: Vec<ListItem> = self.tracks.iter().map(|t| {
                    let artist = t.artist_names.first().cloned().unwrap_or_default();
                    ListItem::new(Line::from(vec![
                        Span::raw(format!("{:<38} ", truncate(&t.title, 37))),
                        Span::styled(truncate(&artist, 25), Style::default().fg(Color::Gray)),
                        Span::styled(format!("  {}", format_duration(t.duration.as_secs())), Style::default().fg(Color::DarkGray)),
                    ]))
                }).collect();
                let list = List::new(items)
                    .block(Block::default())
                    .highlight_style(hl_style)
                    .highlight_symbol(hl_sym);
                frame.render_stateful_widget(list, area, &mut self.tracks_state);
            }
            SearchSection::Albums => {
                let items: Vec<ListItem> = self.albums.iter().map(|a| {
                    ListItem::new(Line::from(vec![
                        Span::raw(format!("{:<38} ", truncate(&a.title, 37))),
                        Span::styled(truncate(&a.artist_name, 25), Style::default().fg(Color::Gray)),
                    ]))
                }).collect();
                let list = List::new(items)
                    .block(Block::default())
                    .highlight_style(hl_style)
                    .highlight_symbol(hl_sym);
                frame.render_stateful_widget(list, area, &mut self.albums_state);
            }
            SearchSection::Artists => {
                let items: Vec<ListItem> = self.artists.iter().map(|a| {
                    ListItem::new(Line::from(Span::raw(truncate(&a.name, 60))))
                }).collect();
                let list = List::new(items)
                    .block(Block::default())
                    .highlight_style(hl_style)
                    .highlight_symbol(hl_sym);
                frame.render_stateful_widget(list, area, &mut self.artists_state);
            }
        }
    }
}
