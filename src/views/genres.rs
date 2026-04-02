use ratatui::{Frame, layout::{Constraint, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState}};
use tornade_core::{models::Genre, services::LibraryService};
use crate::utils::truncate;

pub struct GenresState {
    pub genres: Vec<(Genre, u32, u32)>,
    pub filter: String,
    pub filter_active: bool,
    pub list_state: ListState,
    scrollbar_state: ScrollbarState,
}

impl Default for GenresState {
    fn default() -> Self {
        Self { genres: Vec::new(), filter: String::new(), filter_active: false, list_state: ListState::default(), scrollbar_state: ScrollbarState::default() }
    }
}

impl GenresState {
    pub fn load(&mut self, library: &LibraryService) {
        self.genres = library.list_genres().unwrap_or_default();
        if self.list_state.selected().is_none() && !self.genres.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn filtered_genres(&self) -> Vec<&(Genre, u32, u32)> {
        if self.filter.is_empty() {
            self.genres.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.genres.iter().filter(|(g, _, _)| g.name.to_lowercase().contains(&q)).collect()
        }
    }

    pub fn selected_genre(&self) -> Option<&(Genre, u32, u32)> {
        let filtered = self.filtered_genres();
        self.list_state.selected().and_then(|i| filtered.get(i).copied())
    }

    pub fn move_down(&mut self) { let len = self.filtered_genres().len(); if len == 0 { return; } let n = self.list_state.selected().map(|i| (i+1).min(len-1)).unwrap_or(0); self.list_state.select(Some(n)); }
    pub fn move_up(&mut self) { let p = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0); self.list_state.select(Some(p)); }
    pub fn jump_top(&mut self) { if !self.genres.is_empty() { self.list_state.select(Some(0)); } }
    pub fn jump_bottom(&mut self) { let len = self.filtered_genres().len(); if len > 0 { self.list_state.select(Some(len-1)); } }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)]).split(area);
        render_search_bar(frame, chunks[0], &self.filter, self.filter_active);

        let filtered = self.filtered_genres();
        let filtered_len = filtered.len();
        let items: Vec<ListItem> = filtered.iter().map(|(g, tracks, albums)| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<40} ", truncate(&g.name, 39))),
                Span::styled(format!("{} tracks", tracks), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("  {} albums", albums), Style::default().fg(Color::DarkGray)),
            ]))
        }).collect();
        drop(filtered);
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
        self.scrollbar_state = ScrollbarState::new(filtered_len).position(pos);
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
