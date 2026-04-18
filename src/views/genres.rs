use crate::utils::truncate;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
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
use tornade_core::{models::Genre, services::LibraryService};

/// Height in terminal rows for each genre row (image height).
const ROW_HEIGHT: u16 = 4;
/// Width reserved for the mosaic image.
const IMG_WIDTH: u16 = 8;

#[derive(Default)]
pub struct GenresState {
    pub genres: Vec<(Genre, u32, u32)>,
    pub filter: String,
    pub filter_active: bool,
    pub list_state: ListState,
    /// Artwork paths (up to 4) per genre, indexed parallel to `genres`.
    artwork_paths: Vec<Vec<PathBuf>>,
    /// Lazily loaded mosaic protocols indexed parallel to `genres`.
    /// `None` = not yet loaded; `Some(None)` = no mosaic available.
    image_states: Vec<Option<Option<StatefulProtocol>>>,
    scrollbar_state: ScrollbarState,
}

impl GenresState {
    pub fn load(&mut self, library: &LibraryService) {
        self.genres = library.list_genres().unwrap_or_default();
        if self.list_state.selected().is_none() && !self.genres.is_empty() {
            self.list_state.select(Some(0));
        }

        // Only collect paths - no I/O, no image decoding.
        let n = self.genres.len();
        self.artwork_paths = self
            .genres
            .iter()
            .map(|(g, _, _)| library.get_genre_artwork_paths(g.id, 4).unwrap_or_default())
            .collect();
        self.image_states = (0..n).map(|_| None).collect();
    }

    pub fn filtered_genres(&self) -> Vec<&(Genre, u32, u32)> {
        if self.filter.is_empty() {
            self.genres.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.genres
                .iter()
                .filter(|(g, _, _)| g.name.to_lowercase().contains(&q))
                .collect()
        }
    }

    pub fn selected_genre(&self) -> Option<&(Genre, u32, u32)> {
        let filtered = self.filtered_genres();
        self.list_state
            .selected()
            .and_then(|i| filtered.get(i).copied())
    }

    pub fn move_down(&mut self) {
        let len = self.filtered_genres().len();
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
        if !self.genres.is_empty() {
            self.list_state.select(Some(0));
        }
    }
    pub fn jump_bottom(&mut self) {
        let len = self.filtered_genres().len();
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

        let filtered = self.filtered_genres();
        let filtered_len = filtered.len();
        let selected = self.list_state.selected().unwrap_or(0);

        let has_photos = self.artwork_paths.iter().any(|p| !p.is_empty());
        let has_filter = !self.filter.is_empty();

        if has_photos && !has_filter && IMG_WIDTH + 2 < area.width {
            let orig_indices: Vec<usize> = filtered
                .iter()
                .filter_map(|fg| self.genres.iter().position(|(g, _, _)| g.id == fg.0.id))
                .collect();
            let names: Vec<String> = filtered.iter().map(|(g, _, _)| g.name.clone()).collect();
            drop(filtered);
            self.render_with_images(frame, chunks[2], &names, &orig_indices, selected, focused, picker);
        } else {
            let items: Vec<ListItem> = filtered
                .iter()
                .map(|(g, tracks, albums)| {
                    ListItem::new(Line::from(vec![
                        Span::raw(format!("{:<40} ", truncate(&g.name, 39))),
                        Span::styled(
                            format!("{} tracks", tracks),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("  {} albums", albums),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]))
                })
                .collect();
            drop(filtered);
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
            frame.render_stateful_widget(list, chunks[2], &mut self.list_state);
        }

        let pos = self.list_state.selected().unwrap_or(0);
        self.scrollbar_state = ScrollbarState::new(filtered_len).position(pos);
        frame.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            chunks[2],
            &mut self.scrollbar_state,
        );
    }

    fn render_with_images(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        names: &[String],
        orig_indices: &[usize],
        selected: usize,
        focused: bool,
        picker: &mut Picker,
    ) {
        if area.height == 0 || names.is_empty() {
            return;
        }

        let rows_visible = (area.height / ROW_HEIGHT) as usize;
        let scroll_offset = selected.saturating_sub(rows_visible.saturating_sub(1));
        let end = (scroll_offset + rows_visible + 1).min(names.len());

        // Lazy-load mosaics for the visible window only.
        for rel_idx in scroll_offset..end {
            let orig_idx = orig_indices.get(rel_idx).copied().unwrap_or(rel_idx);
            if orig_idx < self.image_states.len() && self.image_states[orig_idx].is_none() {
                let proto = self.artwork_paths.get(orig_idx)
                    .filter(|paths| !paths.is_empty())
                    .map(|paths| {
                        let images: Vec<DynamicImage> =
                            paths.iter().filter_map(|p| image::open(p).ok()).collect();
                        if images.is_empty() {
                            None
                        } else {
                            let mosaic = build_mosaic(&images, 128);
                            Some(picker.new_resize_protocol(DynamicImage::ImageRgba8(mosaic)))
                        }
                    })
                    .flatten();
                self.image_states[orig_idx] = Some(proto);
            }
        }

        let mut y = area.y;
        for rel_idx in scroll_offset..end {
            let is_selected = rel_idx == selected;
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

            let orig_idx = orig_indices.get(rel_idx).copied().unwrap_or(rel_idx);
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
            let name = names.get(rel_idx).map(|s| s.as_str()).unwrap_or("");
            if name_area.y < area.y + area.height {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("{}{}", prefix, truncate(name, name_w)),
                        name_style,
                    ))),
                    name_area,
                );
            }

            y += ROW_HEIGHT;
        }
    }
}

/// Build a 2x2 mosaic of `size x size` pixels from up to 4 images.
/// Missing tiles are filled with a dark background color.
fn build_mosaic(images: &[DynamicImage], size: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let tile = size / 2;
    let mut canvas = ImageBuffer::from_pixel(size, size, Rgba([30u8, 30, 30, 255]));

    let positions = [(0u32, 0u32), (tile, 0), (0, tile), (tile, tile)];

    for (i, (ox, oy)) in positions.iter().enumerate() {
        if let Some(img) = images.get(i) {
            let resized = img.resize_exact(tile, tile, image::imageops::FilterType::Lanczos3);
            for (px, py, pixel) in resized.pixels() {
                if *ox + px < size && *oy + py < size {
                    canvas.put_pixel(*ox + px, *oy + py, pixel);
                }
            }
        }
    }
    canvas
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
