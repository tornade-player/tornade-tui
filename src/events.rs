use std::time::Duration;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tornade_core::{
    models::RepeatMode,
    services::PlaybackState,
};

use crate::{
    app::{AppState, ConfirmAction, ConfirmCtx, FocusedPanel, InputMode, StatusKind, TextInputAction, TextInputCtx},
    commands::{Command, completions},
    player::parse_seek_position,
    utils::expand_tilde,
    views::{
        AlbumDetailState, ArtistDetailState, GenreDetailState, PlaylistDetailState, SidebarEntry, View,
    },
};

/// Process a mouse event.
pub fn handle_mouse(app: &mut AppState, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let col = mouse.column;
            let row = mouse.row;

            // Player transport buttons
            let zones = app.player_hit_zones;
            if zones.prev.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                let _ = app.player.previous();
                return;
            }
            if zones.play_pause.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                use tornade_core::services::PlaybackState;
                if app.player_cache.state == PlaybackState::Playing {
                    let _ = app.player.pause();
                } else {
                    let _ = app.player.resume();
                }
                return;
            }
            if zones.next.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                let _ = app.player.next();
                return;
            }
            // Toolbar buttons (random / repeat / shuffle / add / remove)
            let tz = app.toolbar_hit_zones;
            if tz.random.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                handle_add_random(app);
                return;
            }
            if tz.shuffle.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                let _ = app.player.set_shuffle(!app.player_cache.shuffle);
                return;
            }
            if tz.repeat.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                let next = match app.player_cache.repeat {
                    RepeatMode::Off => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::Off,
                };
                let _ = app.player.set_repeat(next);
                return;
            }
            if tz.add.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                handle_save_queue_as_playlist(app);
                return;
            }
            if tz.remove.map(|r| rect_contains(r, col, row)).unwrap_or(false) {
                handle_clear_queue_confirm(app);
                return;
            }

            // Click on an album in the grid
            let clicked_idx = if let View::Albums(s) = app.nav.current() {
                s.album_at_pos(col, row)
            } else {
                None
            };
            if let Some(idx) = clicked_idx {
                if let View::Albums(s) = app.nav.current_mut() {
                    s.selected = idx;
                }
                push_album_detail(app);
                return;
            }

            // Click on sidebar entry
            if let Some(area) = app.sidebar_area {
                if rect_contains(area, col, row) {
                    handle_sidebar_click(app, area, row);
                    return;
                }
            }

            // Click on track in library view (single = select, double = add to queue)
            let library_click = if let View::Library(s) = app.nav.current() {
                s.list_area.and_then(|a| {
                    if rect_contains(a, col, row) {
                        let idx = (row - a.y) as usize + s.list_state.offset();
                        if idx < s.display_tracks().len() { Some(idx) } else { None }
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            if let Some(track_idx) = library_click {
                let is_double = check_double_click(app, col, row);
                if let View::Library(s) = app.nav.current_mut() {
                    s.list_state.select(Some(track_idx));
                }
                app.focused_panel = FocusedPanel::Content;
                if is_double {
                    app.add_selected_to_queue();
                }
                return;
            }

            // Click on item in right-panel queue (single = select, double = play)
            let queue_click = app.right_queue_area.and_then(|area| {
                if !rect_contains(area, col, row) { return None; }
                let offset = app.right_panel_queue_state.offset();
                let rel = (row - area.y) as usize + offset;
                let filter_lower = app.queue_filter.to_lowercase();
                if app.queue_filter.is_empty() {
                    if rel < app.cached_queue_tracks.len() { Some((rel, rel)) } else { None }
                } else {
                    app.cached_queue_tracks.iter().enumerate()
                        .filter(|(_, t)| {
                            t.title.to_lowercase().contains(&filter_lower)
                                || t.artist_names.iter().any(|a| a.to_lowercase().contains(&filter_lower))
                        })
                        .nth(rel)
                        .map(|(orig, _)| (rel, orig))
                }
            });
            if let Some((filtered_idx, orig_idx)) = queue_click {
                let is_double = check_double_click(app, col, row);
                app.right_panel_queue_state.select(Some(filtered_idx));
                app.focused_panel = FocusedPanel::RightPanel;
                if is_double {
                    let _ = app.player.jump_to_index(orig_idx);
                    if app.player_cache.state != PlaybackState::Playing {
                        let _ = app.player.resume();
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => scroll_content(app, 1),
        MouseEventKind::ScrollUp => scroll_content(app, -1),
        _ => {}
    }
}

/// Returns true when this click is a double-click (same cell within 400ms).
/// Always records the current click as the last click.
fn check_double_click(app: &mut AppState, col: u16, row: u16) -> bool {
    let now = std::time::Instant::now();
    let is_double = app.last_click
        .map(|(lc, lr, ref lt)| lc == col && lr == row && lt.elapsed() < Duration::from_millis(400))
        .unwrap_or(false);
    app.last_click = Some((col, row, now));
    is_double
}

/// Sidebar entries in display order (must match LIBRARY_ENTRIES in widgets/sidebar.rs).
const SIDEBAR_DISPLAY_ORDER: [SidebarEntry; 5] = [
    SidebarEntry::Search,
    SidebarEntry::Tracks,
    SidebarEntry::Albums,
    SidebarEntry::Artists,
    SidebarEntry::Genres,
];

/// Handle a left-click inside the sidebar. Navigates directly to the clicked entry.
fn handle_sidebar_click(app: &mut AppState, area: ratatui::layout::Rect, row: u16) {
    // The sidebar block has a 1-row top border; items start at area.y + 1.
    // Item layout (0-indexed from the first item row):
    //   0: blank
    //   1: "Library" header
    //   2 + 2*i (i=0..4): library entry i  (matches SIDEBAR_DISPLAY_ORDER)
    //   12: blank (gap before "Playlists")
    //   13: "Playlists" header
    //   14: blank
    //   15 + 2*j (j=0..n): playlist j
    let item_index = row.saturating_sub(area.y + 1) as usize;
    if item_index >= 2 && item_index <= 10 && (item_index - 2) % 2 == 0 {
        let entry_pos = (item_index - 2) / 2;
        if let Some(&entry) = SIDEBAR_DISPLAY_ORDER.get(entry_pos) {
            app.navigate_to(entry);
            app.focused_panel = FocusedPanel::Content;
        }
    } else if item_index >= 15 && (item_index - 15) % 2 == 0 {
        let playlist_pos = (item_index - 15) / 2;
        let playlist_id = app.sidebar_playlists.get(playlist_pos).map(|&(id, _)| id);
        if let Some(id) = playlist_id {
            app.navigate_to_playlist(id);
            app.focused_panel = FocusedPanel::Content;
        }
    }
}

fn rect_contains(r: ratatui::layout::Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

fn scroll_content(app: &mut AppState, delta: i32) {
    if delta > 0 {
        move_down(app);
    } else {
        move_up(app);
    }
}

/// Process one key event. Returns `true` if the application should quit.
pub fn handle_key(app: &mut AppState, key: KeyEvent) -> bool {
    // Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }

    // Dispatch by current input mode
    match app.input_mode {
        InputMode::Normal => handle_normal(app, key),
        InputMode::Command => {
            handle_command_mode(app, key);
            false
        }
        InputMode::TextInput => {
            handle_text_input(app, key);
            false
        }
        InputMode::Confirm => {
            handle_confirm(app, key);
            false
        }
    }
}

// ── Normal mode ──────────────────────────────────────────────────────────────

fn handle_normal(app: &mut AppState, key: KeyEvent) -> bool {
    if app.show_help {
        if matches!(key.code, KeyCode::Char('?') | KeyCode::Esc) { app.show_help = false; }
        return false;
    }

    if app.show_playlist_selector {
        return handle_playlist_selector(app, key);
    }

    // ? toggles help from any panel
    if key.code == KeyCode::Char('?') {
        app.show_help = true;
        return false;
    }

    // Digit shortcuts (1-5) navigate library views from any panel
    if let KeyCode::Char(c @ '1'..='5') = key.code {
        if let Some(entry) = SidebarEntry::from_digit(c as u8 - b'0') {
            app.navigate_to(entry);
            app.focused_panel = FocusedPanel::Content;
            return false;
        }
    }

    // Tab / Shift+Tab cycle panel focus forward / backward
    if key.code == KeyCode::Tab {
        app.cycle_focus();
        return false;
    }
    if key.code == KeyCode::BackTab {
        app.cycle_focus_reverse();
        return false;
    }

    match app.focused_panel {
        FocusedPanel::Sidebar => handle_sidebar_focus(app, key),
        FocusedPanel::RightPanel => handle_right_panel_focus(app, key),
        FocusedPanel::Content => handle_content_focus(app, key),
    }
}

fn handle_sidebar_focus(app: &mut AppState, key: KeyEvent) -> bool {
    let library_len = SidebarEntry::all().len(); // 5
    let total = library_len + app.sidebar_playlists.len();
    match key.code {
        KeyCode::Esc => app.focused_panel = FocusedPanel::Content,
        KeyCode::Char('j') | KeyCode::Down => {
            if total > 0 { app.sidebar_cursor = (app.sidebar_cursor + 1).min(total - 1); }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.sidebar_cursor = app.sidebar_cursor.saturating_sub(1);
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            if app.sidebar_cursor < library_len {
                if let Some(&entry) = SidebarEntry::all().get(app.sidebar_cursor) {
                    app.navigate_to(entry);
                    app.focused_panel = FocusedPanel::Content;
                }
            } else {
                let playlist_idx = app.sidebar_cursor - library_len;
                if let Some(&(id, _)) = app.sidebar_playlists.get(playlist_idx) {
                    app.navigate_to_playlist(id);
                    app.focused_panel = FocusedPanel::Content;
                }
            }
        }
        // Digit shortcuts (1-5) navigate library items and return focus to content
        KeyCode::Char(c @ '1'..='5') => {
            if let Some(entry) = SidebarEntry::from_digit(c as u8 - b'0') {
                app.navigate_to(entry);
                app.focused_panel = FocusedPanel::Content;
            }
        }
        // Playback controls work from any panel
        KeyCode::Char(' ') => toggle_playback(app),
        KeyCode::Char('n') => { let _ = app.player.next(); }
        KeyCode::Char('N') => { let _ = app.player.previous(); }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            let _ = app.player.set_volume((app.player_cache.volume + 0.05).min(1.0));
        }
        KeyCode::Char('-') => {
            let _ = app.player.set_volume((app.player_cache.volume - 0.05).max(0.0));
        }
        KeyCode::Char('S') => { let _ = app.player.set_shuffle(!app.player_cache.shuffle); }
        KeyCode::Char('R') => {
            let next = match app.player_cache.repeat {
                RepeatMode::Off => RepeatMode::All,
                RepeatMode::All => RepeatMode::One,
                RepeatMode::One => RepeatMode::Off,
            };
            let _ = app.player.set_repeat(next);
        }
        KeyCode::Char('q') => return true,
        _ => {}
    }
    false
}

fn handle_right_panel_focus(app: &mut AppState, key: KeyEvent) -> bool {
    // Filter input mode: route all keys to the filter field
    if app.queue_filter_active {
        match key.code {
            KeyCode::Esc => {
                app.queue_filter_active = false;
                app.queue_filter.clear();
            }
            KeyCode::Enter => {
                app.queue_filter_active = false;
            }
            KeyCode::Backspace => { app.queue_filter.pop(); }
            KeyCode::Char(c) => { app.queue_filter.push(c); }
            _ => {}
        }
        return false;
    }

    let queue_len = app.cached_queue_tracks.len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
            app.focused_panel = FocusedPanel::Content;
        }
        KeyCode::Char('/') => {
            app.queue_filter_active = true;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if queue_len > 0 {
                let next = app.right_panel_queue_state.selected()
                    .map(|i| (i + 1).min(queue_len - 1))
                    .unwrap_or(0);
                app.right_panel_queue_state.select(Some(next));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let prev = app.right_panel_queue_state.selected()
                .map(|i| i.saturating_sub(1))
                .unwrap_or(0);
            if queue_len > 0 { app.right_panel_queue_state.select(Some(prev)); }
        }
        KeyCode::Char('g') => {
            if queue_len > 0 { app.right_panel_queue_state.select(Some(0)); }
        }
        KeyCode::Char('G') => {
            if queue_len > 0 { app.right_panel_queue_state.select(Some(queue_len - 1)); }
        }
        KeyCode::Enter => {
            if let Some(idx) = app.right_panel_queue_state.selected() {
                let _ = app.player.jump_to_index(idx);
                if app.player_cache.state != PlaybackState::Playing {
                    let _ = app.player.resume();
                }
            }
        }
        // Playback controls work from any panel
        KeyCode::Char(' ') => toggle_playback(app),
        KeyCode::Char('n') => { let _ = app.player.next(); }
        KeyCode::Char('N') => { let _ = app.player.previous(); }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            let _ = app.player.set_volume((app.player_cache.volume + 0.05).min(1.0));
        }
        KeyCode::Char('-') => {
            let _ = app.player.set_volume((app.player_cache.volume - 0.05).max(0.0));
        }
        KeyCode::Char('S') => { let _ = app.player.set_shuffle(!app.player_cache.shuffle); }
        KeyCode::Char('R') => {
            let next = match app.player_cache.repeat {
                RepeatMode::Off => RepeatMode::All,
                RepeatMode::All => RepeatMode::One,
                RepeatMode::One => RepeatMode::Off,
            };
            let _ = app.player.set_repeat(next);
        }
        KeyCode::Char('q') => return true,
        _ => {}
    }
    false
}

fn handle_content_focus(app: &mut AppState, key: KeyEvent) -> bool {
    // Route keys to inline filter when active
    if route_view_filter(app, key) {
        return false;
    }

    // When in Search view, letter keys feed the query (not shortcuts)
    if matches!(app.nav.current(), View::Search(_)) {
        match key.code {
            KeyCode::Char(c) => {
                if let View::Search(s) = app.nav.current_mut() {
                    s.query.push(c);
                }
                let query = match app.nav.current() {
                    View::Search(s) => s.query.clone(),
                    _ => String::new(),
                };
                if let View::Search(s) = app.nav.current_mut() {
                    s.run_search_with_query(&query, &app.search_svc);
                }
                return false;
            }
            KeyCode::Backspace => {
                if let View::Search(s) = app.nav.current_mut() {
                    s.query.pop();
                    let q = s.query.clone();
                    s.run_search_with_query(&q, &app.search_svc);
                }
                return false;
            }
            _ => {}
        }
    }

    match key.code {
        // ── Quit / back ──
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.nav.is_root() {
                return true;
            }
            app.nav.pop();
        }

        // ── Help ──
        KeyCode::Char('?') => app.show_help = true,

        // ── Command mode ──
        KeyCode::Char(':') => {
            app.input_mode = InputMode::Command;
            app.command_input.clear();
            app.command_completions = completions::complete("");
        }

        // ── Sidebar navigation (1-5) ──
        KeyCode::Char(c @ '1'..='5') => {
            if let Some(entry) = SidebarEntry::from_digit(c as u8 - b'0') {
                app.navigate_to(entry);
            }
        }

        // ── Cursor movement ──
        KeyCode::Char('j') | KeyCode::Down => move_down(app),
        KeyCode::Char('k') | KeyCode::Up => move_up(app),
        KeyCode::Char('h') | KeyCode::Left if matches!(app.nav.current(), View::Albums(_)) => {
            if let View::Albums(s) = app.nav.current_mut() { s.move_left(); }
        }
        KeyCode::Char('l') | KeyCode::Right if matches!(app.nav.current(), View::Albums(_)) => {
            if let View::Albums(s) = app.nav.current_mut() { s.move_right(); }
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => page_down(app),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => page_up(app),
        KeyCode::Char('g') => jump_top(app),
        KeyCode::Char('G') => jump_bottom(app),

        // ── Enter: open detail or play ──
        KeyCode::Enter => handle_enter(app),

        // ── Playback ──
        KeyCode::Char(' ') => toggle_playback(app),
        KeyCode::Char('n') => { let _ = app.player.next(); }
        KeyCode::Char('N') => { let _ = app.player.previous(); }
        KeyCode::Char(']') => { let _ = app.player.seek(Duration::from_secs(10)); }
        KeyCode::Char('[') => { let _ = app.player.seek(Duration::from_secs_f64((app.player_cache.position - 10.0).max(0.0))); }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            let _ = app.player.set_volume((app.player_cache.volume + 0.05).min(1.0));
        }
        KeyCode::Char('-') => {
            let _ = app.player.set_volume((app.player_cache.volume - 0.05).max(0.0));
        }
        KeyCode::Char('S') => { let _ = app.player.set_shuffle(!app.player_cache.shuffle); }
        KeyCode::Char('R') => {
            let next = match app.player_cache.repeat {
                RepeatMode::Off => RepeatMode::All,
                RepeatMode::All => RepeatMode::One,
                RepeatMode::One => RepeatMode::Off,
            };
            let _ = app.player.set_repeat(next);
        }

        // ── Queue operations ──
        KeyCode::Char('a') => app.add_selected_to_queue(),
        KeyCode::Char('x') => handle_remove_selected(app),
        KeyCode::Char('X') => {
            app.confirm = Some(ConfirmCtx {
                prompt: "Clear entire queue?".to_string(),
                action: ConfirmAction::ClearQueue,
            });
            app.input_mode = InputMode::Confirm;
        }
        KeyCode::Char('J') => handle_move_down(app),
        KeyCode::Char('K') => handle_move_up_item(app),

        // ── Playlist operations ──
        KeyCode::Char('A') => show_playlist_selector(app),
        KeyCode::Char('c') => handle_create_playlist(app),
        KeyCode::Char('r') => handle_rename_playlist(app),
        KeyCode::Char('d') => handle_delete_playlist(app),
        KeyCode::Char('i') => handle_import_m3u(app),

        // ── Library ──
        KeyCode::Char('s') => {
            app.text_input = Some(TextInputCtx {
                prompt: "Scan path:".to_string(),
                value: String::new(),
                action: TextInputAction::ScanPath,
            });
            app.input_mode = InputMode::TextInput;
        }
        KeyCode::Char('/') => handle_filter(app),

        // ── Rating (0-5) ──
        KeyCode::Char(c @ '0'..='5') => {
            app.rate_selected(c as u8 - b'0');
        }

        _ => {}
    }

    false
}

fn toggle_playback(app: &mut AppState) {
    match app.player_cache.state {
        PlaybackState::Playing => { let _ = app.player.pause(); }
        PlaybackState::Paused => { let _ = app.player.resume(); }
        PlaybackState::Stopped => app.play_from_current_view(),
    }
}

fn handle_enter(app: &mut AppState) {
    match app.nav.current() {
        View::Library(_) | View::AlbumDetail(_) | View::GenreDetail(_)
        | View::PlaylistDetail(_) | View::Queue(_) => {
            app.play_from_current_view();
        }
        View::Albums(_) => push_album_detail(app),
        View::Artists(_) => push_artist_detail(app),
        View::Genres(_) => push_genre_detail(app),
        View::Playlists(_) => push_playlist_detail(app),
        View::ArtistDetail(_) => push_album_from_artist(app),
        View::Search(_) => handle_search_enter(app),
        View::Scan(_) => {}
    }
}

fn push_album_detail(app: &mut AppState) {
    let album = match app.nav.current() {
        View::Albums(s) => s.selected_album().cloned(),
        _ => None,
    };
    if let Some(album) = album {
        let state = AlbumDetailState::new(album, &app.library, &mut app.picker);
        app.nav.push(View::AlbumDetail(state));
    }
}

fn push_artist_detail(app: &mut AppState) {
    let artist = match app.nav.current() {
        View::Artists(s) => s.selected_artist().cloned(),
        _ => None,
    };
    if let Some(artist) = artist {
        let photo_dir = app.paths.artist_photo_dir();
        let state = ArtistDetailState::new(artist, &app.library, &mut app.picker, &photo_dir);
        app.nav.push(View::ArtistDetail(state));
    }
}

fn push_genre_detail(app: &mut AppState) {
    let genre = match app.nav.current() {
        View::Genres(s) => s.selected_genre().cloned().map(|(g, _, _)| g),
        _ => None,
    };
    if let Some(genre) = genre {
        app.nav.push(View::GenreDetail(GenreDetailState::new(genre, &app.library)));
    }
}

fn push_playlist_detail(app: &mut AppState) {
    let playlist = match app.nav.current() {
        View::Playlists(s) => s.selected_playlist().cloned(),
        _ => None,
    };
    if let Some(playlist) = playlist {
        app.nav.push(View::PlaylistDetail(PlaylistDetailState::new(playlist, &app.library)));
    }
}

fn push_album_from_artist(app: &mut AppState) {
    let album = match app.nav.current() {
        View::ArtistDetail(s) => s.selected_album().cloned(),
        _ => None,
    };
    if let Some(album) = album {
        let state = AlbumDetailState::new(album, &app.library, &mut app.picker);
        app.nav.push(View::AlbumDetail(state));
    }
}

fn handle_search_enter(app: &mut AppState) {
    use crate::views::search::SearchSection;
    let (section, album, artist) = match app.nav.current() {
        View::Search(s) => (s.section, s.selected_album().cloned(), s.selected_artist().cloned()),
        _ => return,
    };
    match section {
        SearchSection::Tracks => app.play_from_current_view(),
        SearchSection::Albums => {
            if let Some(album) = album {
                let state = AlbumDetailState::new(album, &app.library, &mut app.picker);
                app.nav.push(View::AlbumDetail(state));
            }
        }
        SearchSection::Artists => {
            if let Some(artist) = artist {
                let photo_dir = app.paths.artist_photo_dir();
                let state = ArtistDetailState::new(artist, &app.library, &mut app.picker, &photo_dir);
                app.nav.push(View::ArtistDetail(state));
            }
        }
    }
}

fn handle_filter(app: &mut AppState) {
    match app.nav.current_mut() {
        View::Library(s) => s.filter_active = true,
        View::Artists(s) => s.filter_active = true,
        View::Albums(s) => s.filter_active = true,
        View::Genres(s) => s.filter_active = true,
        _ => {}
    }
}

/// Route keyboard input to the inline filter of the current view.
/// Returns true if the key was consumed.
fn route_view_filter(app: &mut AppState, key: KeyEvent) -> bool {
    let is_active = match app.nav.current() {
        View::Library(s) => s.filter_active,
        View::Artists(s) => s.filter_active,
        View::Albums(s)  => s.filter_active,
        View::Genres(s)  => s.filter_active,
        _ => false,
    };
    if !is_active { return false; }

    // Genres use client-side filter; Library/Artists/Albums use DB search.
    let is_db_search = matches!(app.nav.current(), View::Library(_) | View::Artists(_) | View::Albums(_));

    match key.code {
        KeyCode::Esc => {
            match app.nav.current_mut() {
                View::Library(s) => { s.filter_active = false; s.filter.clear(); }
                View::Artists(s) => { s.filter_active = false; s.filter.clear(); }
                View::Albums(s)  => { s.filter_active = false; s.filter.clear(); }
                View::Genres(s)  => { s.filter_active = false; s.filter.clear(); }
                _ => {}
            }
        }
        KeyCode::Enter => {
            match app.nav.current_mut() {
                View::Library(s) => s.filter_active = false,
                View::Artists(s) => s.filter_active = false,
                View::Albums(s)  => s.filter_active = false,
                View::Genres(s)  => s.filter_active = false,
                _ => {}
            }
        }
        KeyCode::Backspace => {
            match app.nav.current_mut() {
                View::Library(s) => { s.filter.pop(); }
                View::Artists(s) => { s.filter.pop(); }
                View::Albums(s)  => { s.filter.pop(); }
                View::Genres(s)  => { s.filter.pop(); }
                _ => {}
            }
        }
        KeyCode::Char(c) => {
            match app.nav.current_mut() {
                View::Library(s) => { s.filter.push(c); }
                View::Artists(s) => { s.filter.push(c); }
                View::Albums(s)  => { s.filter.push(c); }
                View::Genres(s)  => { s.filter.push(c); }
                _ => {}
            }
        }
        _ => {}
    }

    if is_db_search {
        app.apply_view_search();
    }
    true
}

// ── Queue / playlist item movement ──────────────────────────────────────────

fn handle_remove_selected(app: &mut AppState) {
    match app.nav.current() {
        View::Queue(s) => {
            if let Some(pos) = s.list_state.selected() {
                app.confirm = Some(ConfirmCtx {
                    prompt: "Remove track from queue?".to_string(),
                    action: ConfirmAction::RemoveFromQueue { position: pos },
                });
                app.input_mode = InputMode::Confirm;
            }
        }
        View::PlaylistDetail(s) => {
            if let Some(pos) = s.list_state.selected() {
                let pid = s.playlist.id;
                app.confirm = Some(ConfirmCtx {
                    prompt: "Remove track from playlist?".to_string(),
                    action: ConfirmAction::RemoveFromPlaylist { playlist_id: pid, position: pos },
                });
                app.input_mode = InputMode::Confirm;
            }
        }
        _ => {}
    }
}

fn handle_move_down(app: &mut AppState) {
    match app.nav.current() {
        View::Queue(s) => {
            if let Some(pos) = s.list_state.selected() {
                let next = pos + 1;
                if next < app.player_cache.queue.len() {
                    let _ = app.player.set_queue({
                        let mut q = app.player_cache.queue.clone();
                        q.swap(pos, next);
                        q
                    });
                    if let View::Queue(s) = app.nav.current_mut() {
                        s.list_state.select(Some(next));
                    }
                }
            }
        }
        View::PlaylistDetail(s) => {
            if let Some(pos) = s.list_state.selected() {
                let pid = s.playlist.id;
                let len = s.tracks.len();
                if pos + 1 < len {
                    let _ = app.playlists.move_track(pid, pos, pos + 1);
                    reload_playlist_detail(app, pid);
                    if let View::PlaylistDetail(s) = app.nav.current_mut() {
                        s.list_state.select(Some(pos + 1));
                    }
                }
            }
        }
        _ => {}
    }
}

fn handle_move_up_item(app: &mut AppState) {
    match app.nav.current() {
        View::Queue(s) => {
            if let Some(pos) = s.list_state.selected() {
                if pos > 0 {
                    let prev = pos - 1;
                    let _ = app.player.set_queue({
                        let mut q = app.player_cache.queue.clone();
                        q.swap(pos, prev);
                        q
                    });
                    if let View::Queue(s) = app.nav.current_mut() {
                        s.list_state.select(Some(prev));
                    }
                }
            }
        }
        View::PlaylistDetail(s) => {
            if let Some(pos) = s.list_state.selected() {
                if pos > 0 {
                    let pid = s.playlist.id;
                    let _ = app.playlists.move_track(pid, pos, pos - 1);
                    reload_playlist_detail(app, pid);
                    if let View::PlaylistDetail(s) = app.nav.current_mut() {
                        s.list_state.select(Some(pos - 1));
                    }
                }
            }
        }
        _ => {}
    }
}

fn reload_playlist_detail(app: &mut AppState, playlist_id: i64) {
    if let Ok(Some(pl)) = app.playlists.get_playlist(playlist_id) {
        if let View::PlaylistDetail(s) = app.nav.current_mut() {
            let sel = s.list_state.selected();
            *s = PlaylistDetailState::new(pl, &app.library);
            s.list_state.select(sel);
        }
    }
}

// ── Playlist management ──────────────────────────────────────────────────────

fn show_playlist_selector(app: &mut AppState) {
    if app.selected_track_id().is_some() {
        app.show_playlist_selector = true;
        app.playlist_selector_state = ratatui::widgets::ListState::default();
        // Pre-load playlists
        if let Ok(pls) = app.playlists.list_playlists() {
            if !pls.is_empty() {
                app.playlist_selector_state.select(Some(0));
            }
            let _ = pls; // playlists re-queried during render via AppState
        }
    }
}

fn handle_create_playlist(app: &mut AppState) {
    if matches!(app.nav.current(), View::Playlists(_)) {
        app.text_input = Some(TextInputCtx {
            prompt: "Playlist name:".to_string(),
            value: String::new(),
            action: TextInputAction::CreatePlaylist,
        });
        app.input_mode = InputMode::TextInput;
    }
}

fn handle_add_random(app: &mut AppState) {
    match app.library.get_random_tracks(30) {
        Ok(ids) if !ids.is_empty() => {
            match app.player.add_to_queue(ids) {
                Ok(_) => app.set_status("Added 30 random tracks", StatusKind::Success),
                Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
        Ok(_) => app.set_status("No tracks in library", StatusKind::Info),
        Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
    }
}

fn handle_save_queue_as_playlist(app: &mut AppState) {
    if app.player.get_queue().is_empty() {
        app.set_status("Queue is empty", StatusKind::Info);
        return;
    }
    app.text_input = Some(TextInputCtx {
        prompt: "Save queue as playlist:".to_string(),
        value: String::new(),
        action: TextInputAction::SaveQueueAsPlaylist,
    });
    app.input_mode = InputMode::TextInput;
}

fn handle_clear_queue_confirm(app: &mut AppState) {
    if app.player.get_queue().is_empty() {
        app.set_status("Queue is already empty", StatusKind::Info);
        return;
    }
    app.confirm = Some(ConfirmCtx {
        prompt: "Clear entire queue? (y/n)".to_string(),
        action: ConfirmAction::ClearQueue,
    });
    app.input_mode = InputMode::Confirm;
}

fn handle_rename_playlist(app: &mut AppState) {
    let id_and_name = match app.nav.current() {
        View::Playlists(s) => s.selected_playlist().map(|p| (p.id, p.name.clone())),
        _ => None,
    };
    if let Some((id, name)) = id_and_name {
        app.text_input = Some(TextInputCtx {
            prompt: "New name:".to_string(),
            value: name,
            action: TextInputAction::RenamePlaylist { id },
        });
        app.input_mode = InputMode::TextInput;
    }
}

fn handle_delete_playlist(app: &mut AppState) {
    let id_and_name = match app.nav.current() {
        View::Playlists(s) => s.selected_playlist().map(|p| (p.id, p.name.clone())),
        _ => None,
    };
    if let Some((id, name)) = id_and_name {
        app.confirm = Some(ConfirmCtx {
            prompt: format!("Delete playlist \"{}\"?", name),
            action: ConfirmAction::DeletePlaylist { id },
        });
        app.input_mode = InputMode::Confirm;
    }
}

fn handle_import_m3u(app: &mut AppState) {
    if matches!(app.nav.current(), View::Playlists(_)) {
        app.text_input = Some(TextInputCtx {
            prompt: "M3U file path:".to_string(),
            value: String::new(),
            action: TextInputAction::ImportM3u,
        });
        app.input_mode = InputMode::TextInput;
    }
}

// ── Playlist selector overlay ────────────────────────────────────────────────

fn handle_playlist_selector(app: &mut AppState, key: KeyEvent) -> bool {
    let playlists = app.playlists.list_playlists().unwrap_or_default();
    let len = playlists.len();

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.show_playlist_selector = false,
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                let n = app.playlist_selector_state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
                app.playlist_selector_state.select(Some(n));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let p = app.playlist_selector_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
            app.playlist_selector_state.select(Some(p));
        }
        KeyCode::Enter => {
            if let Some(idx) = app.playlist_selector_state.selected() {
                if let Some(pl) = playlists.get(idx) {
                    let pid = pl.id;
                    if let Some(track_id) = app.selected_track_id() {
                        match app.playlists.add_tracks(pid, vec![track_id]) {
                            Ok(_) => app.set_status("Added to playlist", StatusKind::Success),
                            Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
                        }
                    }
                }
            }
            app.show_playlist_selector = false;
        }
        _ => {}
    }
    false
}

// ── Command mode ─────────────────────────────────────────────────────────────

fn handle_command_mode(app: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.command_input.clear();
            app.command_completions.clear();
        }
        KeyCode::Enter => {
            let input = app.command_input.clone();
            app.input_mode = InputMode::Normal;
            app.command_input.clear();
            app.command_completions.clear();
            execute_command(app, &input);
        }
        KeyCode::Backspace => {
            app.command_input.pop();
            app.command_completions = completions::complete(&app.command_input);
        }
        KeyCode::Tab => {
            // Pick first completion
            if let Some(first) = app.command_completions.first().cloned() {
                app.command_input = first;
                app.command_completions = completions::complete(&app.command_input);
            }
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
            app.command_completions = completions::complete(&app.command_input);
        }
        _ => {}
    }
}

fn execute_command(app: &mut AppState, input: &str) {
    match Command::parse(input) {
        Command::Scan { path } => start_scan(app, path),
        Command::Cleanup => {
            app.set_status("Cleanup not available in this version", StatusKind::Info);
        }
        Command::Rate { stars } => app.rate_selected(stars),
        Command::QueueAdd => app.add_selected_to_queue(),
        Command::QueueClear => {
            app.confirm = Some(ConfirmCtx {
                prompt: "Clear entire queue?".to_string(),
                action: ConfirmAction::ClearQueue,
            });
            app.input_mode = InputMode::Confirm;
        }
        Command::PlaylistCreate { name } => {
            match app.playlists.create_playlist(&name, None) {
                Ok(_) => {
                    app.set_status(format!("Created playlist \"{}\"", name), StatusKind::Success);
                    app.reload_current_view();
                }
                Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
        Command::PlaylistDelete { name } => {
            let id = app.playlists.list_playlists().ok()
                .and_then(|pls| pls.into_iter().find(|p| p.name == name).map(|p| p.id));
            if let Some(id) = id {
                app.confirm = Some(ConfirmCtx {
                    prompt: format!("Delete playlist \"{}\"?", name),
                    action: ConfirmAction::DeletePlaylist { id },
                });
                app.input_mode = InputMode::Confirm;
            } else {
                app.set_status(format!("Playlist not found: {}", name), StatusKind::Error);
            }
        }
        Command::PlaylistAdd { name } => {
            let id = app.playlists.list_playlists().ok()
                .and_then(|pls| pls.into_iter().find(|p| p.name == name).map(|p| p.id));
            if let Some(pid) = id {
                if let Some(track_id) = app.selected_track_id() {
                    match app.playlists.add_tracks(pid, vec![track_id]) {
                        Ok(_) => app.set_status(format!("Added to \"{}\"", name), StatusKind::Success),
                        Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
                    }
                }
            } else {
                app.set_status(format!("Playlist not found: {}", name), StatusKind::Error);
            }
        }
        Command::Import { path } => {
            match app.playlists.import_m3u(&path) {
                Ok(pl) => {
                    app.set_status(format!("Imported \"{}\"", pl.name), StatusKind::Success);
                    app.reload_current_view();
                }
                Err(e) => app.set_status(format!("Import error: {}", e), StatusKind::Error),
            }
        }
        Command::Seek { position_str } => {
            if let Some(dur) = parse_seek_position(&position_str) {
                let _ = app.player.seek(dur);
            } else {
                app.set_status("Invalid seek position (use mm:ss)", StatusKind::Error);
            }
        }
        Command::Help => app.show_help = true,
        Command::Navigate(entry) => app.navigate_to(entry),
        Command::Unknown(msg) => {
            if !msg.is_empty() {
                app.set_status(msg, StatusKind::Error);
            }
        }
    }
}

fn start_scan(app: &mut AppState, path: std::path::PathBuf) {
    let path = expand_tilde(&path.to_string_lossy());
    match app.library.add_source("Music", &path) {
        Ok(source) => {
            match app.library.scan_directory(&path, source.id) {
                Ok(result) => {
                    app.set_status(
                        format!("Scan complete: {} tracks added", result.tracks_added),
                        StatusKind::Success,
                    );
                    app.reload_current_view();
                }
                Err(e) => app.set_status(format!("Scan error: {}", e), StatusKind::Error),
            }
        }
        Err(e) => app.set_status(format!("Source error: {}", e), StatusKind::Error),
    }
}

// ── Text input mode ──────────────────────────────────────────────────────────

fn handle_text_input(app: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.text_input = None;
        }
        KeyCode::Enter => {
            if let Some(ctx) = app.text_input.take() {
                app.input_mode = InputMode::Normal;
                execute_text_input(app, ctx.action, ctx.value);
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut ctx) = app.text_input {
                ctx.value.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut ctx) = app.text_input {
                ctx.value.push(c);
            }
        }
        _ => {}
    }
}

fn execute_text_input(app: &mut AppState, action: TextInputAction, value: String) {
    match action {
        TextInputAction::ScanPath => {
            let path = expand_tilde(&value);
            start_scan(app, path);
        }
        TextInputAction::CreatePlaylist => {
            if !value.is_empty() {
                match app.playlists.create_playlist(&value, None) {
                    Ok(_) => {
                        app.set_status(format!("Created \"{}\"", value), StatusKind::Success);
                        app.reload_current_view();
                    }
                    Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
                }
            }
        }
        TextInputAction::RenamePlaylist { id } => {
            if !value.is_empty() {
                match app.playlists.rename_playlist(id, &value) {
                    Ok(_) => {
                        app.set_status(format!("Renamed to \"{}\"", value), StatusKind::Success);
                        app.reload_current_view();
                    }
                    Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
                }
            }
        }
        TextInputAction::ImportM3u => {
            let path = expand_tilde(&value);
            match app.playlists.import_m3u(&path) {
                Ok(pl) => {
                    app.set_status(format!("Imported \"{}\"", pl.name), StatusKind::Success);
                    app.reload_current_view();
                }
                Err(e) => app.set_status(format!("Import error: {}", e), StatusKind::Error),
            }
        }
        TextInputAction::SaveQueueAsPlaylist => {
            if !value.is_empty() {
                let track_ids = app.player.get_queue();
                if track_ids.is_empty() {
                    app.set_status("Queue is empty", StatusKind::Info);
                    return;
                }
                match app.playlists.create_playlist(&value, None) {
                    Ok(pl) => match app.playlists.add_tracks(pl.id, track_ids) {
                        Ok(_) => {
                            app.set_status(
                                format!("Saved queue to \"{}\"", value),
                                StatusKind::Success,
                            );
                            app.reload_current_view();
                        }
                        Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
                    },
                    Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
                }
            }
        }
    }
}

// ── Confirm mode ─────────────────────────────────────────────────────────────

fn handle_confirm(app: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(ctx) = app.confirm.take() {
                app.input_mode = InputMode::Normal;
                execute_confirm(app, ctx.action);
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.confirm = None;
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

fn execute_confirm(app: &mut AppState, action: ConfirmAction) {
    match action {
        ConfirmAction::ClearQueue => {
            match app.player.clear_queue() {
                Ok(_) => app.set_status("Queue cleared", StatusKind::Success),
                Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
        ConfirmAction::DeletePlaylist { id } => {
            match app.playlists.delete_playlist(id) {
                Ok(_) => {
                    app.set_status("Playlist deleted", StatusKind::Success);
                    // Pop if we're in the detail view
                    if matches!(app.nav.current(), View::PlaylistDetail(_)) {
                        app.nav.pop();
                    }
                    app.reload_current_view();
                }
                Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
        ConfirmAction::RemoveFromQueue { position } => {
            let mut queue = app.player_cache.queue.clone();
            if position < queue.len() {
                queue.remove(position);
                match app.player.set_queue(queue) {
                    Ok(_) => app.set_status("Removed from queue", StatusKind::Success),
                    Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
                }
            }
        }
        ConfirmAction::RemoveFromPlaylist { playlist_id, position } => {
            match app.playlists.remove_track(playlist_id, position) {
                Ok(_) => {
                    app.set_status("Removed from playlist", StatusKind::Success);
                    reload_playlist_detail(app, playlist_id);
                }
                Err(e) => app.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
        ConfirmAction::LibraryCleanup => {
            app.set_status("Cleanup not available in this version", StatusKind::Info);
        }
    }
}

// ── Cursor helpers ────────────────────────────────────────────────────────────

fn move_down(app: &mut AppState) {
    match app.nav.current_mut() {
        View::Library(s) => s.move_down(),
        View::Albums(s) => s.move_down(),
        View::Artists(s) => s.move_down(),
        View::Genres(s) => s.move_down(),
        View::Playlists(s) => s.move_down(),
        View::AlbumDetail(s) => s.move_down(),
        View::ArtistDetail(s) => s.move_down(),
        View::GenreDetail(s) => s.move_down(),
        View::PlaylistDetail(s) => s.move_down(),
        View::Queue(s) => {
            let len = app.player_cache.queue.len();
            s.move_down(len);
        }
        View::Search(s) => s.move_down(),
        View::Scan(_) => {}
    }
}

fn move_up(app: &mut AppState) {
    match app.nav.current_mut() {
        View::Library(s) => s.move_up(),
        View::Albums(s) => s.move_up(),
        View::Artists(s) => s.move_up(),
        View::Genres(s) => s.move_up(),
        View::Playlists(s) => s.move_up(),
        View::AlbumDetail(s) => s.move_up(),
        View::ArtistDetail(s) => s.move_up(),
        View::GenreDetail(s) => s.move_up(),
        View::PlaylistDetail(s) => s.move_up(),
        View::Queue(s) => s.move_up(),
        View::Search(s) => s.move_up(),
        View::Scan(_) => {}
    }
}

fn page_down(app: &mut AppState) {
    match app.nav.current_mut() {
        View::Library(s) => s.page_down(),
        View::Albums(s) => s.page_down(),
        View::Artists(s) => s.page_down(),
        View::GenreDetail(s) => { let n = s.list_state.selected().map(|i| (i + 10).min(s.tracks.len().saturating_sub(1))).unwrap_or(0); s.list_state.select(Some(n)); }
        View::AlbumDetail(s) => {
            let len = s.tracks.len();
            if len > 0 {
                let n = s.list_state.selected().map(|i| (i + 10).min(len - 1)).unwrap_or(0);
                s.list_state.select(Some(n));
            }
        }
        View::Queue(s) => {
            let len = app.player_cache.queue.len();
            s.move_down(len.saturating_sub(1).min(
                s.list_state.selected().map(|i| i + 10).unwrap_or(10)
            ));
        }
        _ => {}
    }
}

fn page_up(app: &mut AppState) {
    match app.nav.current_mut() {
        View::Library(s) => s.page_up(),
        View::Albums(s) => s.page_up(),
        View::Artists(s) => s.page_up(),
        View::GenreDetail(s) => { let p = s.list_state.selected().map(|i| i.saturating_sub(10)).unwrap_or(0); s.list_state.select(Some(p)); }
        View::AlbumDetail(s) => {
            let p = s.list_state.selected().map(|i| i.saturating_sub(10)).unwrap_or(0);
            s.list_state.select(Some(p));
        }
        View::Queue(s) => {
            let p = s.list_state.selected().map(|i| i.saturating_sub(10)).unwrap_or(0);
            s.list_state.select(Some(p));
        }
        _ => {}
    }
}

fn jump_top(app: &mut AppState) {
    match app.nav.current_mut() {
        View::Library(s) => s.jump_top(),
        View::Albums(s) => s.jump_top(),
        View::Artists(s) => s.jump_top(),
        View::Genres(s) => s.jump_top(),
        View::Playlists(s) => s.jump_top(),
        View::AlbumDetail(s) => s.jump_top(),
        View::ArtistDetail(s) => s.jump_top(),
        View::GenreDetail(s) => s.jump_top(),
        View::PlaylistDetail(s) => s.jump_top(),
        View::Queue(s) => s.jump_top(),
        _ => {}
    }
}

fn jump_bottom(app: &mut AppState) {
    match app.nav.current_mut() {
        View::Library(s) => s.jump_bottom(),
        View::Albums(s) => s.jump_bottom(),
        View::Artists(s) => s.jump_bottom(),
        View::Genres(s) => s.jump_bottom(),
        View::Playlists(s) => s.jump_bottom(),
        View::AlbumDetail(s) => s.jump_bottom(),
        View::ArtistDetail(s) => s.jump_bottom(),
        View::GenreDetail(s) => s.jump_bottom(),
        View::PlaylistDetail(s) => s.jump_bottom(),
        View::Queue(s) => {
            let len = app.player_cache.queue.len();
            s.jump_bottom(len);
        }
        _ => {}
    }
}
