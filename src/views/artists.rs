use crate::utils::truncate;
use crate::views::{ACTIVE_IMAGE_THREADS, MAX_IMAGE_THREADS};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use ratatui_image::{Resize, StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tornade_core::models::Artist;
use tornade_core::services::LibraryService;

// Image height in terminal rows (fixed); width computed from font metrics for square cells
const IMG_H: u16 = 12;
// Text rows below the image
const TEXT_H: u16 = 2;
// Padding rows between image bottom and text
const TEXT_PADDING: u16 = 1;
// Gap between cells
const GAP_W: u16 = 2;
const GAP_H: u16 = 1;
// Left/right padding inside the grid area
const GRID_PAD: u16 = 1;

/// None signals a failed load so the id is removed from loading_ids without caching.
type PendingQueue = Arc<Mutex<Vec<(i64, Option<image::DynamicImage>)>>>;

pub struct ArtistsState {
    pub artists: Vec<Artist>,
    pub search_results: Option<Vec<Artist>>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_active: bool,
    pub mode: crate::views::ViewMode,
    pub selected: usize,
    pub scroll_row: usize,
    pub cols: usize,
    pub search_bar_area: Option<Rect>,
    pub last_grid_area: Rect,
    pub list_area: Option<Rect>,
    pub list_mode_state: ListState,
    img_cols: u16,
    cell_stride_w: u16,
    cell_stride_h: u16,
    image_cache: HashMap<i64, StatefulProtocol>,
    pending_decoded: PendingQueue,
    loading_ids: HashSet<i64>,
    failed_ids: HashSet<i64>,
}

impl Default for ArtistsState {
    fn default() -> Self {
        Self {
            artists: Vec::new(),
            search_results: None,
            list_state: ListState::default(),
            filter: String::new(),
            filter_active: false,
            mode: crate::views::ViewMode::default(),
            selected: 0,
            scroll_row: 0,
            cols: 4,
            search_bar_area: None,
            last_grid_area: Rect::default(),
            list_area: None,
            list_mode_state: ListState::default(),
            img_cols: IMG_H,
            cell_stride_w: IMG_H + GAP_W,
            cell_stride_h: IMG_H + TEXT_PADDING + TEXT_H + GAP_H,
            image_cache: HashMap::new(),
            pending_decoded: Arc::new(Mutex::new(Vec::new())),
            loading_ids: HashSet::new(),
            failed_ids: HashSet::new(),
        }
    }
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

    pub fn artist_at_pos(&self, x: u16, y: u16) -> Option<usize> {
        let a = self.last_grid_area;
        if a.width == 0 || x < a.x || y < a.y || x >= a.x + a.width || y >= a.y + a.height {
            return None;
        }
        let col = ((x - a.x) / self.cell_stride_w) as usize;
        let row_in_view = ((y - a.y) / self.cell_stride_h) as usize;
        if col >= self.cols {
            return None;
        }
        let row = self.scroll_row + row_in_view;
        let idx = row * self.cols + col;
        let total = self.display_artists().len();
        if idx >= total { None } else { Some(idx) }
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
        if len == 0 {
            return;
        }
        self.selected = (self.selected + self.cols).min(len - 1);
    }
    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.cols);
    }
    pub fn page_down(&mut self) {
        let len = self.display_artists().len();
        if len == 0 {
            return;
        }
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
        if len > 0 {
            self.selected = len - 1;
        }
    }

    // Keep list_state in sync so existing event code using list_state still works
    fn sync_list_state(&mut self) {
        self.list_state.select(Some(self.selected));
    }

    /// Toggle between the compact list and the artwork grid.
    pub fn toggle_mode(&mut self) {
        self.mode.toggle();
    }

    /// Compact text list of artists (name + formed year / country).
    fn render_list(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        self.cols = 1;
        let artists = self.display_artists();
        let len = artists.len();
        let items: Vec<ListItem> = artists
            .iter()
            .map(|a| {
                let mut extra = Vec::new();
                if let Some(y) = a.formed_year {
                    extra.push(y.to_string());
                }
                if let Some(ref c) = a.country {
                    extra.push(c.clone());
                }
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<44} ", truncate(&a.name, 43))),
                    Span::styled(extra.join(" · "), Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();
        let hl = if focused {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let mut ls = ListState::default();
        if len > 0 {
            ls.select(Some(self.selected.min(len - 1)));
        }
        frame.render_stateful_widget(
            List::new(items).highlight_style(hl).highlight_symbol("> "),
            area,
            &mut ls,
        );
        self.list_area = Some(area);
        self.list_mode_state = ls;
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        picker: &mut Picker,
        tui_dir: &Path,
        artwork: bool,
    ) -> bool {
        self.sync_list_state();

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
        render_search_bar(frame, chunks[0], &self.filter, self.filter_active);
        self.search_bar_area = Some(chunks[0]);
        let area = chunks[2];

        // List mode: compact text rows, no artwork decoding.
        if self.mode == crate::views::ViewMode::List {
            self.render_list(frame, area, focused);
            return false;
        }

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
        self.last_grid_area = grid_area;

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
        //    Thumbnails are pre-generated at exact target pixel size, no runtime resize needed.
        if let Ok(mut pending) = self.pending_decoded.try_lock() {
            for (id, result) in pending.drain(..) {
                self.loading_ids.remove(&id);
                if let Some(img) = result {
                    self.image_cache.insert(id, picker.new_resize_protocol(img));
                } else {
                    self.failed_ids.insert(id);
                }
            }
        }

        // 2. Spawn background threads for visible artists not yet in cache or
        //    loading. Skipped in text-only mode (`:artwork off`).
        if artwork {
            for (_, id, _, photo_path) in &visible {
                if ACTIVE_IMAGE_THREADS.load(Ordering::Relaxed) >= MAX_IMAGE_THREADS {
                    break;
                }
                if self.image_cache.contains_key(id)
                    || self.loading_ids.contains(id)
                    || self.failed_ids.contains(id)
                {
                    continue;
                }
                if let Some(orig) = photo_path {
                    let path = crate::tui_artwork::resolve_artwork_path(orig, tui_dir);
                    self.loading_ids.insert(*id);
                    ACTIVE_IMAGE_THREADS.fetch_add(1, Ordering::Relaxed);
                    let id = *id;
                    let pending = Arc::clone(&self.pending_decoded);
                    std::thread::spawn(move || {
                        let result = image::open(&path).ok();
                        if let Ok(mut guard) = pending.lock() {
                            guard.push((id, result));
                        }
                        ACTIVE_IMAGE_THREADS.fetch_sub(1, Ordering::Relaxed);
                        crate::wake::signal();
                    });
                }
            }
        }

        let has_pending = artwork
            && (!self.loading_ids.is_empty()
                || self
                    .pending_decoded
                    .try_lock()
                    .map(|g| !g.is_empty())
                    .unwrap_or(true));

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
            let cell_rect = Rect {
                x,
                y,
                width: w,
                height: h,
            };

            let is_sel = *flat_idx == self.selected;
            let protocol = if artwork {
                self.image_cache.get_mut(id)
            } else {
                None
            };
            render_cell(frame, cell_rect, name, is_sel, focused, protocol, img_cols);
        }

        has_pending
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
    let img_rect = Rect {
        width: img_w,
        height: img_h,
        ..area
    };

    if let Some(proto) = protocol {
        frame.render_stateful_widget(
            StatefulImage::new().resize(Resize::Crop(None)),
            img_rect,
            proto,
        );
    } else {
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(50, 50, 55))),
            img_rect,
        );
    }

    let text_y = area.y + img_h + TEXT_PADDING;
    if text_y >= area.y + area.height {
        return;
    }
    let text_h = area.height.saturating_sub(img_h + TEXT_PADDING);
    let text_rect = Rect {
        y: text_y,
        height: text_h,
        width: img_w,
        x: area.x,
    };
    if text_rect.height == 0 {
        return;
    }

    let name_style = if is_selected && focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    frame.render_widget(
        Paragraph::new(vec![Line::from(Span::styled(
            truncate(name, img_w as usize),
            name_style,
        ))]),
        text_rect,
    );
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
