use ratatui::{Frame, layout::{Constraint, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState}};
use tornade_core::models::Artist;
use tornade_core::services::LibraryService;
use crate::utils::truncate;

pub struct ArtistsState {
    pub artists: Vec<Artist>,
    pub search_results: Option<Vec<Artist>>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_active: bool,
    scrollbar_state: ScrollbarState,
}

impl Default for ArtistsState {
    fn default() -> Self {
        Self { artists: Vec::new(), search_results: None, list_state: ListState::default(), filter: String::new(), filter_active: false, scrollbar_state: ScrollbarState::default() }
    }
}

impl ArtistsState {
    pub fn load(&mut self, library: &LibraryService) {
        self.artists = library.list_artists().unwrap_or_default();
        if self.list_state.selected().is_none() && !self.artists.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn display_artists(&self) -> &[Artist] {
        self.search_results.as_deref().unwrap_or(&self.artists)
    }

    pub fn selected_artist(&self) -> Option<&Artist> {
        self.list_state.selected().and_then(|i| self.display_artists().get(i))
    }

    pub fn move_down(&mut self) { let len = self.display_artists().len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn move_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn page_down(&mut self) { let len = self.display_artists().len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+10).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn page_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(10)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn jump_top(&mut self) { if !self.display_artists().is_empty() { self.list_state.select(Some(0)); } }
    pub fn jump_bottom(&mut self) { let len = self.display_artists().len(); if len > 0 { self.list_state.select(Some(len-1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)]).split(area);
        render_search_bar(frame, chunks[0], &self.filter, self.filter_active);

        let display: Vec<Artist> = self.display_artists().to_vec();
        let items: Vec<ListItem> = display.iter().map(|a| ListItem::new(Line::from(Span::raw(truncate(&a.name, 60))))).collect();
        let (hl_style, hl_sym) = if focused {
            (Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD), "> ")
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        let list = List::new(items)
            .block(Block::default())
            .highlight_style(hl_style)
            .highlight_symbol(hl_sym);
        frame.render_stateful_widget(list, chunks[2], &mut self.list_state);

        let pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(display.len()).position(pos);
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            chunks[2],
            &mut self.scrollbar_state,
        );
    }
}

fn render_search_bar(frame: &mut Frame, area: Rect, filter: &str, active: bool) {
    let cursor = if active { "_" } else { "" };
    let (text, style) = if filter.is_empty() && !active {
        ("\u{f002}  Search...".to_string(), Style::default().fg(Color::DarkGray))
    } else {
        (format!("\u{f002}  {}{}", filter, cursor), Style::default().fg(Color::White))
    };
    let bg = if active { Style::default().bg(Color::Rgb(40, 42, 54)) } else { Style::default() };
    frame.render_widget(Paragraph::new(Line::from(Span::styled(text, style))).style(bg), area);
}
