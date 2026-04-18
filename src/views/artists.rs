use crate::utils::truncate;
use image::Rgba;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use ratatui_image::{Resize, StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tornade_core::models::Artist;
use tornade_core::services::LibraryService;

// Image height in terminal rows (fixed); width computed from font metrics for square cells
const IMG_H: u16 = 8;
// Text rows below the image
const TEXT_H: u16 = 2;
// Padding rows between image bottom and text
const TEXT_PADDING: u16 = 1;
// Gap between cells
const GAP_W: u16 = 3;
const GAP_H: u16 = 2;
// Left/right padding inside the grid area
const GRID_PAD: u16 = 1;

type PendingQueue = Arc<Mutex<Vec<(i64, image::DynamicImage)>>>;

pub struct ArtistsState {
    pub artists: Vec<Artist>,
    pub search_results: Option<Vec<Artist>>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_active: bool,
    pub selected: usize,
    pub scroll_row: usize,
    pub cols: usize,
    img_cols: u16,
    cell_stride_w: u16,
    cell_stride_h: u16,
    image_cache: HashMap<i64, StatefulProtocol>,
    scrollbar_state: ScrollbarState,
    pending_decoded: PendingQueue,
    loading_ids: HashSet<i64>,
}

impl Default for ArtistsState {
    fn default() -> Self {
        Self {
            artists: Vec::new(),
            search_results: None,
            list_state: ListState::default(),
            filter: String::new(),
            filter_active: false,
            selected: 0,
            scroll_row: 0,
            cols: 4,
            img_cols: IMG_H,
            cell_stride_w: IMG_H + GAP_W,
            cell_stride_h: IMG_H + TEXT_PADDING + TEXT_H + GAP_H,
            image_cache: HashMap::new(),
            scrollbar_state: ScrollbarState::default(),
            pending_decoded: Arc::new(Mutex::new(Vec::new())),
            loading_ids: HashSet::new(),
        }
    }
}

/// Zero alpha outside the inscribed circle (for circular photo appearance).
fn apply_circle_mask(img: image::DynamicImage) -> image::DynamicImage {
    let (w, h) = (img.width(), img.height());
    let mut rgba = img.into_rgba8();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let r = cx.min(cy);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy > r * r {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
    image::DynamicImage::ImageRgba8(rgba)
}

impl ArtistsState {
    pub fn load(&mut self, library: &LibraryService) {
        self.artists = library.list_artists().unwrap_or_default();
        // Reset selection only on first load
        if self.artists.is_empty() {
            self.selected = 0;
        }
        self.selected = self.selected.min(self.artists.len().saturating_sub(1));
    }

    pub fn display_artists(&self) -> &[Artist] {
        self.search_results.as_deref().unwrap_or(&self.artists)
    }

    pub fn selected_artist(&self) -> Option<&Artist> {
        self.display_artists().get(self.selected)
    }

    pub fn move_right(&mut self) {
        let len = self.display_artists().len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }
    pub fn move_left(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
    pub fn move_down(&mut self) {
        let len = self.display_artists().len();
        if len == 0 { return; }
        self.selected = (self.selected + self.cols).min(len - 1);
    }
    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.cols);
    }
    pub fn page_down(&mut self) {
        let len = self.display_artists().len();
        if len == 0 { return; }
        self.selected = (self.selected + self.cols * 3).min(len - 1);
    }
    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.cols * 3);
    }
    pub fn jump_top(&mut self) {
        self.selected = 0;
        self.scroll_row = 0;
    }
    pub fn jump_bottom(&mut self) {
        let len = self.display_artists().len();
        if len > 0 { self.selected = len - 1; }
    }

    // Keep list_state in sync so existing event code using list_state still works
    fn sync_list_state(&mut self) {
        self.list_state.select(Some(self.selected));
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        picker: &mut Picker,
    ) -> bool {
        self.sync_list_state();

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
        render_search_bar(frame, chunks[0], &self.filter, self.filter_active);
        let area = chunks[2];

        let font = picker.font_size();
        self.img_cols = if font.0 > 0 {
            ((IMG_H as u32 * font.1 as u32) / font.0 as u32) as u16
        } else {
            IMG_H
        };
        self.img_cols = self.img_cols.max(10);

        self.cell_stride_w = self.img_cols + GAP_W;
        self.cell_stride_h = IMG_H + TEXT_PADDING + TEXT_H + GAP_H;

        let grid_area = Rect {
            x: area.x + GRID_PAD,
            width: area.width.saturating_sub(GRID_PAD * 2 + 1),
            ..area
        };

        self.cols = ((grid_area.width / self.cell_stride_w) as usize).max(1);
        let rows_visible = ((grid_area.height / self.cell_stride_h) as usize).max(1);

        let display: &[Artist] = self.search_results.as_deref().unwrap_or(&self.artists);
        let total = display.len();

        if total > 0 && self.selected >= total {
            self.selected = total - 1;
        }

        let sel_row = self.selected / self.cols.max(1);
        if sel_row < self.scroll_row {
            self.scroll_row = sel_row;
        } else if sel_row >= self.scroll_row + rows_visible {
            self.scroll_row = sel_row + 1 - rows_visible;
        }

        let total_rows = total.div_ceil(self.cols.max(1));
        let start = self.scroll_row * self.cols;
        let end = ((self.scroll_row + rows_visible) * self.cols).min(total);

        // Collect visible artist data (owned) to release borrow on self
        let visible: Vec<(usize, i64, String, Option<std::path::PathBuf>)> = display[start..end]
            .iter()
            .enumerate()
            .map(|(i, a)| (start + i, a.id, a.name.clone(), a.photo_path.clone()))
            .collect();

        // 1. Drain images decoded by background threads → encode into StatefulProtocol
        if let Ok(mut pending) = self.pending_decoded.try_lock() {
            for (id, img) in pending.drain(..) {
                self.loading_ids.remove(&id);
                self.image_cache.insert(id, picker.new_resize_protocol(img));
            }
        }

        // 2. Spawn background threads for visible artists not yet in cache or loading
        let (fw, fh) = (font.0, font.1);
        let target_px = ((self.img_cols as u32) * (fw as u32)).max(1);
        for (_, id, _, photo_path) in &visible {
            if self.image_cache.contains_key(id) || self.loading_ids.contains(id) {
                continue;
            }
            if let Some(path) = photo_path.clone() {
                self.loading_ids.insert(*id);
                let id = *id;
                let pending = Arc::clone(&self.pending_decoded);
                let target_h = ((IMG_H as u32) * (fh as u32)).max(1);
                std::thread::spawn(move || {
                    if let Ok(img) = image::open(&path) {
                        let img = img.resize_to_fill(
                            target_px,
                            target_h,
                            image::imageops::FilterType::Triangle,
                        );
                        let img = apply_circle_mask(img);
                        if let Ok(mut guard) = pending.lock() {
                            guard.push((id, img));
                        }
                    }
                });
            }
        }

        let has_pending = !self.loading_ids.is_empty()
            || self.pending_decoded.try_lock().map(|g| !g.is_empty()).unwrap_or(true);

        let img_cols = self.img_cols;
        let cell_stride_w = self.cell_stride_w;
        let cell_stride_h = self.cell_stride_h;

        // 3. Render visible cells
        for (flat_idx, id, name, _) in &visible {
            let row = flat_idx / self.cols;
            let col = flat_idx % self.cols;
            if row >= self.scroll_row + rows_visible || row >= total_rows {
                continue;
            }
            let x = grid_area.x + (col as u16) * cell_stride_w;
            let y = grid_area.y + ((row - self.scroll_row) as u16) * cell_stride_h;
            if x >= grid_area.x + grid_area.width || y >= grid_area.y + grid_area.height {
                continue;
            }
            let w = img_cols.min(grid_area.x + grid_area.width - x);
            let h = (IMG_H + TEXT_PADDING + TEXT_H).min(grid_area.y + grid_area.height - y);
            let cell_rect = Rect { x, y, width: w, height: h };

            let is_sel = *flat_idx == self.selected;
            let protocol = self.image_cache.get_mut(id);
            render_cell(frame, cell_rect, name, is_sel, focused, protocol, img_cols);
        }

        // 4. Scrollbar
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

        has_pending
    }

    /// Fallback text list (used when filter is active and grid would be confusing).
    pub fn render_text_list(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let display = self.display_artists();
        let items: Vec<ListItem> = display
            .iter()
            .map(|a| ListItem::new(Line::from(Span::raw(truncate(&a.name, 60)))))
            .collect();
        let (hl_style, hl_sym) = if focused {
            (Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD), "> ")
        } else {
            (Style::default().fg(Color::DarkGray), "  ")
        };
        let list = List::new(items)
            .block(Block::default())
            .highlight_style(hl_style)
            .highlight_symbol(hl_sym);
        self.list_state.select(Some(self.selected));
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}

fn render_cell(
    frame: &mut Frame,
    area: Rect,
    name: &str,
    is_selected: bool,
    focused: bool,
    protocol: Option<&mut StatefulProtocol>,
    img_cols: u16,
) {
    let img_h = IMG_H.min(area.height);
    let img_w = img_cols.min(area.width);
    let img_rect = Rect { width: img_w, height: img_h, ..area };

    if let Some(proto) = protocol {
        frame.render_stateful_widget(StatefulImage::new().resize(Resize::Crop(None)), img_rect, proto);
    } else {
        frame.render_widget(Block::default().style(Style::default().bg(Color::Rgb(50, 50, 55))), img_rect);
    }

    let text_y = area.y + img_h + TEXT_PADDING;
    if text_y >= area.y + area.height {
        return;
    }
    let text_h = area.height.saturating_sub(img_h + TEXT_PADDING);
    let text_rect = Rect { y: text_y, height: text_h, width: img_w, x: area.x };
    if text_rect.height == 0 {
        return;
    }

    let name_style = if is_selected && focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(truncate(name, img_w as usize), name_style)),
        ]),
        text_rect,
    );
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
