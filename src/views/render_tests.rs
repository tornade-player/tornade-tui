/// Visual render tests using ratatui's TestBackend.
///
/// These tests render views with mock data into an in-memory buffer and print
/// the result as text. Run with:
///   cargo test render -- --nocapture
///
/// Images won't appear (halfblocks fallback), but layout, text positions,
/// selection highlight and truncation are all visible.
#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    use ratatui_image::picker::Picker;
    use tornade_core::models::{Artist, Genre};

    use crate::views::{ArtistsState, GenresState};

    /// Render the buffer to a human-readable string: one line per terminal row,
    /// with a leading row index.
    fn dump_to_file(name: &str, content: &str) {
        let path = format!("/tmp/tui_render_{}.txt", name);
        std::fs::write(&path, content).unwrap();
    }

    fn dump_buffer(terminal: &Terminal<TestBackend>) -> String {
        use ratatui::style::{Color, Modifier};
        let buf = terminal.backend().buffer();
        let width = buf.area.width as usize;
        let height = buf.area.height as usize;
        let mut out = String::new();
        for row in 0..height {
            out.push_str(&format!("{:02} │", row));
            for col in 0..width {
                let cell = buf.cell((col as u16, row as u16)).unwrap();
                let ch = cell.symbol();
                let has_bg = !matches!(cell.bg, Color::Reset);
                let is_cyan = matches!(cell.fg, Color::Cyan);
                let is_bold = cell.modifier.contains(Modifier::BOLD);

                if has_bg && ch.chars().all(|c| c == ' ' || c == '\0') {
                    // Colored background, empty content → show as block
                    out.push('▓');
                } else if is_cyan && is_bold {
                    // Selected text → wrap in angle brackets
                    out.push('<');
                    if ch.chars().all(|c| c.is_control() || c == '\0') {
                        out.push('·');
                    } else {
                        out.push_str(ch);
                    }
                    out.push('>');
                } else if ch.chars().all(|c| c.is_control() || c == '\0') {
                    out.push(' ');
                } else {
                    out.push_str(ch);
                }
            }
            out.push('\n');
        }
        out
    }

    fn mock_artists() -> Vec<Artist> {
        (1..=12i64).map(|i| Artist {
            id: i,
            name: format!("Artist {:02}", i),
            name_sort: None,
            bio: None,
            country: None,
            genre: None,
            style: None,
            mood: None,
            formed_year: None,
            born_year: None,
            died_year: None,
            disbanded: None,
            musicbrainz_id: None,
            theaudiodb_id: None,
            photo_path: None,
        }).collect()
    }

    fn mock_genres() -> Vec<(Genre, u32, u32)> {
        vec![
            (Genre { id: 1, name: "Rock".into() }, 42, 8),
            (Genre { id: 2, name: "Jazz".into() }, 18, 3),
            (Genre { id: 3, name: "Electronic".into() }, 75, 12),
            (Genre { id: 4, name: "Classical".into() }, 30, 6),
            (Genre { id: 5, name: "Hip-Hop".into() }, 55, 9),
        ]
    }

    #[test]
    fn render_artists_grid_no_photos() {
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let mut picker = Picker::halfblocks();
        let mut state = ArtistsState::default();
        state.artists = mock_artists();

        terminal.draw(|frame| {
            let area = Rect::new(0, 0, 80, 30);
            state.render(frame, area, true, &mut picker);
        }).unwrap();

        let output = dump_buffer(&terminal);
        dump_to_file("artists_80x30", &format!("=== Artists grid (80x30, focused, no photos) ===\n{}", output));
    }

    #[test]
    fn render_artists_grid_selected_middle() {
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let mut picker = Picker::halfblocks();
        let mut state = ArtistsState::default();
        state.artists = mock_artists();
        state.selected = 5;

        terminal.draw(|frame| {
            let area = Rect::new(0, 0, 80, 30);
            state.render(frame, area, true, &mut picker);
        }).unwrap();

        let output = dump_buffer(&terminal);
        dump_to_file("artists_selected5", &format!("=== Artists grid (selected=5) ===\n{}", output));
    }

    #[test]
    fn render_artists_grid_narrow() {
        let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
        let mut picker = Picker::halfblocks();
        let mut state = ArtistsState::default();
        state.artists = mock_artists();

        terminal.draw(|frame| {
            let area = Rect::new(0, 0, 40, 24);
            state.render(frame, area, true, &mut picker);
        }).unwrap();

        let output = dump_buffer(&terminal);
        dump_to_file("artists_40x24", &format!("=== Artists grid (narrow 40x24) ===\n{}", output));
    }

    #[test]
    fn render_genres_list_no_images() {
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let mut picker = Picker::halfblocks();
        let mut state = GenresState::default();
        state.genres = mock_genres();

        terminal.draw(|frame| {
            let area = Rect::new(0, 0, 80, 30);
            state.render(frame, area, true, &mut picker);
        }).unwrap();

        let output = dump_buffer(&terminal);
        dump_to_file("genres_80x30", &format!("=== Genres list (80x30, no images) ===\n{}", output));
    }

    #[test]
    fn render_artists_filter_active() {
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let mut picker = Picker::halfblocks();
        let mut state = ArtistsState::default();
        state.artists = mock_artists();
        state.filter = "Art".into();
        state.filter_active = true;

        terminal.draw(|frame| {
            let area = Rect::new(0, 0, 80, 30);
            state.render(frame, area, true, &mut picker);
        }).unwrap();

        let output = dump_buffer(&terminal);
        dump_to_file("artists_filter", &format!("=== Artists grid (filter='Art') ===\n{}", output));
    }
}
