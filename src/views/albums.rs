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
use tornade_core::{models::Album, services::LibraryService};

// Image height in terminal rows (fixed); width is computed from font metrics to make it square
const IMG_H: u16 = 12;
// Text rows below the image
const TEXT_H: u16 = 3;
// Padding rows between image bottom and text
const TEXT_PADDING: u16 = 1;
// Gap between cells (cols/rows)
const GAP_W: u16 = 2;
const GAP_H: u16 = 1;
// Left/right padding inside the grid area
const GRID_PAD: u16 = 1;
/// Images decoded in background threads, waiting to be encoded by the picker on the main thread.
/// None signals a failed load so the id is removed from loading_ids without caching.
type PendingQueue = Arc<Mutex<Vec<(i64, Option<image::DynamicImage>)>>>;

use crate::views::ViewMode;

pub struct AlbumsState {
    pub albums: Vec<Album>,
    pub search_results: Option<Vec<Album>>,
    pub filter: String,
    pub filter_active: bool,
    pub mode: ViewMode,
    pub selected: usize,
    pub scroll_row: usize,
    pub cols: usize,
    pub last_grid_area: Rect,
    pub search_bar_area: Option<Rect>,
    pub list_area: Option<Rect>,
    pub list_mode_state: ListState,
    // computed each render, stored for click detection between renders
    img_cols: u16,
    cell_stride_w: u16,
    cell_stride_h: u16,
    image_cache: HashMap<i64, StatefulProtocol>,
    // Background loading
    pending_decoded: PendingQueue,
    loading_ids: HashSet<i64>,
    failed_ids: HashSet<i64>,
}

impl Default for AlbumsState {
    fn default() -> Self {
        Self {
            albums: Vec::new(),
            search_results: None,
            filter: String::new(),
            filter_active: false,
            mode: ViewMode::default(),
            selected: 0,
            scroll_row: 0,
            cols: 4,
            last_grid_area: Rect::default(),
            search_bar_area: None,
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

impl AlbumsState {
    pub fn load(&mut self, library: &LibraryService) {
        self.albums = library
            .list_albums(None, None, None, Some(5000), Some(0))
            .unwrap_or_default();
    }

    pub fn display_albums(&self) -> &[Album] {
        self.search_results.as_deref().unwrap_or(&self.albums)
    }

    pub fn selected_album(&self) -> Option<&Album> {
        self.display_albums().get(self.selected)
    }

    pub fn album_at_pos(&self, x: u16, y: u16) -> Option<usize> {
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
        let total = self.display_albums().len();
        if idx >= total { None } else { Some(idx) }
    }

    pub fn move_right(&mut self) {
        let len = self.display_albums().len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    pub fn move_left(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let len = self.display_albums().len();
        if len == 0 {
            return;
        }
        self.selected = (self.selected + self.cols).min(len - 1);
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.cols);
    }

    pub fn page_down(&mut self) {
        let len = self.display_albums().len();
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
        let len = self.display_albums().len();
        if len > 0 {
            self.selected = len - 1;
        }
    }

    /// Render the albums grid. Returns `true` when background image loads are still in progress
    /// (caller should redraw soon).
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        picker: &mut Picker,
        tui_dir: &Path,
        artwork: bool,
    ) -> bool {
        self.render_impl(frame, area, focused, picker, tui_dir, artwork)
    }

    /// Toggle between the compact list and the artwork grid.
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            ViewMode::List => ViewMode::Grid,
            ViewMode::Grid => ViewMode::List,
        };
    }

    /// Compact text list of albums (title / artist / year). No image decoding.
    fn render_list(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        self.cols = 1; // one album per row → up/down navigation
        let albums = self.display_albums();
        let len = albums.len();
        let items: Vec<ListItem> = albums
            .iter()
            .map(|a| {
                let year = a.year.map(|y| y.to_string()).unwrap_or_default();
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<42} ", truncate(&a.title, 41))),
                    Span::styled(
                        format!("{:<28} ", truncate(&a.artist_name, 27)),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(year, Style::default().fg(Color::DarkGray)),
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

    #[allow(clippy::too_many_arguments)]
    fn render_impl(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        picker: &mut Picker,
        tui_dir: &Path,
        artwork: bool,
    ) -> bool {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
        render_search_bar(frame, chunks[0], &self.filter, self.filter_active);
        self.search_bar_area = Some(chunks[0]);
        let area = chunks[2];

        // List mode: compact text rows, no artwork decoding at all.
        if self.mode == ViewMode::List {
            self.render_list(frame, area, focused);
            return false;
        }

        // Width in columns that makes the image square in *pixels* for the
        // current font cell aspect: img_cols * font_w ≈ IMG_H * font_h. Keeping
        // this exact is what lets the (square) thumbnail fill the cell instead of
        // being letterboxed; a hard minimum wider than this would reintroduce a
        // gap, so only clamp to a small floor.
        let font = picker.font_size();
        self.img_cols = if font.0 > 0 {
            ((IMG_H as u32 * font.1 as u32) / font.0 as u32) as u16
        } else {
            IMG_H
        };
        self.img_cols = self.img_cols.max(6);

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

        let total = self.display_albums().len();
        if total > 0 && self.selected >= total {
            self.selected = total - 1;
        }

        let sel_row = self.selected.checked_div(self.cols).unwrap_or(0);
        if sel_row < self.scroll_row {
            self.scroll_row = sel_row;
        } else if sel_row >= self.scroll_row + rows_visible {
            self.scroll_row = sel_row + 1 - rows_visible;
        }

        let total_rows = total.div_ceil(self.cols);
        let start = self.scroll_row * self.cols;
        let end = ((self.scroll_row + rows_visible) * self.cols).min(total);

        // Collect visible album data (owned) to release the immutable borrow on self
        type AlbumTuple = (
            usize,
            i64,
            String,
            String,
            Option<u16>,
            Option<std::path::PathBuf>,
            Option<std::path::PathBuf>,
        );
        let visible: Vec<AlbumTuple> = {
            let filtered = self.display_albums();
            filtered[start..end]
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    (
                        start + i,
                        a.id,
                        a.title.clone(),
                        a.artist_name.clone(),
                        a.year,
                        a.online_artwork_path.clone(),
                        a.artwork_path.clone(),
                    )
                })
                .collect()
        };

        // 1. Drain up to 3 images per frame from background threads → encode into
        //    StatefulProtocol. Rate-limited to keep the UI responsive (encoding is
        //    CPU-intensive). Remaining images are picked up on the next 30ms redraw.
        if let Ok(mut pending) = self.pending_decoded.try_lock() {
            let mut encoded = 0u32;
            pending.retain(|(id, result)| {
                if encoded >= 3 {
                    return true; // keep for next frame
                }
                self.loading_ids.remove(id);
                if let Some(img) = result {
                    self.image_cache
                        .insert(*id, picker.new_resize_protocol(img.clone()));
                } else {
                    self.failed_ids.insert(*id);
                }
                encoded += 1;
                false // remove from pending
            });
        }

        // 2. Spawn background threads for visible images not yet in cache or loading.
        //    Thumbnails are pre-sized at target resolution, just load from disk.
        //    Skipped entirely in text-only mode (`:artwork off`).
        if artwork {
            for (_, id, _, _, _, online, local) in &visible {
                if ACTIVE_IMAGE_THREADS.load(Ordering::Relaxed) >= MAX_IMAGE_THREADS {
                    break;
                }
                if self.image_cache.contains_key(id)
                    || self.loading_ids.contains(id)
                    || self.failed_ids.contains(id)
                {
                    continue;
                }
                let path = online
                    .as_ref()
                    .map(|p| crate::tui_artwork::resolve_artwork_path(p, tui_dir))
                    .or_else(|| local.as_ref().cloned());
                if let Some(path) = path {
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

        // Whether any images are still pending (loading or waiting to be encoded).
        // Always false in text-only mode so the redraw loop stays at idle cadence.
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
        for (flat_idx, id, title, artist, year, _, _) in &visible {
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
            render_cell(
                frame, cell_rect, title, artist, *year, is_sel, focused, protocol, img_cols,
            );
        }

        has_pending
    }
}

#[allow(clippy::too_many_arguments)]
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
    let img_rect = Rect {
        width: img_w,
        height: img_h,
        ..area
    };
    let text_y = area.y + img_h + TEXT_PADDING;
    let text_h = area.height.saturating_sub(img_h + TEXT_PADDING);
    let text_rect = Rect {
        y: text_y,
        height: text_h,
        width: img_w,
        x: area.x,
    };

    if let Some(proto) = protocol {
        frame.render_stateful_widget(
            // Crop fills the whole cell (covers) instead of letterboxing a
            // smaller image inside it — thumbnails are supersampled so there is
            // enough resolution and the crop of a square cover is negligible.
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

    if text_rect.height == 0 {
        return;
    }

    let title_style = if is_selected && focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let max_w = img_w as usize;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(truncate(title, max_w), title_style)),
            Line::from(Span::styled(
                truncate(artist, max_w),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                year.map(|y| y.to_string()).unwrap_or_default(),
                Style::default().fg(Color::DarkGray),
            )),
        ]),
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
