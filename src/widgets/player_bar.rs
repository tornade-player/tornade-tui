use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Gauge, Paragraph},
};
use ratatui_image::{StatefulImage, protocol::StatefulProtocol};
use tornade_core::models::AudioFormat;
use tornade_core::services::PlaybackState;
use crate::player::PlayerStateCache;
use crate::utils::{format_duration, truncate};

/// Full player panel: artwork | info | volume (top) + controls + progress (bottom).
pub fn render(
    frame: &mut Frame,
    area: Rect,
    cache: &PlayerStateCache,
    image_state: Option<&mut StatefulProtocol>,
    album_name: Option<&str>,
) {
    if area.height < 3 {
        return;
    }

    // Vertical: [top: artwork+info+vol] | [controls 1] | [progress 1]
    let vert = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    // Top horizontal: [artwork 10] | [info] | [volume 7]
    let top = Layout::horizontal([
        Constraint::Length(10),
        Constraint::Min(0),
        Constraint::Length(7),
    ])
    .split(vert[0]);

    // Artwork
    if let Some(proto) = image_state {
        frame.render_stateful_widget(StatefulImage::new(), top[0], proto);
    } else {
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(35, 37, 48))),
            top[0],
        );
    }

    // Track info
    let w = top[1].width as usize;
    let (title, album, fmt, size) = match &cache.current_track {
        Some(t) => {
            let album = album_name.unwrap_or("").to_string();
            let (f, s) = format_audio_info(t);
            (truncate(&t.title, w), truncate(&album, w), f, s)
        }
        None => (
            "Nothing playing".into(),
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    let artist = cache
        .current_track
        .as_ref()
        .and_then(|t| t.artist_names.first().cloned())
        .unwrap_or_default();

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(album, Style::default().fg(Color::Gray))),
            Line::from(Span::styled(
                truncate(&artist, w),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(fmt, Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled(size, Style::default().fg(Color::DarkGray))),
        ]),
        top[1],
    );

    // Volume knob
    let vol_pct = (cache.volume * 100.0).round() as u8;
    render_volume_knob(frame, top[2], vol_pct);

    // Controls: ◀◀  ▶/⏸  ▶▶
    let play_icon = match cache.state {
        PlaybackState::Playing => "⏸",
        _ => "▶",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("◀◀ ", Style::default().fg(Color::Gray)),
            Span::styled(
                play_icon,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ▶▶", Style::default().fg(Color::Gray)),
        ])),
        vert[1],
    );

    // Progress bar
    let total_secs = cache
        .current_track
        .as_ref()
        .map(|t| t.duration.as_secs_f64())
        .unwrap_or(1.0);
    let ratio = if total_secs > 0.0 {
        (cache.position / total_secs).min(1.0)
    } else {
        0.0
    };
    let elapsed = format_duration(cache.position as u64);
    let total_dur = format_duration(total_secs as u64);
    frame.render_widget(
        Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::Rgb(50, 52, 64)),
            )
            .ratio(ratio)
            .label(format!("{}  {}", elapsed, total_dur)),
        vert[2],
    );
}

fn render_volume_knob(frame: &mut Frame, area: Rect, pct: u8) {
    if area.height < 4 {
        return;
    }
    let top_pad = area.height.saturating_sub(4) / 2;
    let knob = Rect {
        x: area.x,
        y: area.y + top_pad,
        width: area.width.min(7),
        height: 4,
    };
    let col = if pct > 0 { Color::Cyan } else { Color::DarkGray };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("╭────╮", Style::default().fg(col))),
            Line::from(vec![
                Span::styled("│", Style::default().fg(col)),
                Span::styled(
                    format!("{:>3} ", pct),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("│", Style::default().fg(col)),
            ]),
            Line::from(vec![
                Span::styled("│", Style::default().fg(col)),
                Span::styled("  % ", Style::default().fg(Color::Gray)),
                Span::styled("│", Style::default().fg(col)),
            ]),
            Line::from(Span::styled("╰────╯", Style::default().fg(col))),
        ]),
        knob,
    );
}

fn format_audio_info(track: &tornade_core::models::Track) -> (String, String) {
    let fmt = match track.file_type {
        AudioFormat::Flac => "FLAC",
        AudioFormat::Mp3 => "MP3",
        AudioFormat::Aac => "AAC",
        AudioFormat::Alac => "ALAC",
    };
    let bit = track
        .bit_depth
        .map(|b| format!(" {}bit", b))
        .unwrap_or_default();
    let sr = track
        .sample_rate
        .map(|s| {
            if s % 1000 == 0 {
                format!(" {}kHz", s / 1000)
            } else {
                format!(" {:.1}kHz", s as f64 / 1000.0)
            }
        })
        .unwrap_or_default();
    let size = format!("{:.1} MB", track.file_size as f64 / (1024.0 * 1024.0));
    (format!("{}{}{}", fmt, bit, sr), size)
}
