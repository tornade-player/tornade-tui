use crate::utils::truncate;
use image::{DynamicImage, Rgba};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::path::Path;
use tornade_core::{
    models::{Album, Artist},
    services::LibraryService,
};

pub struct ArtistDetailState {
    pub artist: Artist,
    pub albums: Vec<Album>,
    pub list_state: ListState,
    image_state: Option<StatefulProtocol>,
    scrollbar_state: ScrollbarState,
}

/// Apply a circular mask to an image (pixels outside the circle become transparent).
fn apply_circle_mask(img: DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let mut rgba = img.into_rgba8();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let r = cx.min(cy);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if (dx * dx + dy * dy).sqrt() > r {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
    DynamicImage::ImageRgba8(rgba)
}

impl ArtistDetailState {
    pub fn new(
        artist: Artist,
        library: &LibraryService,
        picker: &mut Picker,
        photo_dir: &Path,
    ) -> Self {
        let albums = library.get_artist_albums(artist.id).unwrap_or_default();
        let mut list_state = ListState::default();
        if !albums.is_empty() {
            list_state.select(Some(0));
        }

        let photo_path = photo_dir.join(format!("{}.jpg", artist.id));
        let image_state = if photo_path.exists() {
            image::open(&photo_path)
                .ok()
                .map(|img| apply_circle_mask(img))
                .map(|img| picker.new_resize_protocol(img))
        } else {
            None
        };

        Self {
            artist,
            albums,
            list_state,
            image_state,
            scrollbar_state: ScrollbarState::default(),
        }
    }

    pub fn selected_album(&self) -> Option<&Album> {
        self.list_state.selected().and_then(|i| self.albums.get(i))
    }

    pub fn move_down(&mut self) {
        let len = self.albums.len();
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
    pub fn jump_top(&mut self) {
        if !self.albums.is_empty() {
            self.list_state.select(Some(0));
        }
    }
    pub fn jump_bottom(&mut self) {
        if !self.albums.is_empty() {
            self.list_state.select(Some(self.albums.len() - 1));
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let header_height = if self.image_state.is_some() { 10 } else { 4 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Min(0)])
            .split(area);

        self.render_header(frame, chunks[0]);

        let items: Vec<ListItem> = self
            .albums
            .iter()
            .map(|a| {
                let year = a.year.map(|y| format!("  {}", y)).unwrap_or_default();
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<45} ", truncate(&a.title, 44))),
                    Span::styled(year, Style::default().fg(Color::DarkGray)),
                ]))
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
        frame.render_stateful_widget(list, chunks[1], &mut self.list_state);

        let pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(self.albums.len()).position(pos);
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            chunks[1],
            &mut self.scrollbar_state,
        );
    }

    fn render_header(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(ref mut protocol) = self.image_state {
            let h_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(20), Constraint::Min(0)])
                .split(area);

            // Circle image - rendered directly, no border
            frame.render_stateful_widget(
                StatefulImage::new().resize(ratatui_image::Resize::Fit(None)),
                h_chunks[0],
                protocol,
            );

            let meta = Paragraph::new(vec![
                Line::from(Span::styled(
                    truncate(&self.artist.name, 50),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    format!("{} albums", self.albums.len()),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(Block::default());
            frame.render_widget(meta, h_chunks[1]);
        } else {
            let header = Paragraph::new(vec![
                Line::from(Span::styled(
                    truncate(&self.artist.name, 60),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    format!("{} albums", self.albums.len()),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(Block::default());
            frame.render_widget(header, area);
        }
    }
}
