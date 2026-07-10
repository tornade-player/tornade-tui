//! Overlay "tag editor" for editing a track's metadata.
//!
//! For a SINGLE highlighted track all fields are editable (title, artist,
//! album, album artist, year, genre, track number). For a MULTI-track
//! selection only the album-level fields are editable (album, album artist,
//! year, genre); fields whose values differ across the targets are shown with a
//! "multiple values" placeholder until the user overwrites them.
//!
//! The editor follows the overlay pattern used by [`crate::widgets::input_dialog`]
//! (a [`Clear`] over a centred rect). It never touches the database directly:
//! prefill comes from [`tornade_core::services::MetadataEditService`] and saves
//! go back through the same service (wired up in `events.rs`).

use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tornade_core::services::TrackTagUpdate;

/// One editable field in the tag editor.
///
/// The ordering here defines the on-screen order and the Tab focus order for a
/// single-track edit. Multi-track edits expose only the album-level subset (see
/// [`TagEditorState::visible_fields`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Title,
    Artist,
    Album,
    AlbumArtist,
    Year,
    Genre,
    TrackNumber,
}

impl Field {
    /// All fields, in display / focus order.
    pub const ALL: [Field; 7] = [
        Field::Title,
        Field::Artist,
        Field::Album,
        Field::AlbumArtist,
        Field::Year,
        Field::Genre,
        Field::TrackNumber,
    ];

    /// Album-level fields, editable for a multi-track selection.
    pub const ALBUM_LEVEL: [Field; 4] =
        [Field::Album, Field::AlbumArtist, Field::Year, Field::Genre];

    /// Human-readable label shown to the left of the field.
    pub fn label(self) -> &'static str {
        match self {
            Field::Title => "Title",
            Field::Artist => "Artist",
            Field::Album => "Album",
            Field::AlbumArtist => "Album Artist",
            Field::Year => "Year",
            Field::Genre => "Genre",
            Field::TrackNumber => "Track #",
        }
    }
}

/// Placeholder shown for a field that has different values across the targets.
pub const MIXED_PLACEHOLDER: &str = "<multiple values>";

/// Editable state for the tag editor overlay.
///
/// Built from the prefilled [`TrackTagUpdate`] of every target track. Only the
/// fields the user actually edits (tracked in [`Self::dirty`]) are applied on
/// save; untouched fields keep their prefilled value.
pub struct TagEditorState {
    /// Track ids being edited (length 1 = single-track, >1 = album-level).
    pub targets: Vec<i64>,
    /// Current text buffer per field (indexed by [`Field`]).
    fields: Vec<(Field, String)>,
    /// Fields whose prefill differed across targets (multi-track only).
    pub mixed: HashSet<Field>,
    /// Fields the user has edited since the editor opened.
    pub dirty: HashSet<Field>,
    /// Inline validation error (currently only the year field), blocks save.
    pub validation_error: Option<String>,
    /// Index into [`Self::visible_fields`] of the currently focused field.
    pub focus: usize,
    /// Cached prefill used to fill non-dirty fields on save.
    prefill: TrackTagUpdate,
}

impl TagEditorState {
    /// Build editor state for a single track from its current values.
    pub fn single(track_id: i64, current: TrackTagUpdate) -> Self {
        let fields = vec![
            (Field::Title, current.title.clone()),
            (Field::Artist, current.artist_name.clone()),
            (
                Field::Album,
                current.album_title.clone().unwrap_or_default(),
            ),
            (
                Field::AlbumArtist,
                current.album_artist_name.clone().unwrap_or_default(),
            ),
            (
                Field::Year,
                current.year.map(|y| y.to_string()).unwrap_or_default(),
            ),
            (
                Field::Genre,
                current.genre_names.first().cloned().unwrap_or_default(),
            ),
            (
                Field::TrackNumber,
                current
                    .track_number
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            ),
        ];
        Self {
            targets: vec![track_id],
            fields,
            mixed: HashSet::new(),
            dirty: HashSet::new(),
            validation_error: None,
            focus: 0,
            prefill: current,
        }
    }

    /// Build editor state for a multi-track (album-level) selection.
    ///
    /// `currents` is the prefill of every target. Only album-level fields are
    /// editable; a field is marked [`mixed`](Self::mixed) (and left blank) when
    /// its value differs across the targets.
    pub fn multi(track_ids: Vec<i64>, currents: &[TrackTagUpdate]) -> Self {
        // Use the first target as the prefill baseline for shared values.
        let base = currents.first().cloned().unwrap_or_else(empty_update);

        let mut mixed = HashSet::new();
        let album_same = currents.iter().all(|c| c.album_title == base.album_title);
        let album_artist_same = currents
            .iter()
            .all(|c| c.album_artist_name == base.album_artist_name);
        let year_same = currents.iter().all(|c| c.year == base.year);
        let genre_same = currents.iter().all(|c| c.genre_names == base.genre_names);
        if !album_same {
            mixed.insert(Field::Album);
        }
        if !album_artist_same {
            mixed.insert(Field::AlbumArtist);
        }
        if !year_same {
            mixed.insert(Field::Year);
        }
        if !genre_same {
            mixed.insert(Field::Genre);
        }

        let val = |f: Field, s: String| if mixed.contains(&f) { String::new() } else { s };
        let fields = vec![
            (Field::Title, String::new()),
            (Field::Artist, String::new()),
            (
                Field::Album,
                val(Field::Album, base.album_title.clone().unwrap_or_default()),
            ),
            (
                Field::AlbumArtist,
                val(
                    Field::AlbumArtist,
                    base.album_artist_name.clone().unwrap_or_default(),
                ),
            ),
            (
                Field::Year,
                val(
                    Field::Year,
                    base.year.map(|y| y.to_string()).unwrap_or_default(),
                ),
            ),
            (
                Field::Genre,
                val(
                    Field::Genre,
                    base.genre_names.first().cloned().unwrap_or_default(),
                ),
            ),
            (Field::TrackNumber, String::new()),
        ];

        Self {
            targets: track_ids,
            fields,
            mixed,
            dirty: HashSet::new(),
            validation_error: None,
            focus: 0,
            prefill: base,
        }
    }

    /// True when more than one track is being edited (album-level mode).
    pub fn is_multi(&self) -> bool {
        self.targets.len() > 1
    }

    /// Fields shown/editable for this editor: all for single-track, only the
    /// album-level subset for multi-track.
    pub fn visible_fields(&self) -> &'static [Field] {
        if self.is_multi() {
            &Field::ALBUM_LEVEL
        } else {
            &Field::ALL
        }
    }

    /// Current buffer text for a field.
    pub fn value(&self, field: Field) -> &str {
        self.fields
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    fn value_mut(&mut self, field: Field) -> &mut String {
        // Every field is always present, so this cannot fail.
        &mut self
            .fields
            .iter_mut()
            .find(|(f, _)| *f == field)
            .expect("field present")
            .1
    }

    /// The currently focused field.
    pub fn focused_field(&self) -> Field {
        let visible = self.visible_fields();
        visible[self.focus.min(visible.len() - 1)]
    }

    /// Move focus to the next field (wrapping).
    pub fn focus_next(&mut self) {
        let len = self.visible_fields().len();
        self.focus = (self.focus + 1) % len;
    }

    /// Move focus to the previous field (wrapping).
    pub fn focus_prev(&mut self) {
        let len = self.visible_fields().len();
        self.focus = (self.focus + len - 1) % len;
    }

    /// Append a character to the focused field and mark it dirty.
    pub fn push_char(&mut self, c: char) {
        let field = self.focused_field();
        self.dirty.insert(field);
        self.value_mut(field).push(c);
        self.revalidate();
    }

    /// Delete the last character of the focused field and mark it dirty.
    pub fn backspace(&mut self) {
        let field = self.focused_field();
        self.dirty.insert(field);
        self.value_mut(field).pop();
        self.revalidate();
    }

    /// Recompute the (year) validation error. Called after every edit.
    fn revalidate(&mut self) {
        self.validation_error = self.validate_year_err();
    }

    /// Validate the current year buffer. Returns an error message if the field
    /// is non-empty and either non-numeric or out of range
    /// (`1900..=current_year + 1`).
    fn validate_year_err(&self) -> Option<String> {
        let raw = self.value(Field::Year).trim();
        if raw.is_empty() {
            return None;
        }
        match raw.parse::<i64>() {
            Ok(y) => {
                let max = i64::from(current_year()) + 1;
                if y < 1900 || y > max {
                    Some(format!("Year must be between 1900 and {max}"))
                } else {
                    None
                }
            }
            Err(_) => Some("Year must be a number".to_string()),
        }
    }

    /// True when the editor is in a valid state and can be saved.
    pub fn is_valid(&self) -> bool {
        self.validate_year_err().is_none()
    }

    /// Build the [`TrackTagUpdate`] to persist.
    ///
    /// Only dirty fields override the prefilled baseline; untouched fields keep
    /// their prefill values. For multi-track edits the returned update is passed
    /// to `update_tracks`, which itself only applies the album-level fields.
    pub fn build_update(&self) -> TrackTagUpdate {
        let mut update = self.prefill.clone();

        if self.dirty.contains(&Field::Title) {
            update.title = self.value(Field::Title).to_string();
        }
        if self.dirty.contains(&Field::Artist) {
            update.artist_name = self.value(Field::Artist).to_string();
        }
        if self.dirty.contains(&Field::Album) {
            update.album_title = non_empty(self.value(Field::Album));
        }
        if self.dirty.contains(&Field::AlbumArtist) {
            update.album_artist_name = non_empty(self.value(Field::AlbumArtist));
        }
        if self.dirty.contains(&Field::Year) {
            let raw = self.value(Field::Year).trim();
            update.year = raw.parse::<u16>().ok().filter(|_| !raw.is_empty());
            if raw.is_empty() {
                update.year = None;
            }
        }
        if self.dirty.contains(&Field::Genre) {
            let g = self.value(Field::Genre).trim();
            update.genre_names = if g.is_empty() {
                Vec::new()
            } else {
                vec![g.to_string()]
            };
        }
        if self.dirty.contains(&Field::TrackNumber) {
            let raw = self.value(Field::TrackNumber).trim();
            update.track_number = raw.parse::<u32>().ok().filter(|_| !raw.is_empty());
        }

        update
    }
}

/// `Some(s)` if `s` is non-empty after trimming, else `None`.
fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// An empty prefill baseline (used when a multi-track list is unexpectedly empty).
#[allow(dead_code)] // only reached via `multi`, wired up in US2
fn empty_update() -> TrackTagUpdate {
    TrackTagUpdate {
        title: String::new(),
        artist_name: String::new(),
        album_title: None,
        album_artist_name: None,
        year: None,
        genre_names: Vec::new(),
        track_number: None,
        disc_number: None,
    }
}

/// Current year from the system clock, computed without pulling in `chrono`
/// (which is only a transitive dependency of this crate).
///
/// Uses the civil-from-days algorithm (Howard Hinnant) on the Unix day count.
fn current_year() -> i32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    // days-from-civil inverse: derive (year, month, day) from days since epoch.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let year = if mp >= 10 { y + 1 } else { y };
    year as i32
}

/// Render the tag editor overlay.
pub fn render(frame: &mut Frame, state: &TagEditorState) {
    let visible = state.visible_fields();
    // form rows + title + actions + error + padding
    let height = (visible.len() as u16) + 6;
    let area = centered_rect(60, height, frame.area());
    frame.render_widget(Clear, area);

    let title = if state.is_multi() {
        format!(" Edit Tags — {} tracks ", state.targets.len())
    } else {
        " Edit Tags ".to_string()
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    let focused = state.focused_field();
    let label_w = visible.iter().map(|f| f.label().len()).max().unwrap_or(0);

    for &field in visible {
        let is_focus = field == focused;
        let raw = state.value(field);
        let (text, is_placeholder) = if raw.is_empty() && state.mixed.contains(&field) {
            (MIXED_PLACEHOLDER.to_string(), true)
        } else {
            (raw.to_string(), false)
        };

        let label = format!("{:>width$}", field.label(), width = label_w);
        let label_style = if is_focus {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let value_style = if is_placeholder {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            Span::styled(label, label_style),
            Span::raw("  "),
            Span::styled(text, value_style),
        ];
        if is_focus {
            spans.push(Span::styled("█", Style::default().fg(Color::Cyan)));
        }
        lines.push(Line::from(spans));
    }

    // Blank + error line.
    lines.push(Line::from(""));
    if let Some(err) = &state.validation_error {
        lines.push(Line::from(Span::styled(
            format!("  {err}"),
            Style::default().fg(Color::Red),
        )));
    } else {
        lines.push(Line::from(""));
    }

    // Actions row.
    lines.push(Line::from(vec![
        Span::styled(
            "  Enter: Save  ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  Esc: Cancel  ", Style::default().fg(Color::DarkGray)),
        Span::styled("  Tab: next field", Style::default().fg(Color::DarkGray)),
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
    use ratatui::{Terminal, backend::TestBackend};

    fn make_update(
        title: &str,
        artist: &str,
        album: Option<&str>,
        album_artist: Option<&str>,
        year: Option<u16>,
        genre: Option<&str>,
        track_number: Option<u32>,
    ) -> TrackTagUpdate {
        TrackTagUpdate {
            title: title.to_string(),
            artist_name: artist.to_string(),
            album_title: album.map(str::to_string),
            album_artist_name: album_artist.map(str::to_string),
            year,
            genre_names: genre.map(|g| vec![g.to_string()]).unwrap_or_default(),
            track_number,
            disc_number: None,
        }
    }

    /// Render into an in-memory buffer and return it as plain text.
    fn render_to_text(state: &TagEditorState, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| render(frame, state)).unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for row in 0..buf.area.height {
            for col in 0..buf.area.width {
                out.push_str(buf.cell((col, row)).unwrap().symbol());
            }
            out.push('\n');
        }
        out
    }

    // T015: single-track render shows all fields. ---------------------------
    #[test]
    fn test_tag_editor_render_single_shows_all_fields() {
        let update = make_update(
            "My Title",
            "My Artist",
            Some("My Album"),
            Some("My Album Artist"),
            Some(2020),
            Some("Jazz"),
            Some(3),
        );
        let state = TagEditorState::single(42, update);
        let text = render_to_text(&state, 80, 24);

        for label in [
            "Title",
            "Artist",
            "Album",
            "Album Artist",
            "Year",
            "Genre",
            "Track #",
        ] {
            assert!(
                text.contains(label),
                "missing field label {label} in:\n{text}"
            );
        }
        assert!(text.contains("My Title"));
        assert!(text.contains("2020"));
        assert!(text.contains("Jazz"));
    }

    // T016: multi-track render shows ONLY album fields + mixed placeholder. --
    #[test]
    fn test_tag_editor_render_multi_shows_only_album_fields_and_mixed() {
        // Two tracks share the album but have different years -> Year is mixed.
        let a = make_update(
            "Song A",
            "Artist A",
            Some("Shared"),
            Some("VA"),
            Some(2000),
            Some("Rock"),
            Some(1),
        );
        let b = make_update(
            "Song B",
            "Artist B",
            Some("Shared"),
            Some("VA"),
            Some(2001),
            Some("Rock"),
            Some(2),
        );
        let state = TagEditorState::multi(vec![1, 2], &[a, b]);
        let text = render_to_text(&state, 80, 24);

        // Album-level fields present.
        for label in ["Album", "Album Artist", "Year", "Genre"] {
            assert!(text.contains(label), "missing album field {label}");
        }
        // Track-only fields absent (Title/Artist/Track # not in album-level set).
        assert!(
            !text.contains("Track #"),
            "multi editor must not show Track #"
        );
        // Shared album value shown, mixed year shown as placeholder.
        assert!(text.contains("Shared"));
        assert!(
            text.contains(MIXED_PLACEHOLDER),
            "expected mixed placeholder for year"
        );
    }

    // T017: year validation rejects non-numeric and out-of-range. -----------
    #[test]
    fn test_year_validation_blocks_save() {
        let update = make_update("t", "a", None, None, None, None, None);

        // Non-numeric.
        let mut s = TagEditorState::single(1, update.clone());
        // focus Year
        while s.focused_field() != Field::Year {
            s.focus_next();
        }
        for c in "abcd".chars() {
            s.push_char(c);
        }
        assert!(!s.is_valid(), "non-numeric year should be invalid");
        assert!(s.validation_error.is_some());

        // Out of range (too low).
        let mut s = TagEditorState::single(1, update.clone());
        while s.focused_field() != Field::Year {
            s.focus_next();
        }
        for c in "1800".chars() {
            s.push_char(c);
        }
        assert!(!s.is_valid(), "1800 is out of range");

        // Out of range (far future).
        let mut s = TagEditorState::single(1, update.clone());
        while s.focused_field() != Field::Year {
            s.focus_next();
        }
        let future = format!("{}", current_year() + 5);
        for c in future.chars() {
            s.push_char(c);
        }
        assert!(!s.is_valid(), "far-future year is out of range");

        // Valid in-range year.
        let mut s = TagEditorState::single(1, update);
        while s.focused_field() != Field::Year {
            s.focus_next();
        }
        for c in "2000".chars() {
            s.push_char(c);
        }
        assert!(s.is_valid(), "2000 should be valid");
        assert!(s.validation_error.is_none());
    }

    // T018: only dirty fields differ from prefill in the built update. -------
    #[test]
    fn test_only_dirty_fields_change() {
        let update = make_update(
            "Orig Title",
            "Orig Artist",
            Some("Orig Album"),
            Some("Orig AA"),
            Some(1999),
            Some("Blues"),
            Some(7),
        );
        let mut s = TagEditorState::single(1, update);

        // Edit ONLY the title.
        while s.focused_field() != Field::Title {
            s.focus_next();
        }
        // Clear and retype.
        for _ in 0.."Orig Title".len() {
            s.backspace();
        }
        for c in "New Title".chars() {
            s.push_char(c);
        }

        let built = s.build_update();
        // Dirty field changed.
        assert_eq!(built.title, "New Title");
        // Untouched fields keep prefill values.
        assert_eq!(built.artist_name, "Orig Artist");
        assert_eq!(built.album_title.as_deref(), Some("Orig Album"));
        assert_eq!(built.album_artist_name.as_deref(), Some("Orig AA"));
        assert_eq!(built.year, Some(1999));
        assert_eq!(built.genre_names, vec!["Blues".to_string()]);
        assert_eq!(built.track_number, Some(7));
    }
}
