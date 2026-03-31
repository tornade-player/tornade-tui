use ratatui::{Frame, layout::Rect, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState}};
use tornade_core::{models::Playlist, services::PlaylistService};
use crate::utils::truncate;

pub struct PlaylistsState {
    pub playlists: Vec<Playlist>,
    pub list_state: ListState,
    scrollbar_state: ScrollbarState,
}

impl Default for PlaylistsState {
    fn default() -> Self {
        Self { playlists: Vec::new(), list_state: ListState::default(), scrollbar_state: ScrollbarState::default() }
    }
}

impl PlaylistsState {
    pub fn load(&mut self, playlists: &PlaylistService) {
        self.playlists = playlists.list_playlists().unwrap_or_default();
        if self.list_state.selected().is_none() && !self.playlists.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn selected_playlist(&self) -> Option<&Playlist> {
        self.list_state.selected().and_then(|i| self.playlists.get(i))
    }

    pub fn move_down(&mut self) { let len = self.playlists.len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn move_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn jump_top(&mut self) { if !self.playlists.is_empty() { self.list_state.select(Some(0)); } }
    pub fn jump_bottom(&mut self) { if !self.playlists.is_empty() { self.list_state.select(Some(self.playlists.len()-1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let items: Vec<ListItem> = self.playlists.iter().map(|p| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<40} ", truncate(&p.name, 39))),
                Span::styled(format!("{} tracks", p.tracks.len()), Style::default().fg(Color::DarkGray)),
            ]))
        }).collect();
        let (hl_style, hl_sym) = if focused {
            (Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD), "> ")
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(" Playlists ({}) ", self.playlists.len())))
            .highlight_style(hl_style)
            .highlight_symbol(hl_sym);
        frame.render_stateful_widget(list, area, &mut self.list_state);

        let pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(self.playlists.len()).position(pos);
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            area,
            &mut self.scrollbar_state,
        );
    }
}
