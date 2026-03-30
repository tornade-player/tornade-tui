use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, Gauge, Paragraph}};
use tornade_core::models::RepeatMode;
use crate::player::PlayerStateCache;
use crate::utils::{format_duration, truncate};

pub fn render(frame: &mut Frame, area: Rect, cache: &PlayerStateCache) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(35),
            Constraint::Percentage(20),
        ])
        .split(area);

    // Left: track info
    let (title, artist) = match &cache.current_track {
        Some(t) => (
            truncate(&t.title, 35),
            t.artist_names.first().cloned().unwrap_or_default(),
        ),
        None => ("Nothing playing".to_string(), String::new()),
    };

    let state_icon = match cache.state {
        tornade_core::services::PlaybackState::Playing => "▶",
        tornade_core::services::PlaybackState::Paused => "⏸",
        tornade_core::services::PlaybackState::Stopped => "⏹",
    };

    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(state_icon, Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled(truncate(&artist, 40), Style::default().fg(Color::Gray))),
    ])
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(info, chunks[0]);

    // Center: progress bar
    let total_secs = cache.current_track.as_ref().map(|t| t.duration.as_secs_f64()).unwrap_or(1.0);
    let ratio = if total_secs > 0.0 { (cache.position / total_secs).min(1.0) } else { 0.0 };
    let elapsed = format_duration(cache.position as u64);
    let total = format_duration(total_secs as u64);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(format!(" {} / {} ", elapsed, total)))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio);
    frame.render_widget(gauge, chunks[1]);

    // Right: volume + modes
    let shuffle_str = if cache.shuffle { " S " } else { "   " };
    let repeat_str = match cache.repeat {
        RepeatMode::Off => "   ",
        RepeatMode::All => " ↺ ",
        RepeatMode::One => "↺1 ",
    };
    let vol_pct = (cache.volume * 100.0) as u8;
    let controls = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(shuffle_str, if cache.shuffle { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(repeat_str, if cache.repeat != RepeatMode::Off { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(format!("Vol {:>3}%", vol_pct), Style::default().fg(Color::Gray)),
        ]),
        Line::from(Span::styled("[Space] play/pause  [n/N] next/prev  [+/-] vol", Style::default().fg(Color::DarkGray))),
    ])
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(controls, chunks[2]);
}
