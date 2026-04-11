use crate::player::PlayerStateCache;
use crate::utils::{format_duration, truncate};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Gauge, Paragraph},
};
use ratatui_image::{StatefulImage, protocol::StatefulProtocol};
use tornade_core::models::AudioFormat;
use tornade_core::services::PlaybackState;

/// Hit zones for player buttons — positions relative to the player area.
/// Stored in AppState so the mouse handler can use them.
#[derive(Default, Clone, Copy)]
pub struct PlayerHitZones {
    pub prev: Option<Rect>,
    pub play_pause: Option<Rect>,
    pub next: Option<Rect>,
    pub shuffle: Option<Rect>,
    pub repeat: Option<Rect>,
}

/// Full player panel: artwork | info | volume (top) + controls+progress (bottom row).
/// Returns the hit zones for the three transport buttons.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    cache: &PlayerStateCache,
    image_state: Option<&mut StatefulProtocol>,
    album_name: Option<&str>,
) -> PlayerHitZones {
    if area.height < 2 {
        return PlayerHitZones::default();
    }

    // Vertical: [top: artwork+info+vol (fills)] | [bottom: controls+progress (1 row)]
    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);

    // Top horizontal: [artwork 10] | [gap 1] | [info] | [volume 7]
    let top = Layout::horizontal([
        Constraint::Length(10),
        Constraint::Length(1),
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

    // Track info (top[2] after the gap at top[1])
    let w = top[2].width as usize;
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
        top[2],
    );

    // Volume knob
    let vol_pct = (cache.volume * 100.0).round() as u8;
    render_volume_knob(frame, top[3], vol_pct);

    // Bottom row: [transport=10] [gap=1] [elapsed=5] [progress] [total=5]
    let bottom = Layout::horizontal([
        Constraint::Length(10), // transport (prev + play + next)
        Constraint::Length(1),  // gap — mirrors top artwork gap
        Constraint::Length(5),  // elapsed
        Constraint::Min(0),     // progress gauge
        Constraint::Length(5),  // total
    ])
    .split(vert[1]);

    // Transport: spread prev / play / next across the full artwork width (3+4+3=10)
    let transport = Layout::horizontal([
        Constraint::Length(3), // ◀◀
        Constraint::Length(4), // ▶/⏸  (wider = more prominent)
        Constraint::Length(3), // ▶▶
    ])
    .split(bottom[0]);

    let play_icon = match cache.state {
        PlaybackState::Playing => "⏸",
        _ => "▶",
    };

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "◀◀",
            Style::default().fg(Color::Gray),
        )))
        .alignment(Alignment::Center),
        transport[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            play_icon,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        transport[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "▶▶",
            Style::default().fg(Color::Gray),
        )))
        .alignment(Alignment::Center),
        transport[2],
    );

    // Elapsed / progress / total
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
        Paragraph::new(Line::from(Span::styled(
            elapsed,
            Style::default().fg(Color::Gray),
        ))),
        bottom[2],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(50, 52, 64)))
            .ratio(ratio)
            .label(""),
        bottom[3],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            total_dur,
            Style::default().fg(Color::Gray),
        )))
        .alignment(ratatui::layout::Alignment::Right),
        bottom[4],
    );

    PlayerHitZones {
        prev: Some(transport[0]),
        play_pause: Some(transport[1]),
        next: Some(transport[2]),
        shuffle: None,
        repeat: None,
    }
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
    let col = if pct > 0 {
        Color::Cyan
    } else {
        Color::DarkGray
    };
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
