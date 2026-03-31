use ratatui::{Frame, layout::Rect, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState}};
use tornade_core::{models::Artist, services::LibraryService};
use crate::utils::truncate;

pub struct ArtistsState {
    pub artists: Vec<Artist>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_active: bool,
    scrollbar_state: ScrollbarState,
}

impl Default for ArtistsState {
    fn default() -> Self {
        Self { artists: Vec::new(), list_state: ListState::default(), filter: String::new(), filter_active: false, scrollbar_state: ScrollbarState::default() }
    }
}

impl ArtistsState {
    pub fn load(&mut self, library: &LibraryService) {
        self.artists = library.list_artists().unwrap_or_default();
        if self.list_state.selected().is_none() && !self.artists.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn filtered_artists(&self) -> Vec<&Artist> {
        if self.filter.is_empty() {
            self.artists.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.artists.iter().filter(|a| a.name.to_lowercase().contains(&q)).collect()
        }
    }

    pub fn selected_artist(&self) -> Option<&Artist> {
        let filtered = self.filtered_artists();
        self.list_state.selected().and_then(|i| filtered.get(i).copied())
    }

    pub fn move_down(&mut self) { let len = self.filtered_artists().len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn move_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn page_down(&mut self) { let len = self.filtered_artists().len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+10).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn page_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(10)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn jump_top(&mut self) { if !self.artists.is_empty() { self.list_state.select(Some(0)); } }
    pub fn jump_bottom(&mut self) { let len = self.filtered_artists().len(); if len > 0 { self.list_state.select(Some(len-1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let filtered = self.filtered_artists();
        let filtered_len = filtered.len();
        let title = if !self.filter.is_empty() { format!(" Artists [filter: {}] ", self.filter) } else { format!(" Artists ({}) ", filtered_len) };
        let items: Vec<ListItem> = filtered.iter().map(|a| ListItem::new(Line::from(Span::raw(truncate(&a.name, 60))))).collect();
        drop(filtered);
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

        let pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(filtered_len).position(pos);
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            area,
            &mut self.scrollbar_state,
        );
    }
}
