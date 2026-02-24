// Application state management
// Uses tornade-core services directly (TUI has direct Rust access)

use std::time::Duration;
use tornade_core::{db, models::AudioFormat, services::SearchService, utils::AppPaths};

// Simplified track for TUI display
#[derive(Debug, Clone)]
pub struct TuiTrack {
    #[expect(dead_code, reason = "Stored for future navigation to track detail")]
    pub id: i64,
    pub title: String,
    pub artist_id: i64,
    #[expect(dead_code, reason = "Stored for future navigation to album detail")]
    pub album_id: Option<i64>,
    pub duration: Duration,
    pub file_type: AudioFormat,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
}

#[derive(Debug)]
pub enum View {
    Library,
    Albums,
    #[expect(dead_code, reason = "Search view navigation not yet wired up")]
    Search,
}

pub struct App {
    pub view: View,
    pub tracks: Vec<TuiTrack>,
    pub selected_index: usize,
    pub status_message: String,
    pub search_query: String,
    pub search_mode: bool,
    pub album_count: i64,
    pub artist_count: i64,
    pub track_count: i64,
    // Services
    pool: db::DbPool,
    search_service: SearchService,
}

impl App {
    pub fn new() -> Self {
        // Initialize application paths
        let app_paths = match AppPaths::new() {
            Ok(paths) => paths,
            Err(e) => panic!("Failed to initialize app paths: {e}"),
        };

        // Create database connection pool
        let pool = match db::create_pool(app_paths.database_path()) {
            Ok(pool) => pool,
            Err(e) => panic!("Failed to create database pool: {e}"),
        };

        // Initialize database schema
        if let Err(e) = db::initialize_database(&pool) {
            panic!("Failed to initialize database: {e}");
        }

        let search_service = SearchService::new(pool.clone());

        let mut app = App {
            view: View::Library,
            tracks: Vec::new(),
            selected_index: 0,
            status_message: String::from(
                "Welcome to Tornade TUI! Press 'q' to quit, '/' to search",
            ),
            search_query: String::new(),
            search_mode: false,
            album_count: 0,
            artist_count: 0,
            track_count: 0,
            pool,
            search_service,
        };

        // Load initial data
        app.load_stats();
        app.load_tracks();

        app
    }

    // T106: Load library stats
    pub fn load_stats(&mut self) {
        let conn = match self.pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                self.status_message = format!("Failed to get connection: {e}");
                return;
            }
        };

        // Query counts directly
        let album_count: Result<i64, _> =
            conn.query_row("SELECT COUNT(*) FROM albums", [], |row| row.get(0));
        let artist_count: Result<i64, _> =
            conn.query_row("SELECT COUNT(*) FROM artists", [], |row| row.get(0));
        let track_count: Result<i64, _> =
            conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0));

        match (album_count, artist_count, track_count) {
            (Ok(albums), Ok(artists), Ok(tracks)) => {
                self.album_count = albums;
                self.artist_count = artists;
                self.track_count = tracks;
                self.status_message =
                    format!("Library: {tracks} tracks, {albums} albums, {artists} artists");
            }
            _ => {
                self.status_message = String::from("Failed to load library stats");
            }
        }
    }

    // T107: Load tracks
    pub fn load_tracks(&mut self) {
        let conn = match self.pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                self.status_message = format!("Failed to get connection: {e}");
                return;
            }
        };

        let query = "SELECT id, title, artist_id, album_id, duration, file_type, \
                     sample_rate, bit_depth \
                     FROM tracks ORDER BY title LIMIT 100";

        match conn.prepare(query) {
            Ok(mut stmt) => {
                let tracks_iter = stmt.query_map([], |row| {
                    let duration_ms: u64 = row.get(4)?;
                    let file_type_str: String = row.get(5)?;

                    Ok(TuiTrack {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        artist_id: row.get(2)?,
                        album_id: row.get(3)?,
                        duration: Duration::from_millis(duration_ms),
                        file_type: AudioFormat::from_str(&file_type_str)
                            .unwrap_or(AudioFormat::Flac),
                        sample_rate: row.get(6)?,
                        bit_depth: row.get(7)?,
                    })
                });

                match tracks_iter {
                    Ok(tracks) => {
                        let tracks_vec: Result<Vec<_>, _> = tracks.collect();
                        match tracks_vec {
                            Ok(tracks_data) => {
                                self.tracks = tracks_data;
                                if self.tracks.is_empty() {
                                    self.status_message = String::from(
                                        "No tracks found. Scan a library folder first.",
                                    );
                                } else {
                                    self.status_message =
                                        format!("Loaded {} tracks", self.tracks.len());
                                }
                            }
                            Err(e) => {
                                self.status_message = format!("Failed to fetch tracks: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to query tracks: {e}");
                    }
                }
            }
            Err(e) => {
                self.status_message = format!("Failed to prepare query: {e}");
            }
        }
    }

    // T108: Play selected track (placeholder - player not implemented in TUI)
    pub fn play_selected(&mut self) {
        if let Some(track) = self.tracks.get(self.selected_index) {
            self.status_message = format!(
                "▶ Would play: {} (Player not implemented in TUI)",
                track.title
            );
        }
    }

    // T109: Toggle playback (placeholder)
    pub fn toggle_playback(&mut self) {
        self.status_message = String::from("⏸ Player controls not implemented in TUI");
    }

    // Search tracks
    pub fn search(&mut self) {
        if self.search_query.is_empty() {
            return;
        }

        match self.search_service.search(&self.search_query) {
            Ok(results) => {
                self.tracks = results
                    .tracks
                    .into_iter()
                    .take(50)
                    .map(|t| TuiTrack {
                        id: t.id,
                        title: t.title,
                        artist_id: t.artist_id,
                        album_id: t.album_id,
                        duration: t.duration,
                        file_type: t.file_type,
                        sample_rate: t.sample_rate,
                        bit_depth: t.bit_depth,
                    })
                    .collect();
                self.selected_index = 0;
                self.status_message = format!(
                    "Found {} tracks for '{}'",
                    self.tracks.len(),
                    self.search_query
                );
            }
            Err(e) => {
                self.status_message = format!("Search error: {e}");
            }
        }
    }

    // Navigation
    pub fn next_item(&mut self) {
        if !self.tracks.is_empty() && self.selected_index < self.tracks.len() - 1 {
            self.selected_index += 1;
        }
    }

    pub fn previous_item(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn refresh_state() {
        // Placeholder for periodic state refresh
    }
}
