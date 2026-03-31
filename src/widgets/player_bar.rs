use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, Gauge, Paragraph}};
use tornade_core::models::RepeatMode;
use tornade_core::services::PlaybackState;
use crate::player::PlayerStateCache;
use crate::utils::{format_duration, truncate};

/// Compact stacked player for the right panel (no horizontal split).
pub fn render_compact(frame: &mut Frame, area: Rect, cache: &PlayerStateCache) {
    let block = Block::default().borders(Borders::ALL).title(" Now Playing ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // state icon + title
            Constraint::Length(1), // artist
            Constraint::Length(1), // progress gauge
            Constraint::Min(0),    // shuffle / repeat / vol
        ])
        .split(inner);

    let max_title = (inner.width as usize).saturating_sub(3); // icon(1) + space(1) + 1
    let (title, artist) = match &cache.current_track {
        Some(t) => (
            truncate(&t.title, max_title),
            t.artist_names.first().cloned().unwrap_or_default(),
        ),
        None => ("Nothing playing".to_string(), String::new()),
    };

    let state_icon = match cache.state {
        PlaybackState::Playing => "▶",
        PlaybackState::Paused => "⏸",
        PlaybackState::Stopped => "⏹",
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(state_icon, Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        ])),
        rows[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {}", truncate(&artist, (inner.width as usize).saturating_sub(2))),
            Style::default().fg(Color::Gray),
        ))),
        rows[1],
    );

    let total_secs = cache.current_track.as_ref().map(|t| t.duration.as_secs_f64()).unwrap_or(1.0);
    let ratio = if total_secs > 0.0 { (cache.position / total_secs).min(1.0) } else { 0.0 };
    let elapsed = format_duration(cache.position as u64);
    let total = format_duration(total_secs as u64);

    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Cyan))
            .ratio(ratio)
            .label(format!("{}/{}", elapsed, total)),
        rows[2],
    );

    if rows.len() > 3 && rows[3].height > 0 {
        let shuffle_str = if cache.shuffle { "S" } else { " " };
        let repeat_str = match cache.repeat {
            RepeatMode::Off => " ",
            RepeatMode::All => "↺",
            RepeatMode::One => "↺1",
        };
        let vol_pct = (cache.volume * 100.0) as u8;

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    shuffle_str,
                    if cache.shuffle { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) },
                ),
                Span::raw(" "),
                Span::styled(
                    repeat_str,
                    if cache.repeat != RepeatMode::Off { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) },
                ),
                Span::raw("  "),
                Span::styled(format!("Vol {:>3}%", vol_pct), Style::default().fg(Color::Gray)),
            ])),
            rows[3],
        );
    }
}

