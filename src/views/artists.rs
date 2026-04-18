use crate::utils::truncate;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::path::PathBuf;
use tornade_core::models::Artist;
use tornade_core::services::LibraryService;

/// Height in terminal rows for each artist row (image height).
const ROW_HEIGHT: u16 = 4;
/// Width reserved for the artist photo (cols).
const IMG_WIDTH: u16 = 8;

#[derive(Default)]
pub struct ArtistsState {
    pub artists: Vec<Artist>,
    pub search_results: Option<Vec<Artist>>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_active: bool,
    /// Photo paths indexed parallel to `artists` (populated by load, never changes).
    photo_paths: Vec<Option<PathBuf>>,
    /// Lazily loaded image protocols indexed parallel to `artists`.
    /// `None` = not yet loaded; `Some(None)` = no image available.
    image_states: Vec<Option<Option<StatefulProtocol>>>,
    scrollbar_state: ScrollbarState,
}

impl ArtistsState {
    pub fn load(&mut self, library: &LibraryService) {
        self.artists = library.list_artists().unwrap_or_default();
        if self.list_state.selected().is_none() && !self.artists.is_empty() {
            self.list_state.select(Some(0));
        }
        // Only collect paths - no I/O, no image decoding.
        let n = self.artists.len();
        self.photo_paths = self.artists.iter().map(|a| a.photo_path.clone()).collect();
        self.image_states = (0..n).map(|_| None).collect();
    }

    pub fn display_artists(&self) -> &[Artist] {
        self.search_results.as_deref().unwrap_or(&self.artists)
    }

    pub fn selected_artist(&self) -> Option<&Artist> {
        self.list_state
            .selected()
            .and_then(|i| self.display_artists().get(i))
    }

    pub fn move_down(&mut self) {
        let len = self.display_artists().len();
        if len == 0 {
            return;
        }
        let n = self
            .list_state
            .selected()
            .map(|i| (i + 1).min(len - 1))
            .unwrap_or(0);
        self.list_state.select(Some(n));
    }
    pub fn move_up(&mut self) {
        let p = self
            .list_state
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.list_state.select(Some(p));
    }
    pub fn page_down(&mut self) {
        let len = self.display_artists().len();
        if len == 0 {
            return;
        }
        let n = self
            .list_state
            .selected()
            .map(|i| (i + 10).min(len - 1))
            .unwrap_or(0);
        self.list_state.select(Some(n));
    }
    pub fn page_up(&mut self) {
        let p = self
            .list_state
            .selected()
            .map(|i| i.saturating_sub(10))
            .unwrap_or(0);
        self.list_state.select(Some(p));
    }
    pub fn jump_top(&mut self) {
        if !self.display_artists().is_empty() {
            self.list_state.select(Some(0));
        }
    }
    pub fn jump_bottom(&mut self) {
        let len = self.display_artists().len();
        if len > 0 {
            self.list_state.select(Some(len - 1));
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool, picker: &mut Picker) {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
        render_search_bar(frame, chunks[0], &self.filter, self.filter_active);

        let display: Vec<Artist> = self.display_artists().to_vec();
        let display_len = display.len();

        let selected = self.list_state.selected().unwrap_or(0);
        let has_photos = self.photo_paths.iter().any(|p| p.is_some());

        if has_photos && IMG_WIDTH + 2 < area.width {
            self.render_with_images(frame, chunks[2], &display, selected, focused, picker);
        } else {
            self.render_text_only(frame, chunks[2], &display, focused);
        }

        let pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(display_len).position(pos);
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            chunks[2],
            &mut self.scrollbar_state,
        );
    }

    /// Render with photo thumbnails: each artist occupies ROW_HEIGHT rows.
    /// Images are loaded lazily on first render for each visible row.
    fn render_with_images(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        display: &[Artist],
        selected: usize,
        focused: bool,
        picker: &mut Picker,
    ) {
        if area.height == 0 {
            return;
        }

        let rows_visible = (area.height / ROW_HEIGHT) as usize;
        let scroll_offset = selected.saturating_sub(rows_visible.saturating_sub(1));
        let end = (scroll_offset + rows_visible + 1).min(display.len());
        let visible = &display[scroll_offset..end];

        // Lazy-load images for the visible window only.
        for (rel_idx, artist) in visible.iter().enumerate() {
            let abs_idx = scroll_offset + rel_idx;
            let orig_idx = self
                .artists
                .iter()
                .position(|a| a.id == artist.id)
                .unwrap_or(abs_idx);
            if orig_idx < self.image_states.len() && self.image_states[orig_idx].is_none() {
                let proto = self.photo_paths.get(orig_idx)
                    .and_then(|p| p.as_ref())
                    .and_then(|p| image::open(p).ok())
                    .map(|img| picker.new_resize_protocol(img));
                self.image_states[orig_idx] = Some(proto);
            }
        }

        let mut y = area.y;
        for (rel_idx, artist) in visible.iter().enumerate() {
            let abs_idx = scroll_offset + rel_idx;
            let is_selected = abs_idx == selected;
            if y >= area.y + area.height {
                break;
            }
            let row_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height: ROW_HEIGHT.min(area.y + area.height - y),
            };
            if row_area.height == 0 {
                break;
            }

            if is_selected && focused {
                frame.render_widget(
                    Block::default().style(Style::default().bg(Color::Rgb(40, 42, 54))),
                    row_area,
                );
            }

            let cols = Layout::horizontal([Constraint::Length(IMG_WIDTH), Constraint::Min(0)])
                .split(row_area);

            let orig_idx = self
                .artists
                .iter()
                .position(|a| a.id == artist.id)
                .unwrap_or(abs_idx);
            if let Some(Some(Some(proto))) = self.image_states.get_mut(orig_idx) {
                frame.render_stateful_widget(StatefulImage::new(), cols[0], proto);
            } else {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        " ♪ ",
                        Style::default().fg(Color::DarkGray),
                    ))),
                    cols[0],
                );
            }

            let name_w = (cols[1].width as usize).saturating_sub(2);
            let name_style = if is_selected && focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let prefix = if is_selected && focused { "> " } else { "  " };
            let name_area = Rect {
                x: cols[1].x,
                y: cols[1].y + ROW_HEIGHT / 2,
                width: cols[1].width,
                height: 1,
            };
            if name_area.y < area.y + area.height {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("{}{}", prefix, truncate(&artist.name, name_w)),
                        name_style,
                    ))),
                    name_area,
                );
            }

            y += ROW_HEIGHT;
        }
    }

    /// Fallback: render as a plain text list (no images available).
    fn render_text_only(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        display: &[Artist],
        focused: bool,
    ) {
        let items: Vec<ListItem> = display
            .iter()
            .map(|a| ListItem::new(Line::from(Span::raw(truncate(&a.name, 60)))))
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
        frame.render_stateful_widget(list, area, &mut self.list_state);
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
