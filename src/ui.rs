use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, Paragraph},
};

use crate::{
    app::{AppState, FocusedPanel, InputMode, StatusKind},
    views::View,
    widgets::{command_bar, confirm_dialog, help_overlay, input_dialog, player_bar, sidebar},
};

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

pub fn draw(frame: &mut Frame, app: &mut AppState) {
    let full_area = frame.area();
    if full_area.width < MIN_WIDTH || full_area.height < MIN_HEIGHT {
        let msg = format!(
            "Terminal too small — resize to at least {}x{}  (current: {}x{})",
            MIN_WIDTH, MIN_HEIGHT, full_area.width, full_area.height
        );
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::Yellow))
            .alignment(ratatui::layout::Alignment::Center);
        let y = full_area.height / 2;
        let area = Rect {
            x: full_area.x,
            y,
            width: full_area.width,
            height: 1,
        };
        frame.render_widget(p, area);
        return;
    }

    let area = margin_rect(full_area, 1);

    // Body layout: sidebar (fixed) + content (fills remaining) + right panel (fixed)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(18), // sidebar
            Constraint::Min(0),     // content
            Constraint::Length(44), // right panel: queue + player
        ])
        .split(area);

    let sidebar_area = body_chunks[0];
    let content_area = pad_h(body_chunks[1], 1);
    let right_panel_area = pad_h(body_chunks[2], 1);

    app.sidebar_area = Some(sidebar_area);

    // 1. Sidebar
    let active_playlist_id = match app.nav.current() {
        crate::views::View::PlaylistDetail(s) => Some(s.playlist.id),
        _ => None,
    };
    sidebar::render(
        frame,
        sidebar_area,
        app.nav.current().sidebar_entry(),
        active_playlist_id,
        matches!(app.focused_panel, FocusedPanel::Sidebar),
        app.sidebar_cursor,
        &app.sidebar_playlists,
    );

    // 2. Content area (view + status bar at bottom)
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(content_area);

    app.has_pending_images = render_view(frame, app, content_chunks[0]);
    render_status(frame, app, content_chunks[1]);

    // 3. Right panel: filter (1) + queue (fills) + toolbar (3) + player (7)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(7),
        ])
        .split(right_panel_area);

    app.right_queue_area = Some(right_chunks[1]);
    crate::views::queue::render_filter_bar(
        frame,
        right_chunks[0],
        &app.queue_filter,
        app.queue_filter_active,
    );
    render_right_queue(frame, app, right_chunks[1]);
    app.toolbar_hit_zones = crate::views::queue::render_toolbar(
        frame,
        right_chunks[2],
        app.player_cache.shuffle,
        &app.player_cache.repeat,
    );
    let album_name = app.current_album_name.as_deref();
    app.player_hit_zones = player_bar::render(
        frame,
        right_chunks[3],
        &app.player_cache,
        if app.artwork_enabled {
            app.player_artwork.as_mut()
        } else {
            None
        },
        album_name,
    );

    // 4. Overlays (rendered on top)
    render_overlays(frame, app, area);
}

/// Returns true when background image loads are still in progress (caller should redraw soon).
fn render_view(frame: &mut Frame, app: &mut AppState, area: Rect) -> bool {
    let focused = matches!(app.focused_panel, FocusedPanel::Content);
    let tui_album_dir = crate::tui_artwork::tui_album_dir(&app.paths, app.tui_target);
    let tui_artist_dir = crate::tui_artwork::tui_artist_dir(&app.paths, app.tui_target);
    let selection = app.selection.clone();
    let artwork = app.artwork_enabled;
    match app.nav.current_mut() {
        View::Library(s) => {
            s.render(frame, area, focused, &selection);
            false
        }
        View::Albums(s) => s.render(
            frame,
            area,
            focused,
            &mut app.picker,
            &tui_album_dir,
            artwork,
        ),
        View::Artists(s) => s.render(
            frame,
            area,
            focused,
            &mut app.picker,
            &tui_artist_dir,
            artwork,
        ),
        View::Genres(s) => s.render(
            frame,
            area,
            focused,
            &mut app.picker,
            &tui_album_dir,
            artwork,
        ),
        View::Playlists(s) => {
            s.render(frame, area, focused);
            false
        }
        View::AlbumDetail(s) => {
            s.render(frame, area, focused, &selection);
            false
        }
        View::ArtistDetail(s) => s.render(frame, area, focused, &mut app.picker, artwork),
        View::GenreDetail(s) => {
            s.render(frame, area, focused, &selection);
            false
        }
        View::PlaylistDetail(s) => {
            s.render(frame, area, focused, &selection);
            false
        }
        View::Scan(s) => {
            s.render(frame, area);
            false
        }
        View::Search(s) => {
            s.render(frame, area, focused, &selection);
            false
        }
        View::Queue(s) => {
            let _ = s;
            render_queue_view(frame, app, area);
            false
        }
    }
}

fn render_queue_view(frame: &mut Frame, app: &mut AppState, area: Rect) {
    // Use cached tracks resolved in tick() — no DB queries per frame
    let active_index = app.player_cache.queue_index;
    let skipped = app.player_cache.skipped_track_ids.clone();
    let focused = matches!(app.focused_panel, FocusedPanel::Content);
    // Borrow cached_queue_tracks and nav as separate fields so the borrow checker is happy
    let tracks = &app.cached_queue_tracks;
    let selection = app.selection.clone();
    if let View::Queue(s) = app.nav.current_mut() {
        s.sync_selection(tracks.len(), active_index);
        s.render(
            frame,
            area,
            tracks,
            active_index,
            &skipped,
            focused,
            &selection,
        );
    }
}

fn render_right_queue(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let active_index = app.player_cache.queue_index;
    let skipped = app.player_cache.skipped_track_ids.clone();
    let tracks = &app.cached_queue_tracks;
    let focused = matches!(app.focused_panel, FocusedPanel::RightPanel);

    // Auto-scroll to active track only when the user is not navigating the panel
    if !focused {
        if tracks.is_empty() {
            app.right_panel_queue_state.select(None);
        } else {
            app.right_panel_queue_state
                .select(Some(active_index.min(tracks.len() - 1)));
        }
    }

    crate::views::queue::render_panel(
        frame,
        area,
        tracks,
        active_index,
        &skipped,
        &mut app.right_panel_queue_state,
        focused,
        &app.queue_filter,
    );
}

fn render_status(frame: &mut Frame, app: &AppState, area: Rect) {
    let (text, style) = match &app.status {
        Some(s) => {
            let color = match s.kind {
                StatusKind::Info => Color::Cyan,
                StatusKind::Success => Color::Green,
                StatusKind::Error => Color::Red,
            };
            (s.text.as_str(), Style::default().fg(color))
        }
        None => ("", Style::default().fg(Color::DarkGray)),
    };
    frame.render_widget(Paragraph::new(Line::from(Span::styled(text, style))), area);
}

fn render_overlays(frame: &mut Frame, app: &mut AppState, area: Rect) {
    // Command bar
    if app.input_mode == InputMode::Command {
        command_bar::render(frame, area, &app.command_input, &app.command_completions);
    }

    // Text input dialog
    if app.input_mode == InputMode::TextInput
        && let Some(ref ctx) = app.text_input
    {
        input_dialog::render(frame, &ctx.prompt, &ctx.value);
    }

    // Confirm dialog
    if app.input_mode == InputMode::Confirm
        && let Some(ref ctx) = app.confirm
    {
        confirm_dialog::render(frame, &ctx.prompt);
    }

    // Playlist selector overlay
    if app.show_playlist_selector {
        render_playlist_selector(frame, app, area);
    }

    // Tag editor overlay
    if let Some(ref editor) = app.tag_editor {
        crate::widgets::tag_editor::render(frame, editor);
    }

    // US3 overlays sit on top of the tag editor.
    if let Some(ref picker) = app.scrape_picker {
        crate::widgets::scrape_picker::render(frame, picker);
    }
    if let Some(ref menu) = app.artwork_menu {
        crate::widgets::artwork_menu::render(frame, menu);
    }

    // Help overlay (always on top)
    if app.show_help {
        help_overlay::render(frame);
    }

    if app.show_stats {
        crate::widgets::stats_overlay::render(frame, &app.stats_lines);
    }

    if app.show_prefs {
        crate::widgets::stats_overlay::render_titled(frame, " Preferences ", &app.prefs_lines);
    }

    if app.show_vu {
        let track = app.player_cache.current_track.as_ref();
        crate::widgets::vu_overlay::render(
            frame,
            &app.vu,
            track.map(|t| t.title.as_str()),
            track.and_then(|t| t.artist_names.first().map(String::as_str)),
        );
    }
}

fn render_playlist_selector(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let playlists = app.playlists.list_playlists().unwrap_or_default();
    let popup_area = centered_rect(50, 50, area);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = playlists
        .iter()
        .map(|p| ListItem::new(Line::from(Span::raw(p.name.as_str()))))
        .collect();

    let list = List::new(items)
        .block(Block::default())
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, popup_area, &mut app.playlist_selector_state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let height = r.height * percent_y / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}

fn margin_rect(area: Rect, m: u16) -> Rect {
    Rect {
        x: area.x + m,
        y: area.y + m,
        width: area.width.saturating_sub(m * 2),
        height: area.height.saturating_sub(m * 2),
    }
}

fn pad_h(area: Rect, p: u16) -> Rect {
    Rect {
        x: area.x + p,
        y: area.y,
        width: area.width.saturating_sub(p * 2),
        height: area.height,
    }
}
