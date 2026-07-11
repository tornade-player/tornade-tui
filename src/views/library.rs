use crate::app::Selection;
use crate::utils::{format_audio, format_duration, format_rating, truncate};
use crate::widgets::selection::{count_span, marker_span};
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

#[allow(dead_code)] // reserved for pagination when library is large
const PAGE_SIZE: usize = 50;

/// Sort key for the library track list.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    None,
    Title,
    Artist,
    Duration,
    Rating,
    Plays,
    LastPlayed,
}

impl SortKey {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "title" => Some(Self::Title),
            "artist" => Some(Self::Artist),
            "duration" | "time" => Some(Self::Duration),
            "rating" => Some(Self::Rating),
            "plays" | "playcount" => Some(Self::Plays),
            "lastplayed" | "last" => Some(Self::LastPlayed),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Duration => "duration",
            Self::Rating => "rating",
            Self::Plays => "plays",
            Self::LastPlayed => "last played",
        }
    }
}

#[derive(Default)]
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
    pub sort_key: SortKey,
    pub sort_desc: bool,
    scrollbar_state: ScrollbarState,
    /// Area of the track list (set each frame during render, used for mouse hit detection).
    pub list_area: Option<Rect>,
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
        self.apply_sort();
        if self.list_state.selected().is_none() && !self.tracks.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Set the sort key. Selecting the current key again flips the direction.
    pub fn set_sort(&mut self, key: SortKey) {
        if self.sort_key == key {
            self.sort_desc = !self.sort_desc;
        } else {
            self.sort_key = key;
            self.sort_desc = false;
        }
        self.apply_sort();
    }

    /// Sort the loaded tracks (and any active search results) in place.
    fn apply_sort(&mut self) {
        let key = self.sort_key;
        let desc = self.sort_desc;
        if key == SortKey::None {
            return;
        }
        let cmp = move |a: &Track, b: &Track| {
            let o = match key {
                SortKey::None => std::cmp::Ordering::Equal,
                SortKey::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortKey::Artist => {
                    let an = a.artist_names.first().map(|s| s.to_lowercase()).unwrap_or_default();
                    let bn = b.artist_names.first().map(|s| s.to_lowercase()).unwrap_or_default();
                    an.cmp(&bn)
                }
                SortKey::Duration => a.duration.cmp(&b.duration),
                SortKey::Rating => a.rating.0.cmp(&b.rating.0),
                SortKey::Plays => a.play_count.cmp(&b.play_count),
                SortKey::LastPlayed => a.last_played_at.cmp(&b.last_played_at),
            };
            if desc { o.reverse() } else { o }
        };
        self.tracks.sort_by(&cmp);
        if let Some(sr) = self.search_results.as_mut() {
            sr.sort_by(&cmp);
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool, selection: &Selection) {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
        render_search_bar(
            frame,
            chunks[0],
            &self.filter,
            self.filter_active,
            selection,
        );
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
                let mut spans = Vec::new();
                if let Some(marker) = marker_span(selection, t.id) {
                    spans.push(marker);
                }
                spans.extend([
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
                let line = Line::from(spans);
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

fn render_search_bar(
    frame: &mut Frame,
    area: Rect,
    filter: &str,
    active: bool,
    selection: &Selection,
) {
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
    let mut spans = vec![Span::styled(text, style)];
    if let Some(count) = count_span(selection) {
        spans.push(count);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(bg), area);
}
