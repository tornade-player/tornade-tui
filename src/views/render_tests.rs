/// Visual render tests using ratatui's TestBackend.
///
/// Each test renders a view with mock data into an in-memory terminal buffer,
/// then writes the result to /tmp/tui_render_<name>.txt.
///
/// Legend in output:
///   ▓  = colored background (image placeholder, selection highlight)
///   <x> = cyan+bold character (selected item text)
///
/// Run: cargo test render -- --nocapture
#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    use ratatui_image::picker::Picker;
    use std::path::PathBuf;
    use std::time::Duration;
    use tornade_core::models::{Album, Artist, AudioFormat, Genre, Rating, Track};
    use tornade_core::models::playlist::Playlist;

    use crate::views::{
        AlbumsState, ArtistsState, GenresState, LibraryState, PlaylistsState,
    };
    use crate::views::search::{SearchSection, SearchState};
    use crate::views::queue::QueueState;
    use crate::views::scan::ScanState;

    // ─── Helpers ─────────────────────────────────────────────────────────────

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
                    out.push('▓');
                } else if is_cyan && is_bold {
                    out.push('<');
                    if ch.chars().all(|c| c.is_control() || c == '\0') { out.push('·'); }
                    else { out.push_str(ch); }
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

    fn render<F: FnOnce(&mut ratatui::Frame, Rect)>(
        w: u16, h: u16, f: F,
    ) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| f(frame, Rect::new(0, 0, w, h))).unwrap();
        terminal
    }

    // ─── Mock data ───────────────────────────────────────────────────────────

    fn make_track(id: i64, title: &str, artist: &str, secs: u64) -> Track {
        Track {
            id,
            title: title.into(),
            album_id: Some(1),
            artist_id: 1,
            source_id: 1,
            file_path: PathBuf::from(format!("/music/{}.flac", id)),
            duration: Duration::from_secs(secs),
            track_number: Some(id as u32),
            disc_number: 1,
            sample_rate: Some(44100),
            bit_depth: Some(16),
            file_type: AudioFormat::Flac,
            file_size: 30_000_000,
            rating: Rating(0),
            fingerprint: None,
            is_duplicate: false,
            duplicate_of: None,
            last_played_at: None,
            play_count: 0,
            artist_names: vec![artist.into()],
        }
    }

    fn mock_tracks() -> Vec<Track> {
        vec![
            make_track(1, "Bohemian Rhapsody", "Queen", 354),
            make_track(2, "Hotel California", "Eagles", 391),
            make_track(3, "Stairway to Heaven", "Led Zeppelin", 482),
            make_track(4, "Comfortably Numb", "Pink Floyd", 382),
            make_track(5, "Smells Like Teen Spirit", "Nirvana", 301),
            make_track(6, "Purple Haze", "Jimi Hendrix", 170),
            make_track(7, "Like a Rolling Stone", "Bob Dylan", 369),
            make_track(8, "Johnny B. Goode", "Chuck Berry", 162),
        ]
    }

    fn mock_artists() -> Vec<Artist> {
        vec![
            "Queen", "Eagles", "Led Zeppelin", "Pink Floyd",
            "Nirvana", "Jimi Hendrix", "Bob Dylan", "Chuck Berry",
            "The Beatles", "The Rolling Stones", "David Bowie", "Radiohead",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, name)| Artist {
            id: i as i64 + 1,
            name: name.into(),
            name_sort: None, bio: None, country: None, genre: None,
            style: None, mood: None, formed_year: None, born_year: None,
            died_year: None, disbanded: None, musicbrainz_id: None,
            theaudiodb_id: None, photo_path: None,
        })
        .collect()
    }

    fn mock_albums() -> Vec<Album> {
        vec![
            ("A Night at the Opera", "Queen", 1975),
            ("Hotel California", "Eagles", 1976),
            ("Led Zeppelin IV", "Led Zeppelin", 1971),
            ("The Dark Side of the Moon", "Pink Floyd", 1973),
            ("Nevermind", "Nirvana", 1991),
            ("Are You Experienced", "Jimi Hendrix", 1967),
            ("Abbey Road", "The Beatles", 1969),
            ("OK Computer", "Radiohead", 1997),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (title, artist, year))| Album {
            id: i as i64 + 1,
            title: title.into(),
            artist_id: i as i64 + 1,
            artist_name: artist.into(),
            year: Some(year),
            rating: Rating(0),
            artwork_path: None,
            online_artwork_path: None,
            description: None,
            musicbrainz_id: None,
            label: None,
            country: None,
            barcode: None,
            album_type: None,
            release_status: None,
        })
        .collect()
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

    fn mock_playlists() -> Vec<Playlist> {
        vec![
            Playlist { id: 1, name: "Favourites".into(), description: None,
                tracks: vec![1,2,3], created_at: "2024-01-01".into(), updated_at: "2024-01-01".into() },
            Playlist { id: 2, name: "Road Trip".into(), description: Some("Best for driving".into()),
                tracks: vec![4,5,6,7], created_at: "2024-01-01".into(), updated_at: "2024-01-01".into() },
            Playlist { id: 3, name: "Chill".into(), description: None,
                tracks: vec![8], created_at: "2024-01-01".into(), updated_at: "2024-01-01".into() },
        ]
    }

    // ─── Library (tracks list) ────────────────────────────────────────────────

    #[test]
    fn render_library() {
        let mut state = LibraryState::default();
        state.tracks = mock_tracks();
        state.total_count = state.tracks.len() as i64;
        state.list_state.select(Some(0));

        let t = render(80, 30, |frame, area| state.render(frame, area, true));
        let out = dump_buffer(&t);
        dump_to_file("library_80x30", &format!("=== Library (80x30, focused, selected=0) ===\n{out}"));
    }

    #[test]
    fn render_library_filter() {
        let mut state = LibraryState::default();
        state.tracks = mock_tracks();
        state.total_count = state.tracks.len() as i64;
        state.filter = "queen".into();
        state.filter_active = true;
        state.list_state.select(Some(0));

        let t = render(80, 30, |frame, area| state.render(frame, area, true));
        let out = dump_buffer(&t);
        dump_to_file("library_filter", &format!("=== Library (filter='queen') ===\n{out}"));
    }

    // ─── Albums grid ─────────────────────────────────────────────────────────

    #[test]
    fn render_albums_grid() {
        let mut state = AlbumsState::default();
        state.albums = mock_albums();
        let mut picker = Picker::halfblocks();

        let t = render(80, 30, |frame, area| { state.render(frame, area, true, &mut picker, std::path::Path::new("/tmp")); });
        let out = dump_buffer(&t);
        dump_to_file("albums_80x30", &format!("=== Albums grid (80x30, selected=0) ===\n{out}"));
    }

    #[test]
    fn render_albums_grid_selected() {
        let mut state = AlbumsState::default();
        state.albums = mock_albums();
        state.selected = 5;
        let mut picker = Picker::halfblocks();

        let t = render(80, 30, |frame, area| { state.render(frame, area, true, &mut picker, std::path::Path::new("/tmp")); });
        let out = dump_buffer(&t);
        dump_to_file("albums_selected5", &format!("=== Albums grid (selected=5) ===\n{out}"));
    }

    // ─── Artists grid ─────────────────────────────────────────────────────────

    #[test]
    fn render_artists_grid() {
        let mut state = ArtistsState::default();
        state.artists = mock_artists();
        let mut picker = Picker::halfblocks();

        let t = render(80, 30, |frame, area| { state.render(frame, area, true, &mut picker, std::path::Path::new("/tmp")); });
        let out = dump_buffer(&t);
        dump_to_file("artists_80x30", &format!("=== Artists grid (80x30, selected=0) ===\n{out}"));
    }

    #[test]
    fn render_artists_grid_selected_middle() {
        let mut state = ArtistsState::default();
        state.artists = mock_artists();
        state.selected = 5;
        let mut picker = Picker::halfblocks();

        let t = render(80, 30, |frame, area| { state.render(frame, area, true, &mut picker, std::path::Path::new("/tmp")); });
        let out = dump_buffer(&t);
        dump_to_file("artists_selected5", &format!("=== Artists grid (selected=5) ===\n{out}"));
    }

    #[test]
    fn render_artists_filter() {
        let mut state = ArtistsState::default();
        state.artists = mock_artists();
        state.filter = "the".into();
        state.filter_active = true;
        let mut picker = Picker::halfblocks();

        let t = render(80, 30, |frame, area| { state.render(frame, area, true, &mut picker, std::path::Path::new("/tmp")); });
        let out = dump_buffer(&t);
        dump_to_file("artists_filter", &format!("=== Artists grid (filter='the') ===\n{out}"));
    }

    // ─── Genres list ──────────────────────────────────────────────────────────

    #[test]
    fn render_genres() {
        let mut state = GenresState::default();
        state.genres = mock_genres();
        let mut picker = Picker::halfblocks();

        let t = render(80, 30, |frame, area| { state.render(frame, area, true, &mut picker, std::path::Path::new("/tmp")); });
        let out = dump_buffer(&t);
        dump_to_file("genres_80x30", &format!("=== Genres (80x30, no images) ===\n{out}"));
    }

    #[test]
    fn render_genres_filter() {
        let mut state = GenresState::default();
        state.genres = mock_genres();
        state.filter = "rock".into();
        state.filter_active = true;
        let mut picker = Picker::halfblocks();

        let t = render(80, 30, |frame, area| { state.render(frame, area, true, &mut picker, std::path::Path::new("/tmp")); });
        let out = dump_buffer(&t);
        dump_to_file("genres_filter", &format!("=== Genres (filter='rock') ===\n{out}"));
    }

    // ─── Playlists ────────────────────────────────────────────────────────────

    #[test]
    fn render_playlists() {
        let mut state = PlaylistsState::default();
        state.playlists = mock_playlists();
        state.list_state.select(Some(0));

        let t = render(80, 30, |frame, area| state.render(frame, area, true));
        let out = dump_buffer(&t);
        dump_to_file("playlists_80x30", &format!("=== Playlists (80x30) ===\n{out}"));
    }

    // ─── Queue ────────────────────────────────────────────────────────────────

    #[test]
    fn render_queue() {
        let mut state = QueueState::default();
        let tracks = mock_tracks();

        let t = render(60, 30, |frame, area| {
            state.render(frame, area, &tracks, 2, &[], true);
        });
        let out = dump_buffer(&t);
        dump_to_file("queue_60x30", &format!("=== Queue (60x30, active=2) ===\n{out}"));
    }

    // Queue filter lives in AppState.queue_filter, not QueueState - skip filter test here

    // ─── Search ───────────────────────────────────────────────────────────────

    #[test]
    fn render_search_empty() {
        let mut state = SearchState::default();

        let t = render(80, 30, |frame, area| state.render(frame, area, true));
        let out = dump_buffer(&t);
        dump_to_file("search_empty", &format!("=== Search (empty) ===\n{out}"));
    }

    #[test]
    fn render_search_results() {
        let mut state = SearchState::default();
        state.query = "rock".into();
        state.tracks = mock_tracks();
        state.albums = mock_albums();
        state.artists = mock_artists();
        state.section = SearchSection::Tracks;
        state.tracks_state.select(Some(0));

        let t = render(80, 30, |frame, area| state.render(frame, area, true));
        let out = dump_buffer(&t);
        dump_to_file("search_results", &format!("=== Search (results, tracks tab) ===\n{out}"));
    }

    #[test]
    fn render_search_albums_tab() {
        let mut state = SearchState::default();
        state.query = "rock".into();
        state.tracks = mock_tracks();
        state.albums = mock_albums();
        state.artists = mock_artists();
        state.section = SearchSection::Albums;
        state.albums_state.select(Some(0));

        let t = render(80, 30, |frame, area| state.render(frame, area, true));
        let out = dump_buffer(&t);
        dump_to_file("search_albums", &format!("=== Search (albums tab) ===\n{out}"));
    }

    // ─── Scan ─────────────────────────────────────────────────────────────────

    #[test]
    fn render_scan() {
        let state = ScanState::new(PathBuf::from("/Users/thomas/Music"));

        let t = render(80, 20, |frame, area| state.render(frame, area));
        let out = dump_buffer(&t);
        dump_to_file("scan_80x20", &format!("=== Scan (initial state) ===\n{out}"));
    }
}
