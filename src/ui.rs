use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::{
    app::{AppState, InputMode, StatusKind},
    views::View,
    widgets::{command_bar, confirm_dialog, help_overlay, input_dialog, player_bar, sidebar},
};

pub fn draw(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    // Main layout: body (fills remaining) + player bar at bottom
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(4)])
        .split(area);

    let body_area = main_chunks[0];
    let player_area = main_chunks[1];

    // Body layout: sidebar (fixed) + content
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(0)])
        .split(body_area);

    let sidebar_area = body_chunks[0];
    let content_area = body_chunks[1];

    // 1. Sidebar
    sidebar::render(frame, sidebar_area, app.nav.current().sidebar_entry());

    // 2. Content area (with optional status bar at bottom)
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(content_area);

    let view_area = content_chunks[0];
    let status_area = content_chunks[1];

    // Render current view
    render_view(frame, app, view_area);

    // Status bar
    render_status(frame, app, status_area);

    // 3. Player bar
    player_bar::render(frame, player_area, &app.player_cache);

    // 4. Overlays (rendered on top)
    render_overlays(frame, app, area);
}

fn render_view(frame: &mut Frame, app: &mut AppState, area: Rect) {
    match app.nav.current_mut() {
        View::Library(s) => s.render(frame, area),
        View::Albums(s) => s.render(frame, area),
        View::Artists(s) => s.render(frame, area),
        View::Genres(s) => s.render(frame, area),
        View::Playlists(s) => s.render(frame, area),
        View::AlbumDetail(s) => s.render(frame, area),
        View::ArtistDetail(s) => s.render(frame, area),
        View::GenreDetail(s) => s.render(frame, area),
        View::PlaylistDetail(s) => s.render(frame, area),
        View::Scan(s) => s.render(frame, area),
        View::Search(s) => s.render(frame, area),
        View::Queue(s) => {
            // Queue view needs access to player_cache and library; resolve tracks here
            let _ = s; // borrow ends
            render_queue_view(frame, app, area);
        }
    }
}

fn render_queue_view(frame: &mut Frame, app: &mut AppState, area: Rect) {
    // Resolve queue track IDs to Track objects for display
    let tracks: Vec<tornade_core::models::Track> = app.player_cache.queue.iter()
        .filter_map(|&id| app.library.get_track(id).ok().flatten())
        .collect();
    let active_index = app.player_cache.queue_index;
    let skipped = app.player_cache.skipped_track_ids.clone();

    if let View::Queue(s) = app.nav.current_mut() {
        s.sync_selection(tracks.len(), active_index);
        s.render(frame, area, &tracks, active_index, &skipped);
    }
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
    if app.input_mode == InputMode::TextInput {
        if let Some(ref ctx) = app.text_input {
            input_dialog::render(frame, &ctx.prompt, &ctx.value);
        }
    }

    // Confirm dialog
    if app.input_mode == InputMode::Confirm {
        if let Some(ref ctx) = app.confirm {
            confirm_dialog::render(frame, &ctx.prompt);
        }
    }

    // Playlist selector overlay
    if app.show_playlist_selector {
        render_playlist_selector(frame, app, area);
    }

    // Help overlay (always on top)
    if app.show_help {
        help_overlay::render(frame);
    }
}

fn render_playlist_selector(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let playlists = app.playlists.list_playlists().unwrap_or_default();
    let popup_area = centered_rect(50, 50, area);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = playlists.iter().map(|p| {
        ListItem::new(Line::from(Span::raw(p.name.as_str())))
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Add to Playlist  [Enter select  ESC cancel] ")
            .border_style(Style::default().fg(Color::Cyan)))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, popup_area, &mut app.playlist_selector_state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let height = r.height * percent_y / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: width.min(r.width), height: height.min(r.height) }
}
