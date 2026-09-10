use crate::utils::{format_duration, format_rating, truncate};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::path::Path;
use tornade_core::{
    models::{Album, Artist, Track},
    services::LibraryService,
};

/// Which part of the album-detail view currently has keyboard focus.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailSection {
    #[default]
    Tracks,
    ArtistAlbums,
    SimilarArtists,
}

pub struct AlbumDetailState {
    pub album: Album,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub skipped_ids: Vec<i64>,
    image_state: Option<StatefulProtocol>,
    artist_albums: Vec<Album>,
    artist_albums_state: ListState,
    /// Which section currently has keyboard focus.
    pub section: DetailSection,
    pub artist_albums_area: Option<Rect>,
    similar_artists: Vec<Artist>,
    similar_artists_state: ListState,
    pub similar_artists_area: Option<Rect>,
}

impl AlbumDetailState {
    pub fn new(
        album: Album,
        library: &LibraryService,
        picker: &mut Picker,
        tui_album_dir: &Path,
    ) -> Self {
        let tracks = library.get_album_tracks(album.id).unwrap_or_default();
        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }

        let image_state = album
            .online_artwork_path
            .as_ref()
            .map(|p| crate::tui_artwork::resolve_artwork_path(p, tui_album_dir))
            .or_else(|| album.artwork_path.as_ref().cloned())
            .and_then(|p| image::open(&p).ok())
            .map(|img| image::DynamicImage::ImageRgba8(img.to_rgba8()))
            .map(|img| picker.new_resize_protocol(img));

        let artist_albums = library
            .get_artist_albums(album.artist_id)
            .unwrap_or_default();
        let mut artist_albums_state = ListState::default();
        if !artist_albums.is_empty() {
            artist_albums_state.select(Some(0));
        }
        let similar_artists = library
            .get_similar_artists(album.artist_id)
            .unwrap_or_default();
        let mut similar_artists_state = ListState::default();
        if !similar_artists.is_empty() {
            similar_artists_state.select(Some(0));
        }

        Self {
            album,
            tracks,
            list_state,
            skipped_ids: Vec::new(),
            image_state,
            artist_albums,
            artist_albums_state,
            section: DetailSection::Tracks,
            artist_albums_area: None,
            similar_artists,
            similar_artists_state,
            similar_artists_area: None,
        }
    }

    /// Reload the album's tracks from the library (preserving selection).
    /// Used to reflect tag edits without a rescan.
    pub fn reload(&mut self, library: &LibraryService) {
        let sel = self.list_state.selected();
        self.tracks = library.get_album_tracks(self.album.id).unwrap_or_default();
        if self.tracks.is_empty() {
            self.list_state.select(None);
        } else {
            let idx = sel.unwrap_or(0).min(self.tracks.len() - 1);
            self.list_state.select(Some(idx));
        }
    }

    pub fn selected_track(&self) -> Option<&Track> {
        self.list_state.selected().and_then(|i| self.tracks.get(i))
    }

    pub fn selected_artist_album(&self) -> Option<&Album> {
        self.artist_albums_state
            .selected()
            .and_then(|i| self.artist_albums.get(i))
    }

    pub fn selected_similar_artist(&self) -> Option<&Artist> {
        self.similar_artists_state
            .selected()
            .and_then(|i| self.similar_artists.get(i))
    }

    pub fn has_artist_albums(&self) -> bool {
        !self.artist_albums.is_empty()
    }

    pub fn has_similar_artists(&self) -> bool {
        !self.similar_artists.is_empty()
    }

    pub fn artist_albums_len(&self) -> usize {
        self.artist_albums.len()
    }

    pub fn similar_artists_len(&self) -> usize {
        self.similar_artists.len()
    }

    pub fn select_artist_album(&mut self, idx: usize) {
        self.artist_albums_state.select(Some(idx));
    }

    pub fn select_similar_artist(&mut self, idx: usize) {
        self.similar_artists_state.select(Some(idx));
    }

    /// Cycle keyboard focus forward through Tracks -> Albums by the artist ->
    /// Artists same genre -> Tracks, skipping empty sections.
    pub fn cycle_section(&mut self) {
        let has_albums = self.has_artist_albums();
        let has_similar = self.has_similar_artists();
        self.section = match self.section {
            DetailSection::Tracks if has_albums => DetailSection::ArtistAlbums,
            DetailSection::Tracks if has_similar => DetailSection::SimilarArtists,
            DetailSection::Tracks => DetailSection::Tracks,
            DetailSection::ArtistAlbums if has_similar => DetailSection::SimilarArtists,
            DetailSection::ArtistAlbums => DetailSection::Tracks,
            DetailSection::SimilarArtists => DetailSection::Tracks,
        };
    }

    pub fn visible_track_ids(&self) -> Vec<i64> {
        self.tracks.iter().map(|t| t.id).collect()
    }

    pub fn move_down(&mut self) {
        match self.section {
            DetailSection::ArtistAlbums => {
                let len = self.artist_albums.len();
                if len == 0 {
                    return;
                }
                let n = self
                    .artist_albums_state
                    .selected()
                    .map(|i| (i + 1).min(len - 1))
                    .unwrap_or(0);
                self.artist_albums_state.select(Some(n));
            }
            DetailSection::SimilarArtists => {
                let len = self.similar_artists.len();
                if len == 0 {
                    return;
                }
                let n = self
                    .similar_artists_state
                    .selected()
                    .map(|i| (i + 1).min(len - 1))
                    .unwrap_or(0);
                self.similar_artists_state.select(Some(n));
            }
            DetailSection::Tracks => {
                let len = self.tracks.len();
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
        }
    }

    pub fn move_up(&mut self) {
        match self.section {
            DetailSection::ArtistAlbums => {
                let p = self
                    .artist_albums_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                self.artist_albums_state.select(Some(p));
            }
            DetailSection::SimilarArtists => {
                let p = self
                    .similar_artists_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                self.similar_artists_state.select(Some(p));
            }
            DetailSection::Tracks => {
                let p = self
                    .list_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                self.list_state.select(Some(p));
            }
        }
    }

    pub fn jump_top(&mut self) {
        match self.section {
            DetailSection::ArtistAlbums => {
                if !self.artist_albums.is_empty() {
                    self.artist_albums_state.select(Some(0));
                }
            }
            DetailSection::SimilarArtists => {
                if !self.similar_artists.is_empty() {
                    self.similar_artists_state.select(Some(0));
                }
            }
            DetailSection::Tracks => {
                if !self.tracks.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
        }
    }
    pub fn jump_bottom(&mut self) {
        match self.section {
            DetailSection::ArtistAlbums => {
                if !self.artist_albums.is_empty() {
                    self.artist_albums_state
                        .select(Some(self.artist_albums.len() - 1));
                }
            }
            DetailSection::SimilarArtists => {
                if !self.similar_artists.is_empty() {
                    self.similar_artists_state
                        .select(Some(self.similar_artists.len() - 1));
                }
            }
            DetailSection::Tracks => {
                if !self.tracks.is_empty() {
                    self.list_state.select(Some(self.tracks.len() - 1));
                }
            }
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        selection: &crate::app::Selection,
    ) {
        // Layout: tracks + sections (left ~68%) | artwork + About (right ~32%)
        let right_w = (area.width * 32 / 100).clamp(24, 40);
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(right_w)])
            .split(area);

        self.render_right(frame, h[0], focused, selection);
        self.render_left(frame, h[1]);
    }

    fn render_left(&mut self, frame: &mut Frame, area: Rect) {
        // Apply padding around the whole panel
        let padded = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        // Image: square capped so the About section always has room to show
        let about_rows = self.about_row_count() as u16 + 2;
        let img_h = padded.width.min(padded.height.saturating_sub(about_rows));

        let v = Layout::vertical([
            Constraint::Length(img_h),
            Constraint::Length(about_rows),
            Constraint::Min(0), // empty space at bottom
        ])
        .split(padded);

        // Image
        if img_h > 0
            && let Some(ref mut proto) = self.image_state
        {
            frame.render_stateful_widget(
                StatefulImage::new().resize(ratatui_image::Resize::Fit(Some(
                    image::imageops::FilterType::Triangle,
                ))),
                v[0],
                proto,
            );
        }

        // About section directly below artwork, no gap
        let mut lines = vec![
            Line::from(Span::styled(
                "About",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        self.push_info_row(&mut lines, "Artist", &self.album.artist_name.clone());
        if let Some(y) = self.album.year {
            self.push_info_row(&mut lines, "Year", &y.to_string());
        }
        self.push_info_row(&mut lines, "Tracks", &self.tracks.len().to_string());
        if let Some(ref t) = self.album.album_type.clone() {
            self.push_info_row(&mut lines, "Type", t);
        }
        if let Some(ref s) = self.album.release_status.clone() {
            self.push_info_row(&mut lines, "Status", s);
        }
        if let Some(ref l) = self.album.label.clone() {
            self.push_info_row(&mut lines, "Label", l);
        }
        if let Some(ref c) = self.album.country.clone() {
            self.push_info_row(&mut lines, "Country", c);
        }
        let feats = self.featuring_artists();
        if !feats.is_empty() {
            self.push_info_row(&mut lines, "Featuring", &feats.join(", "));
        }

        frame.render_widget(Paragraph::new(lines), v[1]);
    }

    fn render_right(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        selection: &crate::app::Selection,
    ) {
        let year = self
            .album
            .year
            .map(|y| format!("  ·  {}", y))
            .unwrap_or_default();
        let header_text = format!("{}{}", self.album.title, year);

        // Albums by artist section height
        let artist_albums_h = if self.artist_albums.is_empty() {
            0
        } else {
            3u16 + (self.artist_albums.len() as u16).min(4)
        };
        // Similar artists section height
        let similar_h = if self.similar_artists.is_empty() {
            0
        } else {
            3u16 + (self.similar_artists.len() as u16).min(4)
        };

        let v = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // header
                Constraint::Min(0),    // tracks (2 cols)
                Constraint::Length(artist_albums_h),
                Constraint::Length(similar_h),
            ])
            .split(area);

        // Header: title + artist (+ live selection count when in selection mode)
        let mut title_spans = vec![Span::styled(
            truncate(&header_text, (area.width as usize).saturating_sub(4)),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )];
        if let Some(count) = crate::widgets::selection::count_span(selection) {
            title_spans.push(count);
        }
        let header = Paragraph::new(vec![
            Line::from(title_spans),
            Line::from(Span::styled(
                truncate(
                    &self.album.artist_name,
                    (area.width as usize).saturating_sub(4),
                ),
                Style::default().fg(Color::Gray),
            )),
        ])
        .block(Block::default());
        frame.render_widget(header, v[0]);

        // Tracks: 2 columns if wide enough, 1 column otherwise
        self.render_tracks(
            frame,
            v[1],
            focused && self.section == DetailSection::Tracks,
            selection,
        );

        // Albums by the artist
        if artist_albums_h > 0 {
            self.render_artist_albums(
                frame,
                v[2],
                focused && self.section == DetailSection::ArtistAlbums,
            );
        }

        // Artists same genre
        if similar_h > 0 {
            self.render_similar_artists(
                frame,
                v[3],
                focused && self.section == DetailSection::SimilarArtists,
            );
        }
    }

    fn render_tracks(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        selection: &crate::app::Selection,
    ) {
        // Minimum width per column: num(4) + title(20) + dur(6) + rating(5) + sym(1) = ~36
        const MIN_COL_WIDTH: u16 = 36;
        let two_columns = area.width >= MIN_COL_WIDTH * 2;

        let (hl_style, hl_sym) = if focused {
            (
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
                ">",
            )
        } else {
            (Style::default().fg(Color::DarkGray), " ")
        };

        if two_columns {
            let mid = self.tracks.len().div_ceil(2);
            let col_w = area.width / 2;
            let max_title = ((col_w as usize).saturating_sub(14)).max(10);

            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(col_w), Constraint::Min(0)])
                .split(area);

            let left_items: Vec<ListItem> = self.tracks[..mid.min(self.tracks.len())]
                .iter()
                .map(|t| self.make_track_item(t, max_title, selection))
                .collect();
            let right_items: Vec<ListItem> = self.tracks[mid.min(self.tracks.len())..]
                .iter()
                .map(|t| self.make_track_item(t, max_title, selection))
                .collect();

            let selected = self.list_state.selected().unwrap_or(0);

            let mut left_state = ListState::default();
            if selected < mid {
                left_state.select(Some(selected));
            }
            frame.render_stateful_widget(
                List::new(left_items)
                    .highlight_style(hl_style)
                    .highlight_symbol(hl_sym),
                cols[0],
                &mut left_state,
            );

            let mut right_state = ListState::default();
            if selected >= mid {
                right_state.select(Some(selected - mid));
            }
            frame.render_stateful_widget(
                List::new(right_items)
                    .highlight_style(hl_style)
                    .highlight_symbol(hl_sym),
                cols[1],
                &mut right_state,
            );
        } else {
            let max_title = ((area.width as usize).saturating_sub(14)).max(10);
            let items: Vec<ListItem> = self
                .tracks
                .iter()
                .map(|t| self.make_track_item(t, max_title, selection))
                .collect();
            frame.render_stateful_widget(
                List::new(items)
                    .highlight_style(hl_style)
                    .highlight_symbol(hl_sym),
                area,
                &mut self.list_state,
            );
        }
    }

    fn make_track_item(
        &self,
        t: &Track,
        max_title: usize,
        selection: &crate::app::Selection,
    ) -> ListItem<'static> {
        let is_skipped = self.skipped_ids.contains(&t.id);
        let num = t
            .track_number
            .map(|n| format!("{:>2}. ", n))
            .unwrap_or_else(|| "    ".to_string());
        let dur = format_duration(t.duration.as_secs());
        let rating = format_rating(t.rating.0);
        let mut spans = Vec::new();
        if let Some(marker) = crate::widgets::selection::marker_span(selection, t.id) {
            spans.push(marker);
        }
        spans.extend([
            Span::styled(num, Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{:<width$} ",
                truncate(&t.title, max_title),
                width = max_title
            )),
            Span::styled(format!("{:>5} ", dur), Style::default().fg(Color::DarkGray)),
            Span::styled(rating, Style::default().fg(Color::Yellow)),
        ]);
        let line = Line::from(spans);
        let style = if is_skipped {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        ListItem::new(line).style(style)
    }

    fn render_artist_albums(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let title = Paragraph::new(Line::from(Span::styled(
            "Albums by the artist",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        let chunks = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).split(area);
        frame.render_widget(title, chunks[0]);

        let w = (chunks[1].width as usize).saturating_sub(2);
        let items: Vec<ListItem> = self
            .artist_albums
            .iter()
            .map(|album| {
                let year = album.year.map(|y| format!(" ({})", y)).unwrap_or_default();
                ListItem::new(Line::from(Span::styled(
                    truncate(&format!("{}{}", album.title, year), w),
                    Style::default().fg(Color::Gray),
                )))
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
        frame.render_stateful_widget(list, chunks[1], &mut self.artist_albums_state);
        self.artist_albums_area = Some(chunks[1]);
    }

    fn render_similar_artists(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let title = Paragraph::new(Line::from(Span::styled(
            "Artists same Genre",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        let chunks = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).split(area);
        frame.render_widget(title, chunks[0]);

        let w = (chunks[1].width as usize).saturating_sub(2);
        let items: Vec<ListItem> = self
            .similar_artists
            .iter()
            .map(|artist| {
                ListItem::new(Line::from(Span::styled(
                    truncate(&artist.name, w),
                    Style::default().fg(Color::Gray),
                )))
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
        frame.render_stateful_widget(list, chunks[1], &mut self.similar_artists_state);
        self.similar_artists_area = Some(chunks[1]);
    }

    fn push_info_row<'a>(&self, lines: &mut Vec<Line<'a>>, label: &'static str, value: &str) {
        let w = 28usize;
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<8}", label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                truncate(value, w.saturating_sub(8)),
                Style::default().fg(Color::Gray),
            ),
        ]));
    }

    fn about_row_count(&self) -> usize {
        let mut n = 4usize; // header + blank + artist + tracks
        if self.album.year.is_some() {
            n += 1;
        }
        if self.album.album_type.is_some() {
            n += 1;
        }
        if self.album.release_status.is_some() {
            n += 1;
        }
        if self.album.label.is_some() {
            n += 1;
        }
        if self.album.country.is_some() {
            n += 1;
        }
        if !self.featuring_artists().is_empty() {
            n += 1;
        }
        n
    }

    /// Guest artists appearing on the album's tracks other than the album artist.
    fn featuring_artists(&self) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for t in &self.tracks {
            for name in &t.artist_names {
                if name != &self.album.artist_name {
                    seen.insert(name.clone());
                }
            }
        }
        seen.into_iter().collect()
    }
}
