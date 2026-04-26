use crate::utils::truncate;
use image::DynamicImage;
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
use std::sync::{Arc, Mutex};
use tornade_core::{
    models::{Album, Artist},
    services::LibraryService,
};

pub struct ArtistDetailState {
    pub artist: Artist,
    pub albums: Vec<Album>,
    pub list_state: ListState,
    image_state: Option<StatefulProtocol>,
    pending_image: Arc<Mutex<Option<DynamicImage>>>,
    image_loading: bool,
    scrollbar_state: ScrollbarState,
}


impl ArtistDetailState {
    pub fn new(
        artist: Artist,
        library: &LibraryService,
        _picker: &mut Picker,
        photo_dir: &Path,
    ) -> Self {
        let albums = library.get_artist_albums(artist.id).unwrap_or_default();
        let mut list_state = ListState::default();
        if !albums.is_empty() {
            list_state.select(Some(0));
        }

        let pending_image: Arc<Mutex<Option<DynamicImage>>> = Arc::new(Mutex::new(None));
        let photo_path = photo_dir.join(format!("{}.jpg", artist.id));
        let image_loading = photo_path.exists();

        if image_loading {
            let pending = Arc::clone(&pending_image);
            std::thread::spawn(move || {
                if let Ok(img) = image::open(&photo_path) {
                    if let Ok(mut guard) = pending.lock() {
                        *guard = Some(img);
                    }
                }
            });
        }

        Self {
            artist,
            albums,
            list_state,
            image_state: None,
            pending_image,
            image_loading,
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool, picker: &mut Picker) -> bool {
        // Drain decoded image from background thread
        if self.image_loading && self.image_state.is_none() {
            if let Ok(mut guard) = self.pending_image.try_lock() {
                if let Some(img) = guard.take() {
                    let rgba = image::DynamicImage::ImageRgba8(img.to_rgba8());
                    self.image_state = Some(picker.new_resize_protocol(rgba));
                    self.image_loading = false;
                }
            }
        }

        let has_pending = self.image_loading;

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

        has_pending
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
