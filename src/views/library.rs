use crate::utils::{format_audio, format_duration, format_rating, truncate};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};
use tornade_core::models::Track;
use tornade_core::services::LibraryService;

const PAGE_SIZE: usize = 50;

pub struct LibraryState {
    /// Full unfiltered library (loaded on init / reload).
    pub tracks: Vec<Track>,
    /// DB search results when filter is active and non-empty.
    pub search_results: Option<Vec<Track>>,
    pub list_state: ListState,
    pub total_count: i64,
    pub filter: String,
    pub filter_active: bool,
    pub skipped_ids: Vec<i64>,
    scrollbar_state: ScrollbarState,
    /// Area of the track list (set each frame during render, used for mouse hit detection).
    pub list_area: Option<Rect>,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            search_results: None,
            list_state: ListState::default(),
            total_count: 0,
            filter: String::new(),
            filter_active: false,
            skipped_ids: Vec::new(),
            scrollbar_state: ScrollbarState::default(),
            list_area: None,
        }
    }
}

impl LibraryState {
    /// Load (or reload) the full unfiltered library. Search results are managed separately.
    pub fn load(&mut self, library: &LibraryService) {
        self.tracks = library
            .list_sources()
            .unwrap_or_default()
            .iter()
            .flat_map(|s| library.get_source_tracks(s.id).unwrap_or_default())
            .collect();
        self.total_count = self.tracks.len() as i64;
        if self.list_state.selected().is_none() && !self.tracks.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Active display list: search results when filter is active, full library otherwise.
    pub fn display_tracks(&self) -> &[Track] {
        self.search_results.as_deref().unwrap_or(&self.tracks)
    }

    pub fn selected_track(&self) -> Option<&Track> {
        self.list_state
            .selected()
            .and_then(|i| self.display_tracks().get(i))
    }

    pub fn visible_track_ids(&self) -> Vec<i64> {
        self.display_tracks().iter().map(|t| t.id).collect()
    }

    pub fn move_down(&mut self) {
        let len = self.display_tracks().len();
        if len == 0 {
            return;
        }
        let next = self
            .list_state
            .selected()
            .map(|i| (i + 1).min(len - 1))
            .unwrap_or(0);
        self.list_state.select(Some(next));
    }

    pub fn move_up(&mut self) {
        let prev = self
            .list_state
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.list_state.select(Some(prev));
    }

    pub fn page_down(&mut self) {
        let len = self.display_tracks().len();
        if len == 0 {
            return;
        }
        let next = self
            .list_state
            .selected()
            .map(|i| (i + 10).min(len - 1))
            .unwrap_or(0);
        self.list_state.select(Some(next));
    }

    pub fn page_up(&mut self) {
        let prev = self
            .list_state
            .selected()
            .map(|i| i.saturating_sub(10))
            .unwrap_or(0);
        self.list_state.select(Some(prev));
    }

    pub fn jump_top(&mut self) {
        if !self.display_tracks().is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn jump_bottom(&mut self) {
        let len = self.display_tracks().len();
        if len > 0 {
            self.list_state.select(Some(len - 1));
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
        render_search_bar(frame, chunks[0], &self.filter, self.filter_active);
        self.list_area = Some(chunks[2]);

        if self.total_count == 0 && self.filter.is_empty() {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Library is empty",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Press  :scan <path>  or  s  to scan a music folder."),
            ])
            .block(Block::default())
            .alignment(Alignment::Center);
            frame.render_widget(msg, chunks[2]);
            return;
        }

        let display: Vec<Track> = self.display_tracks().to_vec();
        let items: Vec<ListItem> = display
            .iter()
            .map(|t| {
                let is_skipped = self.skipped_ids.contains(&t.id);
                let dur = format_duration(t.duration.as_secs());
                let fmt = format_audio(t.file_type, t.sample_rate, t.bit_depth);
                let rating = format_rating(t.rating.0);
                let artist = t.artist_names.first().cloned().unwrap_or_default();
                let line = Line::from(vec![
                    Span::raw(format!("{:<40} ", truncate(&t.title, 39))),
                    Span::styled(
                        format!("{:<25} ", truncate(&artist, 24)),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(format!("{:>5} ", dur), Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{:<12} ", fmt),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(rating, Style::default().fg(Color::Yellow)),
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

        frame.render_stateful_widget(list, chunks[2], &mut self.list_state);

        let visible_pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(display.len()).position(visible_pos);
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
        (
            "\u{f002}  Search...".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            format!("\u{f002}  {}{}", filter, cursor),
            Style::default().fg(Color::White),
        )
    };
    let bg = if active {
        Style::default().bg(Color::Rgb(40, 42, 54))
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, style))).style(bg),
        area,
    );
}
