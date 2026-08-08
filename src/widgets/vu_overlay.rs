//! Full-screen VU meter overlay (toggled with `V` or `:vis`).
//!
//! Renders classic left/right channel level meters from the visualizer sample
//! tap exposed by `tornade-core` (`PlayerService::visualizer_samples`): RMS
//! bars with green/yellow/red zones on a dBFS scale, plus a peak-hold marker.
//!
//! Meter ballistics live in [`VuMeterState`], separated from rendering and
//! time-injectable so they are unit-testable: instant attack, timed release,
//! and a peak-hold that sticks for [`PEAK_HOLD`] before falling.

use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Meter floor: anything at or below this renders as an empty bar.
pub const FLOOR_DB: f32 = -60.0;
/// Release rate for the RMS bar and (after hold expiry) the peak marker.
pub const RELEASE_DB_PER_S: f32 = 24.0;
/// How long the peak marker holds before it starts to fall.
pub const PEAK_HOLD: Duration = Duration::from_millis(1500);
/// Zone boundaries (dBFS): green below -12, yellow to -3, red above.
const YELLOW_FROM_DB: f32 = -12.0;
const RED_FROM_DB: f32 = -3.0;

/// One channel's smoothed level and held peak, both in dBFS.
pub struct ChannelMeter {
    pub level_db: f32,
    pub peak_db: f32,
    peak_set_at: Option<Instant>,
}

impl ChannelMeter {
    fn new() -> Self {
        Self {
            level_db: FLOOR_DB,
            peak_db: FLOOR_DB,
            peak_set_at: None,
        }
    }

    fn apply(&mut self, rms_db: f32, peak_db: f32, now: Instant, dt: f32) {
        // RMS bar: instant attack, constant-rate release.
        if rms_db >= self.level_db {
            self.level_db = rms_db;
        } else {
            self.level_db = (self.level_db - RELEASE_DB_PER_S * dt).max(rms_db);
        }

        // Peak marker: latch upward, hold, then fall at the release rate.
        if peak_db >= self.peak_db {
            self.peak_db = peak_db;
            self.peak_set_at = Some(now);
        } else if self
            .peak_set_at
            .is_none_or(|t| now.duration_since(t) >= PEAK_HOLD)
        {
            self.peak_db = (self.peak_db - RELEASE_DB_PER_S * dt).max(peak_db);
        }

        self.level_db = self.level_db.clamp(FLOOR_DB, 0.0);
        self.peak_db = self.peak_db.clamp(FLOOR_DB, 0.0);
    }
}

/// Left/right meter state fed from interleaved output samples.
pub struct VuMeterState {
    pub left: ChannelMeter,
    pub right: ChannelMeter,
    last_update: Option<Instant>,
}

impl Default for VuMeterState {
    fn default() -> Self {
        Self {
            left: ChannelMeter::new(),
            right: ChannelMeter::new(),
            last_update: None,
        }
    }
}

fn to_db(amplitude: f32) -> f32 {
    if amplitude <= 0.0 {
        FLOOR_DB
    } else {
        (20.0 * amplitude.log10()).max(FLOOR_DB)
    }
}

/// RMS and absolute peak of one channel of an interleaved buffer, in dBFS.
fn channel_levels(samples: &[f32], channels: usize, channel: usize) -> (f32, f32) {
    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f32;
    let mut count = 0u32;
    for &s in samples.iter().skip(channel).step_by(channels.max(1)) {
        sum_sq += f64::from(s) * f64::from(s);
        peak = peak.max(s.abs());
        count += 1;
    }
    if count == 0 {
        return (FLOOR_DB, FLOOR_DB);
    }
    let rms = (sum_sq / f64::from(count)).sqrt() as f32;
    (to_db(rms), to_db(peak))
}

impl VuMeterState {
    /// Feed a window of interleaved output samples. Mono input drives both
    /// meters; for >2 channels the first two are shown. `now` is injected for
    /// testability.
    pub fn update(&mut self, samples: &[f32], channels: u16, now: Instant) {
        let dt = self
            .last_update
            .map_or(0.0, |t| now.duration_since(t).as_secs_f32());
        self.last_update = Some(now);

        let ch = channels.max(1) as usize;
        let (l_rms, l_peak) = channel_levels(samples, ch, 0);
        let (r_rms, r_peak) = if ch >= 2 {
            channel_levels(samples, ch, 1)
        } else {
            (l_rms, l_peak)
        };
        self.left.apply(l_rms, l_peak, now, dt);
        self.right.apply(r_rms, r_peak, now, dt);
    }
}

/// Map a dB value to a character column on a bar of `width` cells.
fn db_to_col(db: f32, width: u16) -> u16 {
    let frac = ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
    (frac * f32::from(width)).round() as u16
}

fn zone_color(col: u16, width: u16) -> Color {
    let db = FLOOR_DB + (f32::from(col) / f32::from(width)) * -FLOOR_DB;
    if db >= RED_FROM_DB {
        Color::Red
    } else if db >= YELLOW_FROM_DB {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn meter_line(meter: &ChannelMeter, label: &str, width: u16) -> Line<'static> {
    let filled = db_to_col(meter.level_db, width);
    let peak_col = db_to_col(meter.peak_db, width).min(width.saturating_sub(1));
    let mut spans = vec![Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    for col in 0..width {
        let span = if col == peak_col && meter.peak_db > FLOOR_DB {
            Span::styled("▌", Style::default().fg(Color::White))
        } else if col < filled {
            Span::styled("█", Style::default().fg(zone_color(col, width)))
        } else {
            Span::styled("·", Style::default().fg(Color::DarkGray))
        };
        spans.push(span);
    }
    spans.push(Span::styled(
        format!(" {:>5.1} dB", meter.level_db),
        Style::default().fg(Color::Gray),
    ));
    Line::from(spans)
}

/// dB tick scale aligned under the bars.
fn scale_line(label_pad: usize, width: u16) -> Line<'static> {
    let mut chars = vec![' '; width as usize];
    for &db in &[-60.0f32, -40.0, -30.0, -20.0, -12.0, -6.0, -3.0] {
        let col = db_to_col(db, width).min(width.saturating_sub(1)) as usize;
        let text: Vec<char> = format!("{}", db as i32).chars().collect();
        for (i, c) in text.iter().enumerate() {
            if col + i < chars.len() {
                chars[col + i] = *c;
            }
        }
    }
    if width >= 1 {
        chars[width as usize - 1] = '0';
    }
    Line::from(vec![
        Span::raw(" ".repeat(label_pad)),
        Span::styled(
            chars.into_iter().collect::<String>(),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}

/// Render the overlay. `title`/`artist` describe the current track (if any).
pub fn render(frame: &mut Frame, vu: &VuMeterState, title: Option<&str>, artist: Option<&str>) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" VU ")
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Bar width leaves room for the " L " label and the dB readout.
    let label_pad = 3usize;
    let readout = 9u16;
    let bar_width = inner
        .width
        .saturating_sub(label_pad as u16 + readout)
        .clamp(10, 120);

    let track_line = match title {
        Some(t) => {
            let mut s = t.to_string();
            if let Some(a) = artist {
                s = format!("{a} — {s}");
            }
            Line::from(Span::styled(
                s,
                Style::default().add_modifier(Modifier::BOLD),
            ))
        }
        None => Line::from(Span::styled(
            "Nothing playing",
            Style::default().fg(Color::DarkGray),
        )),
    };

    let lines = vec![
        Line::from(""),
        track_line,
        Line::from(""),
        Line::from(""),
        meter_line(&vu.left, "L", bar_width),
        Line::from(""),
        meter_line(&vu.right, "R", bar_width),
        Line::from(""),
        scale_line(label_pad, bar_width),
        Line::from(""),
        Line::from(Span::styled(
            "V / q / Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    // Center the meter block vertically.
    let content_height = lines.len() as u16;
    let y = inner.y + inner.height.saturating_sub(content_height) / 2;
    let content_area = Rect {
        x: inner.x + 1,
        y,
        width: inner.width.saturating_sub(2),
        height: content_height.min(inner.height),
    };
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        content_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_at(now: Instant) -> VuMeterState {
        let mut vu = VuMeterState::default();
        // Prime last_update so the next update has a known dt.
        vu.update(&[], 2, now);
        vu
    }

    #[test]
    fn silence_reads_floor() {
        let now = Instant::now();
        let mut vu = state_at(now);
        vu.update(&[0.0, 0.0, 0.0, 0.0], 2, now + Duration::from_millis(33));
        assert_eq!(vu.left.level_db, FLOOR_DB);
        assert_eq!(vu.right.level_db, FLOOR_DB);
    }

    #[test]
    fn full_scale_reads_zero_db() {
        let now = Instant::now();
        let mut vu = state_at(now);
        let samples = vec![1.0f32; 512];
        vu.update(&samples, 2, now + Duration::from_millis(33));
        assert!(vu.left.level_db.abs() < 0.01, "got {}", vu.left.level_db);
        assert!(vu.left.peak_db.abs() < 0.01);
    }

    #[test]
    fn channels_are_independent() {
        let now = Instant::now();
        let mut vu = state_at(now);
        // Interleaved L=1.0, R=0.0
        let samples: Vec<f32> = (0..512)
            .map(|i| if i % 2 == 0 { 1.0 } else { 0.0 })
            .collect();
        vu.update(&samples, 2, now + Duration::from_millis(33));
        assert!(vu.left.level_db > -1.0);
        assert_eq!(vu.right.level_db, FLOOR_DB);
    }

    #[test]
    fn mono_drives_both_meters() {
        let now = Instant::now();
        let mut vu = state_at(now);
        let samples = vec![0.5f32; 256];
        vu.update(&samples, 1, now + Duration::from_millis(33));
        assert_eq!(vu.left.level_db, vu.right.level_db);
        assert!(vu.left.level_db > FLOOR_DB);
    }

    #[test]
    fn level_releases_at_fixed_rate() {
        let now = Instant::now();
        let mut vu = state_at(now);
        let loud = vec![1.0f32; 512];
        vu.update(&loud, 2, now + Duration::from_secs(1));
        // One second of silence should drop the bar by RELEASE_DB_PER_S.
        vu.update(&[0.0, 0.0], 2, now + Duration::from_secs(2));
        let expected = 0.0 - RELEASE_DB_PER_S;
        assert!(
            (vu.left.level_db - expected).abs() < 0.5,
            "got {}",
            vu.left.level_db
        );
    }

    #[test]
    fn peak_holds_then_falls() {
        let now = Instant::now();
        let mut vu = state_at(now);
        let loud = vec![1.0f32; 512];
        vu.update(&loud, 2, now + Duration::from_secs(1));
        // Within the hold window the peak must not move.
        vu.update(&[0.0, 0.0], 2, now + Duration::from_millis(2000));
        assert!(vu.left.peak_db.abs() < 0.01, "peak fell during hold");
        // Well past the hold window it must have fallen.
        vu.update(&[0.0, 0.0], 2, now + Duration::from_secs(4));
        assert!(vu.left.peak_db < -10.0, "peak did not fall after hold");
    }

    #[test]
    fn db_to_col_maps_range() {
        assert_eq!(db_to_col(FLOOR_DB, 60), 0);
        assert_eq!(db_to_col(0.0, 60), 60);
        assert_eq!(db_to_col(-30.0, 60), 30);
    }
}
