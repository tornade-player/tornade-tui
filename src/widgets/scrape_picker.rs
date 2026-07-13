//! Overlay for reviewing online metadata candidates (feature 012, US3).
//!
//! Launched from the tag editor with `f`. Displays the async job status
//! (Searching… / No match / Failed: …), a list of [`ScrapeCandidate`]s, and
//! per-field acceptance toggles for the selected candidate. `Space` toggles a
//! field, `Enter` applies the accepted fields to the tag editor, `Esc` cancels.
//!
//! Follows the [`crate::widgets::input_dialog`] / [`crate::widgets::tag_editor`]
//! overlay pattern (a [`Clear`] over a centred rect).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tornade_core::services::metadata_scrape::ScrapeCandidate;

use crate::async_worker::JobStatus;

/// A candidate field the user can accept/reject before applying.
///
/// Ordering defines the on-screen order and the toggle navigation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateField {
    Title,
    Artist,
    AlbumArtist,
    Album,
    Year,
    Genre,
    TrackNumber,
}

impl CandidateField {
    pub const ALL: [CandidateField; 7] = [
        CandidateField::Title,
        CandidateField::Artist,
        CandidateField::AlbumArtist,
        CandidateField::Album,
        CandidateField::Year,
        CandidateField::Genre,
        CandidateField::TrackNumber,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CandidateField::Title => "Title",
            CandidateField::Artist => "Artist",
            CandidateField::AlbumArtist => "Album Artist",
            CandidateField::Album => "Album",
            CandidateField::Year => "Year",
            CandidateField::Genre => "Genre",
            CandidateField::TrackNumber => "Track #",
        }
    }
}

/// State for the scrape picker overlay.
pub struct ScrapePickerState {
    /// Correlation id of the launched job; results with other ids are ignored.
    pub job_id: u64,
    /// Current async status (drives the header line).
    pub status: JobStatus,
    /// Candidates returned by the job (empty until they arrive).
    pub candidates: Vec<ScrapeCandidate>,
    /// Index of the highlighted candidate.
    pub selected: usize,
    /// Index into [`CandidateField::ALL`] of the highlighted toggle row.
    pub field_cursor: usize,
    /// Fields the user has ACCEPTED for application. A field is only applied if
    /// it is accepted AND the selected candidate actually provides a value.
    accepted: [bool; 7],
}

impl ScrapePickerState {
    /// Create a picker in the "searching" state for `job_id`.
    pub fn searching(job_id: u64) -> Self {
        Self {
            job_id,
            status: JobStatus::Searching(job_id),
            candidates: Vec::new(),
            selected: 0,
            field_cursor: 0,
            // Fields that carry a value are accepted by default.
            accepted: [true; 7],
        }
    }

    /// Install candidates once the job resolves, updating status accordingly.
    pub fn set_candidates(&mut self, candidates: Vec<ScrapeCandidate>) {
        if candidates.is_empty() {
            self.status = JobStatus::Empty(self.job_id);
        } else {
            self.status = JobStatus::Ready(self.job_id);
        }
        self.candidates = candidates;
        self.selected = 0;
        self.field_cursor = 0;
    }

    /// Mark the job as failed with a user-facing message.
    pub fn set_failed(&mut self, msg: String) {
        self.status = JobStatus::Failed(msg);
    }

    /// The currently highlighted candidate, if any.
    pub fn current(&self) -> Option<&ScrapeCandidate> {
        self.candidates.get(self.selected)
    }

    /// Move the candidate selection down (wrapping).
    pub fn next_candidate(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.candidates.len();
    }

    /// Move the candidate selection up (wrapping).
    pub fn prev_candidate(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        self.selected = (self.selected + self.candidates.len() - 1) % self.candidates.len();
    }

    /// Move the field toggle cursor down (wrapping).
    pub fn next_field(&mut self) {
        self.field_cursor = (self.field_cursor + 1) % CandidateField::ALL.len();
    }

    /// Move the field toggle cursor up (wrapping).
    pub fn prev_field(&mut self) {
        let len = CandidateField::ALL.len();
        self.field_cursor = (self.field_cursor + len - 1) % len;
    }

    /// Toggle acceptance of the field under the cursor.
    pub fn toggle_field(&mut self) {
        self.accepted[self.field_cursor] = !self.accepted[self.field_cursor];
    }

    /// Whether `field` is currently accepted.
    pub fn is_accepted(&self, field: CandidateField) -> bool {
        let idx = CandidateField::ALL
            .iter()
            .position(|f| *f == field)
            .unwrap();
        self.accepted[idx]
    }

    /// The display value the selected candidate provides for `field`, if any.
    ///
    /// GENRE MAPPING (C1): the genre list is single-valued in the tag model, so
    /// only the FIRST genre is exposed.
    pub fn field_value(&self, field: CandidateField) -> Option<String> {
        let c = self.current()?;
        match field {
            CandidateField::Title => non_empty(&c.title),
            CandidateField::Artist => non_empty(&c.artist),
            CandidateField::AlbumArtist => c.album_artist.as_deref().and_then(non_empty),
            CandidateField::Album => c.album.as_deref().and_then(non_empty),
            CandidateField::Year => c.year.map(|y| y.to_string()),
            CandidateField::Genre => c.genres.first().and_then(|g| non_empty(g)),
            CandidateField::TrackNumber => c.track_number.map(|n| n.to_string()),
        }
    }

    /// True once the header should stop showing a spinner (job resolved).
    pub fn is_resolved(&self) -> bool {
        !matches!(self.status, JobStatus::Searching(_))
    }
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Render the scrape picker overlay.
pub fn render(frame: &mut Frame, state: &ScrapePickerState) {
    let area = centered_rect(70, 22, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Fetch Metadata Online ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Header / status line.
    let (status_text, status_color) = match &state.status {
        JobStatus::Idle | JobStatus::Searching(_) => ("Searching…".to_string(), Color::Yellow),
        JobStatus::Empty(_) => ("No match".to_string(), Color::Yellow),
        JobStatus::Failed(msg) => (format!("Failed: {msg}"), Color::Red),
        JobStatus::Ready(_) => (
            format!("{} candidate(s)", state.candidates.len()),
            Color::Green,
        ),
    };
    lines.push(Line::from(Span::styled(
        format!("  {status_text}"),
        Style::default()
            .fg(status_color)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Candidate list.
    for (i, c) in state.candidates.iter().enumerate() {
        let marker = if i == state.selected { "▶ " } else { "  " };
        let album = c.album.as_deref().unwrap_or("—");
        let year = c.year.map(|y| y.to_string()).unwrap_or_else(|| "—".into());
        let summary = format!(
            "{marker}{} — {} · {} · {} · score {}",
            c.title, c.artist, album, year, c.score
        );
        let style = if i == state.selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(Span::styled(summary, style)));
    }

    if state.candidates.is_empty() && state.is_resolved() {
        lines.push(Line::from(Span::styled(
            "  (no candidates)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Per-field acceptance toggles for the selected candidate.
    if state.current().is_some() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Accept fields (Space to toggle):",
            Style::default().fg(Color::White),
        )));
        for (i, &field) in CandidateField::ALL.iter().enumerate() {
            let value = state.field_value(field);
            let has_value = value.is_some();
            let accepted = state.is_accepted(field) && has_value;
            let checkbox = if accepted { "[x]" } else { "[ ]" };
            let is_cursor = i == state.field_cursor;
            let cursor_marker = if is_cursor { "▶ " } else { "  " };
            let val_text = value.unwrap_or_else(|| "—".into());
            let style = if is_cursor {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if has_value {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "{cursor_marker}{checkbox} {:>12}  {}",
                    field.label(),
                    val_text
                ),
                style,
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  Enter: Apply  ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ↑/↓: candidate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("  j/k: field  ", Style::default().fg(Color::DarkGray)),
        Span::styled("  Space: toggle  ", Style::default().fg(Color::DarkGray)),
        Span::styled("  Esc: Cancel", Style::default().fg(Color::DarkGray)),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> ScrapeCandidate {
        ScrapeCandidate {
            musicbrainz_id: "mbid".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album_artist: Some("Album Artist".into()),
            album: Some("Album".into()),
            year: Some(2021),
            genres: vec!["Rock".into(), "Indie".into()],
            track_number: Some(4),
            disc_number: None,
            has_artwork: true,
            score: 95,
        }
    }

    #[test]
    fn genre_mapping_takes_first_only() {
        let mut s = ScrapePickerState::searching(1);
        s.set_candidates(vec![candidate()]);
        assert_eq!(
            s.field_value(CandidateField::Genre),
            Some("Rock".to_string())
        );
    }

    #[test]
    fn empty_candidates_sets_empty_status() {
        let mut s = ScrapePickerState::searching(2);
        s.set_candidates(Vec::new());
        assert_eq!(s.status, JobStatus::Empty(2));
    }

    #[test]
    fn field_toggle_flips_acceptance() {
        let mut s = ScrapePickerState::searching(3);
        s.set_candidates(vec![candidate()]);
        // Title is first field, accepted by default.
        assert!(s.is_accepted(CandidateField::Title));
        s.field_cursor = 0;
        s.toggle_field();
        assert!(!s.is_accepted(CandidateField::Title));
    }

    #[test]
    fn missing_field_value_is_none() {
        let mut c = candidate();
        c.year = None;
        let mut s = ScrapePickerState::searching(4);
        s.set_candidates(vec![c]);
        assert_eq!(s.field_value(CandidateField::Year), None);
    }

    #[test]
    fn candidate_navigation_wraps() {
        let mut s = ScrapePickerState::searching(5);
        s.set_candidates(vec![candidate(), candidate()]);
        assert_eq!(s.selected, 0);
        s.next_candidate();
        assert_eq!(s.selected, 1);
        s.next_candidate();
        assert_eq!(s.selected, 0);
        s.prev_candidate();
        assert_eq!(s.selected, 1);
    }
}
