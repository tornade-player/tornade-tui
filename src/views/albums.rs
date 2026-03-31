use ratatui::{
    Frame, layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use tornade_core::{models::Album, services::LibraryService};
use crate::utils::truncate;

pub struct AlbumsState {
    pub albums: Vec<Album>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_active: bool,
}

impl Default for AlbumsState {
    fn default() -> Self {
        Self {
            albums: Vec::new(),
            list_state: ListState::default(),
            filter: String::new(),
            filter_active: false,
        }
    }
}

impl AlbumsState {
    pub fn load(&mut self, library: &LibraryService) {
        self.albums = library.list_albums(None, None, None, Some(5000), Some(0)).unwrap_or_default();
        if self.list_state.selected().is_none() && !self.albums.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn filtered_albums(&self) -> Vec<&Album> {
        if self.filter.is_empty() {
            self.albums.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.albums
                .iter()
                .filter(|a| {
                    a.title.to_lowercase().contains(&q)
                        || a.artist_name.to_lowercase().contains(&q)
                })
                .collect()
        }
    }

    pub fn selected_album(&self) -> Option<&Album> {
        let filtered = self.filtered_albums();
        self.list_state.selected().and_then(|i| filtered.get(i).copied())
    }

    pub fn move_down(&mut self) {
        let len = self.filtered_albums().len();
        if len == 0 { return; }
        let n = self.list_state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(n));
    }

    pub fn move_up(&mut self) {
        let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(p));
    }

    pub fn page_down(&mut self) {
        let len = self.filtered_albums().len();
        if len == 0 { return; }
        let n = self.list_state.selected().map(|i| (i + 10).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(n));
    }

    pub fn page_up(&mut self) {
        let p = self.list_state.selected().map(|i| i.saturating_sub(10)).unwrap_or(0);
        self.list_state.select(Some(p));
    }

    pub fn jump_top(&mut self) {
        if !self.albums.is_empty() { self.list_state.select(Some(0)); }
    }

    pub fn jump_bottom(&mut self) {
        let len = self.filtered_albums().len();
        if len > 0 { self.list_state.select(Some(len - 1)); }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let filtered = self.filtered_albums();
        let title = if !self.filter.is_empty() {
            format!(" Albums [filter: {}] ", self.filter)
        } else {
            format!(" Albums ({}) ", filtered.len())
        };
        let items: Vec<ListItem> = filtered.iter().map(|a| {
            let mut spans = vec![
                Span::raw(format!("{:<40} ", truncate(&a.title, 39))),
                Span::styled(truncate(&a.artist_name, 30), Style::default().fg(Color::Gray)),
            ];
            if let Some(y) = a.year {
                spans.push(Span::styled(
                    format!("  {}", y),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            ListItem::new(Line::from(spans))
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
    }
}
