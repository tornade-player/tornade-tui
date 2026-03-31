use std::collections::HashMap;
use image::Rgba;
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

// Image height in terminal rows (fixed); width is computed from font metrics to make it square
const IMG_H: u16 = 8;
// Text rows below the image
const TEXT_H: u16 = 3;
// Padding rows between image bottom and text
const TEXT_PADDING: u16 = 1;
// Gap between cells (cols/rows)
const GAP_W: u16 = 3;
const GAP_H: u16 = 2;
// Left/right padding inside the grid area
const GRID_PAD: u16 = 1;
// Max images to load (resize+encode) per render frame to avoid CPU spikes
const MAX_LOADS_PER_FRAME: usize = 3;
// Corner radius as fraction of the shorter image dimension
const CORNER_RADIUS_FRAC: f32 = 0.12;

pub struct AlbumsState {
    pub albums: Vec<Album>,
    pub filter: String,
    pub filter_active: bool,
    pub selected: usize,
    pub scroll_row: usize,
    pub cols: usize,
    pub last_grid_area: Rect,
    // computed each render, stored for click detection between renders
    img_cols: u16,
    cell_stride_w: u16,
    cell_stride_h: u16,
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
            img_cols: IMG_H, // sensible default before first render
            cell_stride_w: IMG_H + GAP_W,
            cell_stride_h: IMG_H + TEXT_PADDING + TEXT_H + GAP_H,
            image_cache: HashMap::new(),
            scrollbar_state: ScrollbarState::default(),
        }
    }
}

/// Apply rounded corners to an image by zeroing alpha in corner regions.
fn apply_rounded_corners(img: image::DynamicImage) -> image::DynamicImage {
    let (w, h) = (img.width(), img.height());
    let mut rgba = img.into_rgba8();
    let r = (w.min(h) as f32 * CORNER_RADIUS_FRAC).max(1.0);
    let fw = w as f32;
    let fh = h as f32;
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32;
            let fy = y as f32;
            // Distance from each corner's arc center; transparent if outside the arc
            let in_corner =
                (fx < r && fy < r && (fx - r).hypot(fy - r) > r) ||
                (fx > fw - r - 1.0 && fy < r && (fx - (fw - r - 1.0)).hypot(fy - r) > r) ||
                (fx < r && fy > fh - r - 1.0 && (fx - r).hypot(fy - (fh - r - 1.0)) > r) ||
                (fx > fw - r - 1.0 && fy > fh - r - 1.0 &&
                    (fx - (fw - r - 1.0)).hypot(fy - (fh - r - 1.0)) > r);
            if in_corner {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
    image::DynamicImage::ImageRgba8(rgba)
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
        let col = ((x - a.x) / self.cell_stride_w) as usize;
        let row_in_view = ((y - a.y) / self.cell_stride_h) as usize;
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
        // Compute square image width from font metrics
        // font_size() = (cell_width_px, cell_height_px)
        let font = picker.font_size();
        self.img_cols = if font.0 > 0 {
            ((IMG_H as u32 * font.1 as u32) / font.0 as u32) as u16
        } else {
            IMG_H
        };
        self.img_cols = self.img_cols.max(14); // never narrower than 14 cols

        self.cell_stride_w = self.img_cols + GAP_W;
        self.cell_stride_h = IMG_H + TEXT_PADDING + TEXT_H + GAP_H;

        // Left/right padding + 1 col on the right for the scrollbar
        let grid_area = Rect {
            x: area.x + GRID_PAD,
            width: area.width.saturating_sub(GRID_PAD * 2 + 1),
            ..area
        };
        self.last_grid_area = grid_area;

        self.cols = ((grid_area.width / self.cell_stride_w) as usize).max(1);
        let rows_visible = ((grid_area.height / self.cell_stride_h) as usize).max(1);

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
        type AlbumTuple = (usize, i64, String, String, Option<u16>,
                           Option<std::path::PathBuf>, Option<std::path::PathBuf>);
        let visible: Vec<AlbumTuple> = {
            let filtered = self.filtered_albums();
            filtered[start..end].iter().enumerate().map(|(i, a)| (
                start + i, a.id, a.title.clone(), a.artist_name.clone(), a.year,
                a.online_artwork_path.clone(), a.artwork_path.clone(),
            )).collect()
        };

        // Lazy-load images for visible albums.
        // Pre-resize to exact pixel size so rounded corners land exactly at image edges.
        // Use Triangle (bilinear) filter: ~10x faster than Lanczos3, adequate for thumbnails.
        // Limit to MAX_LOADS_PER_FRAME per render to avoid CPU spikes on first paint.
        let (fw, fh) = picker.font_size();
        let target_px_w = (self.img_cols as u32) * (fw as u32);
        let target_px_h = (IMG_H as u32) * (fh as u32);
        let mut loads_this_frame = 0;
        for (_, id, _, _, _, online, local) in &visible {
            if loads_this_frame >= MAX_LOADS_PER_FRAME { break; }
            if !self.image_cache.contains_key(id) {
                let path = online.as_ref().or(local.as_ref());
                if let Some(img) = path.and_then(|p| image::open(p).ok()) {
                    let img = img.resize_to_fill(
                        target_px_w.max(1), target_px_h.max(1),
                        image::imageops::FilterType::Triangle,
                    );
                    let img = apply_rounded_corners(img);
                    self.image_cache.insert(*id, picker.new_resize_protocol(img));
                    loads_this_frame += 1;
                }
            }
        }

        let img_cols = self.img_cols;
        let cell_stride_w = self.cell_stride_w;
        let cell_stride_h = self.cell_stride_h;

        // Render cells
        for (flat_idx, id, title, artist, year, _, _) in &visible {
            let row = flat_idx / self.cols;
            let col = flat_idx % self.cols;
            if row >= self.scroll_row + rows_visible || row >= total_rows { continue; }

            let x = grid_area.x + (col as u16) * cell_stride_w;
            let y = grid_area.y + ((row - self.scroll_row) as u16) * cell_stride_h;
            if x >= grid_area.x + grid_area.width || y >= grid_area.y + grid_area.height { continue; }

            let w = img_cols.min(grid_area.x + grid_area.width - x);
            let h = (IMG_H + TEXT_PADDING + TEXT_H).min(grid_area.y + grid_area.height - y);
            let cell_rect = Rect { x, y, width: w, height: h };

            let is_sel = *flat_idx == self.selected;
            let protocol = self.image_cache.get_mut(id);
            render_cell(frame, cell_rect, title, artist, *year, is_sel, focused, protocol, img_cols);
        }

        // Scrollbar
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
    img_cols: u16,
) {
    let img_h = IMG_H.min(area.height);
    let img_w = img_cols.min(area.width);
    let img_rect = Rect { width: img_w, height: img_h, ..area };
    let text_y = area.y + img_h + TEXT_PADDING;
    let text_h = area.height.saturating_sub(img_h + TEXT_PADDING);
    let text_rect = Rect { y: text_y, height: text_h, width: img_w, x: area.x };

    if let Some(proto) = protocol {
        frame.render_stateful_widget(
            StatefulImage::new().resize(Resize::Crop(None)),
            img_rect,
            proto,
        );
    } else {
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::DarkGray)),
            img_rect,
        );
    }

    if text_rect.height == 0 { return; }

    let title_style = if is_selected && focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let max_w = img_w as usize;
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
