use std::collections::HashMap;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use ratatui_image::{Resize, StatefulImage, picker::Picker, protocol::StatefulProtocol};
use tornade_core::{models::Album, services::LibraryService};
use crate::utils::truncate;

const CELL_W: u16 = 22;
// 12 rows image (no border) + 3 rows text
const CELL_H: u16 = 15;
const IMG_H: u16 = 12;

pub struct AlbumsState {
    pub albums: Vec<Album>,
    pub filter: String,
    pub filter_active: bool,
    pub selected: usize,
    pub scroll_row: usize,
    pub cols: usize,
    pub last_grid_area: Rect,
    image_cache: HashMap<i64, StatefulProtocol>,
    scrollbar_state: ScrollbarState,
}

impl Default for AlbumsState {
    fn default() -> Self {
        Self {
            albums: Vec::new(),
            filter: String::new(),
            filter_active: false,
            selected: 0,
            scroll_row: 0,
            cols: 4,
            last_grid_area: Rect::default(),
            image_cache: HashMap::new(),
            scrollbar_state: ScrollbarState::default(),
        }
    }
}

impl AlbumsState {
    pub fn load(&mut self, library: &LibraryService) {
        self.albums = library.list_albums(None, None, None, Some(5000), Some(0)).unwrap_or_default();
    }

    pub fn filtered_albums(&self) -> Vec<&Album> {
        if self.filter.is_empty() {
            self.albums.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.albums.iter().filter(|a| {
                a.title.to_lowercase().contains(&q) || a.artist_name.to_lowercase().contains(&q)
            }).collect()
        }
    }

    pub fn selected_album(&self) -> Option<&Album> {
        self.filtered_albums().get(self.selected).copied()
    }

    /// Returns the flat album index at terminal coordinates (x, y), if any.
    pub fn album_at_pos(&self, x: u16, y: u16) -> Option<usize> {
        let a = self.last_grid_area;
        if a.width == 0 || x < a.x || y < a.y || x >= a.x + a.width || y >= a.y + a.height {
            return None;
        }
        let col = ((x - a.x) / CELL_W) as usize;
        let row_in_view = ((y - a.y) / CELL_H) as usize;
        if col >= self.cols { return None; }
        let row = self.scroll_row + row_in_view;
        let idx = row * self.cols + col;
        let total = self.filtered_albums().len();
        if idx >= total { None } else { Some(idx) }
    }

    pub fn move_right(&mut self) {
        let len = self.filtered_albums().len();
        if len > 0 { self.selected = (self.selected + 1).min(len - 1); }
    }

    pub fn move_left(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let len = self.filtered_albums().len();
        if len == 0 { return; }
        self.selected = (self.selected + self.cols).min(len - 1);
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.cols);
    }

    pub fn page_down(&mut self) {
        let len = self.filtered_albums().len();
        if len == 0 { return; }
        self.selected = (self.selected + self.cols * 3).min(len - 1);
    }

    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.cols * 3);
    }

    pub fn jump_top(&mut self) { self.selected = 0; self.scroll_row = 0; }

    pub fn jump_bottom(&mut self) {
        let len = self.filtered_albums().len();
        if len > 0 { self.selected = len - 1; }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool, picker: &mut Picker) {
        // Reserve 1 col on the right for the scrollbar
        let inner_w = area.width.saturating_sub(1);
        let grid_area = Rect { width: inner_w, ..area };
        self.last_grid_area = grid_area;

        self.cols = ((grid_area.width / CELL_W) as usize).max(1);
        let rows_visible = ((grid_area.height / CELL_H) as usize).max(1);

        let total = self.filtered_albums().len();
        if total > 0 && self.selected >= total { self.selected = total - 1; }

        let sel_row = if self.cols > 0 { self.selected / self.cols } else { 0 };
        if sel_row < self.scroll_row { self.scroll_row = sel_row; }
        else if sel_row >= self.scroll_row + rows_visible {
            self.scroll_row = sel_row + 1 - rows_visible;
        }

        let total_rows = (total + self.cols - 1) / self.cols;
        let start = self.scroll_row * self.cols;
        let end = ((self.scroll_row + rows_visible) * self.cols).min(total);

        // Collect visible album data (owned) to release the immutable borrow
        let visible: Vec<(usize, i64, String, String, Option<u16>, Option<std::path::PathBuf>, Option<std::path::PathBuf>)> = {
            let filtered = self.filtered_albums();
            filtered[start..end].iter().enumerate().map(|(i, a)| (
                start + i, a.id, a.title.clone(), a.artist_name.clone(), a.year,
                a.online_artwork_path.clone(), a.artwork_path.clone(),
            )).collect()
        };

        // Lazy-load images for visible albums
        for (_, id, _, _, _, online, local) in &visible {
            if !self.image_cache.contains_key(id) {
                let path = online.as_ref().or(local.as_ref());
                if let Some(img) = path.and_then(|p| image::open(p).ok()) {
                    self.image_cache.insert(*id, picker.new_resize_protocol(img));
                }
            }
        }

        // Render cells
        for (flat_idx, id, title, artist, year, _, _) in &visible {
            let row = flat_idx / self.cols;
            let col = flat_idx % self.cols;
            if row >= self.scroll_row + rows_visible || row >= total_rows { continue; }

            let x = grid_area.x + (col as u16) * CELL_W;
            let y = grid_area.y + ((row - self.scroll_row) as u16) * CELL_H;
            if x >= grid_area.x + grid_area.width || y >= grid_area.y + grid_area.height { continue; }

            let w = CELL_W.min(grid_area.x + grid_area.width - x);
            let h = CELL_H.min(grid_area.y + grid_area.height - y);
            let cell_rect = Rect { x, y, width: w, height: h };

            let is_sel = *flat_idx == self.selected;
            let protocol = self.image_cache.get_mut(id);
            render_cell(frame, cell_rect, title, artist, *year, is_sel, focused, protocol);
        }

        // Scrollbar (1 col strip on the right)
        self.scrollbar_state = ScrollbarState::new(total_rows).position(self.scroll_row);
        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            width: 1,
            ..area
        };
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            scrollbar_area,
            &mut self.scrollbar_state,
        );
    }
}

fn render_cell(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    artist: &str,
    year: Option<u16>,
    is_selected: bool,
    focused: bool,
    protocol: Option<&mut StatefulProtocol>,
) {
    let img_rect = Rect { height: IMG_H.min(area.height), ..area };
    let text_y = area.y + img_rect.height;
    let text_h = area.height.saturating_sub(img_rect.height);
    let text_rect = Rect { y: text_y, height: text_h, ..area };

    // Image - rendered directly, no border/block
    if let Some(proto) = protocol {
        frame.render_stateful_widget(StatefulImage::new().resize(Resize::Crop(None)), img_rect, proto);
    } else {
        // Placeholder block (no border, just dark bg)
        frame.render_widget(Block::default().style(Style::default().bg(Color::DarkGray)), img_rect);
    }

    // Selection indicator: cyan top border on image when selected + focused
    if is_selected && focused {
        // Draw a 1-row highlight above the image (if room)
        // Instead, highlight the text title
    }

    if text_rect.height == 0 { return; }

    let title_style = if is_selected && focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let max_w = area.width as usize;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(truncate(title, max_w), title_style)),
            Line::from(Span::styled(truncate(artist, max_w), Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled(
                year.map(|y| y.to_string()).unwrap_or_default(),
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        text_rect,
    );
}
