use crate::utils::{format_duration, format_rating, truncate};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use tornade_core::{
    models::{Album, Artist, Track},
    services::LibraryService,
};

pub struct AlbumDetailState {
    pub album: Album,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub skipped_ids: Vec<i64>,
    image_state: Option<StatefulProtocol>,
    artist_albums: Vec<Album>,
    similar_artists: Vec<Artist>,
}

impl AlbumDetailState {
    pub fn new(album: Album, library: &LibraryService, picker: &mut Picker) -> Self {
        let tracks = library.get_album_tracks(album.id).unwrap_or_default();
        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }

        let image_state = album
            .online_artwork_path
            .as_ref()
            .or(album.artwork_path.as_ref())
            .and_then(|p| image::open(p).ok())
            .map(|img| picker.new_resize_protocol(img));

        let artist_albums = library
            .get_artist_albums(album.artist_id)
            .unwrap_or_default();
        let similar_artists = library
            .get_similar_artists(album.artist_id)
            .unwrap_or_default();

        Self {
            album,
            tracks,
            list_state,
            skipped_ids: Vec::new(),
            image_state,
            artist_albums,
            similar_artists,
        }
    }

    pub fn selected_track(&self) -> Option<&Track> {
        self.list_state.selected().and_then(|i| self.tracks.get(i))
    }

    pub fn visible_track_ids(&self) -> Vec<i64> {
        self.tracks.iter().map(|t| t.id).collect()
    }

    pub fn move_down(&mut self) {
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

    pub fn move_up(&mut self) {
        let p = self
            .list_state
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.list_state.select(Some(p));
    }

    pub fn jump_top(&mut self) {
        if !self.tracks.is_empty() {
            self.list_state.select(Some(0));
        }
    }
    pub fn jump_bottom(&mut self) {
        if !self.tracks.is_empty() {
            self.list_state.select(Some(self.tracks.len() - 1));
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        // Layout: tracks + sections (left ~68%) | artwork + About (right ~32%)
        let right_w = (area.width * 32 / 100).clamp(24, 40);
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(right_w)])
            .split(area);

        self.render_right(frame, h[0], focused);
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
            frame.render_stateful_widget(StatefulImage::new(), v[0], proto);
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

        frame.render_widget(Paragraph::new(lines), v[1]);
    }

    fn render_right(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
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

        // Header: title + artist
        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                truncate(&header_text, (area.width as usize).saturating_sub(4)),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
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
        self.render_tracks(frame, v[1], focused);

        // Albums by the artist
        if artist_albums_h > 0 {
            self.render_artist_albums(frame, v[2]);
        }

        // Artists same genre
        if similar_h > 0 {
            self.render_similar_artists(frame, v[3]);
        }
    }

    fn render_tracks(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
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
                .map(|t| self.make_track_item(t, max_title))
                .collect();
            let right_items: Vec<ListItem> = self.tracks[mid.min(self.tracks.len())..]
                .iter()
                .map(|t| self.make_track_item(t, max_title))
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
                .map(|t| self.make_track_item(t, max_title))
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

    fn make_track_item(&self, t: &Track, max_title: usize) -> ListItem<'static> {
        let is_skipped = self.skipped_ids.contains(&t.id);
        let num = t
            .track_number
            .map(|n| format!("{:>2}. ", n))
            .unwrap_or_else(|| "    ".to_string());
        let dur = format_duration(t.duration.as_secs());
        let rating = format_rating(t.rating.0);
        let line = Line::from(vec![
            Span::styled(num, Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{:<width$} ",
                truncate(&t.title, max_title),
                width = max_title
            )),
            Span::styled(format!("{:>5} ", dur), Style::default().fg(Color::DarkGray)),
            Span::styled(rating, Style::default().fg(Color::Yellow)),
        ]);
        let style = if is_skipped {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        ListItem::new(line).style(style)
    }

    fn render_artist_albums(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![
            Line::from(Span::styled(
                "Albums by the artist",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        let w = (area.width as usize).saturating_sub(8);
        for album in self.artist_albums.iter().take(4) {
            let year = album.year.map(|y| format!(" ({})", y)).unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    truncate(&format!("{}{}", album.title, year), w),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }
        frame.render_widget(Paragraph::new(lines).block(Block::default()), area);
    }

    fn render_similar_artists(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![
            Line::from(Span::styled(
                "Artists same Genre",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        let w = (area.width as usize).saturating_sub(4);
        for artist in self.similar_artists.iter().take(4) {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(truncate(&artist.name, w), Style::default().fg(Color::Gray)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines).block(Block::default()), area);
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
        n
    }
}
