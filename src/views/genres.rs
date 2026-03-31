use ratatui::{Frame, layout::Rect, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, List, ListItem, ListState}};
use tornade_core::{models::Genre, services::LibraryService};
use crate::utils::truncate;

pub struct GenresState {
    pub genres: Vec<(Genre, u32, u32)>,
    pub list_state: ListState,
}

impl Default for GenresState {
    fn default() -> Self {
        Self { genres: Vec::new(), list_state: ListState::default() }
    }
}

impl GenresState {
    pub fn load(&mut self, library: &LibraryService) {
        self.genres = library.list_genres().unwrap_or_default();
        if self.list_state.selected().is_none() && !self.genres.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn selected_genre(&self) -> Option<&(Genre, u32, u32)> {
        self.list_state.selected().and_then(|i| self.genres.get(i))
    }

    pub fn move_down(&mut self) { let len = self.genres.len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn move_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn jump_top(&mut self) { if !self.genres.is_empty() { self.list_state.select(Some(0)); } }
    pub fn jump_bottom(&mut self) { if !self.genres.is_empty() { self.list_state.select(Some(self.genres.len()-1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let items: Vec<ListItem> = self.genres.iter().map(|(g, tracks, albums)| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<40} ", truncate(&g.name, 39))),
                Span::styled(format!("{} tracks", tracks), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("  {} albums", albums), Style::default().fg(Color::DarkGray)),
            ]))
        }).collect();
        let (hl_style, hl_sym) = if focused {
            (Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD), "> ")
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(" Genres ({}) ", self.genres.len())))
            .highlight_style(hl_style)
            .highlight_symbol(hl_sym);
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}
