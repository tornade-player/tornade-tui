// UI rendering with ratatui
// Layout: header (stats), main (track list), footer (status bar)

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, View};

pub fn draw(f: &mut Frame, app: &App) {
    // Main layout: [Header, Body, Footer]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Body
            Constraint::Length(3), // Footer
        ])
        .split(f.size());

    draw_header(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.search_mode {
        format!("🎵 Tornade TUI - Search: {}_", app.search_query)
    } else {
        match app.view {
            View::Library => String::from("🎵 Tornade TUI - Library"),
            View::Albums => String::from("🎵 Tornade TUI - Albums"),
            View::Search => String::from("🎵 Tornade TUI - Search Results"),
        }
    };

    let stats_text = format!(
        " {} tracks │ {} albums │ {} artists ",
        app.track_count, app.album_count, app.artist_count
    );

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(stats_text, Style::default().fg(Color::Gray))),
    ])
    .block(Block::default().borders(Borders::BOTTOM))
    .alignment(Alignment::Center);

    f.render_widget(header, area);
}

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    if app.tracks.is_empty() {
        // Empty state
        let empty_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No tracks loaded",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Scan a music library using the Rust API or add tracks via CLI",
                Style::default().fg(Color::Gray),
            )),
        ])
        .alignment(Alignment::Center);

        f.render_widget(empty_msg, area);
        return;
    }

    // Track list
    let items: Vec<ListItem> = app
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let duration_secs = track.duration.as_secs();
            let minutes = duration_secs / 60;
            let seconds = duration_secs % 60;
            let duration_str = format!("{minutes}:{seconds:02}");

            let format_str = match track.file_type {
                tornade_core::models::AudioFormat::Flac => "FLAC",
                tornade_core::models::AudioFormat::Mp3 => "MP3",
                tornade_core::models::AudioFormat::Aac => "AAC",
                tornade_core::models::AudioFormat::Alac => "ALAC",
            };

            let format_info = format!(
                "{} {} {}kHz",
                format_str,
                track
                    .bit_depth
                    .map(|d| format!("{d}bit"))
                    .unwrap_or_default(),
                track.sample_rate.unwrap_or(0) / 1000
            );

            let content = format!(
                "{:3}. {} - Artist #{} [{}] {}",
                i + 1,
                track.title,
                track.artist_id,
                duration_str,
                format_info
            );

            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Tracks ({}) ", app.tracks.len())),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(list, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let help_text = if app.search_mode {
        "ESC: Cancel │ ENTER: Search"
    } else {
        "↑/↓: Navigate │ ENTER/SPACE: Play │ /: Search │ Q: Quit"
    };

    let footer = Paragraph::new(vec![
        Line::from(Span::styled(
            &app.status_message,
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(Block::default().borders(Borders::TOP));

    f.render_widget(footer, area);
}
