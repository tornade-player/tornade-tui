use crate::{
    async_worker::{AsyncJob, AsyncPayload, AsyncResult, AsyncWorker},
    navigation::NavigationStack,
    player::PlayerStateCache,
    views::queue::ToolbarHitZones,
    views::{
        AlbumsState, ArtistsState, GenresState, LibraryState, PlaylistDetailState, PlaylistsState,
        QueueState, SearchState, SidebarEntry, View,
    },
    widgets::artwork_menu::ArtworkMenuState,
    widgets::player_bar::PlayerHitZones,
    widgets::scrape_picker::{CandidateField, ScrapePickerState},
    widgets::tag_editor::{Field, TagEditorState},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;
use tornade_core::{
    db::DbPool,
    models::Track,
    services::{
        ArtworkService, LibraryService, MetadataEditService, PlaybackState, PlayerService,
        PlaylistService, SearchService,
    },
    utils::AppPaths,
};

/// Which of the three panels currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    Sidebar,
    Content,
    RightPanel,
}

/// Current keyboard input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    /// Vim-style `:` command input
    Command,
    /// Modal text input (rename, create, scan path, etc.)
    TextInput,
    /// Confirmation dialog (y/n)
    Confirm,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Error,
}

pub struct StatusMessage {
    pub text: String,
    pub kind: StatusKind,
    pub set_at: Instant,
}

/// Context for a pending text input dialog.
pub struct TextInputCtx {
    pub prompt: String,
    pub value: String,
    pub action: TextInputAction,
}

#[derive(Debug, Clone)]
pub enum TextInputAction {
    ScanPath,
    CreatePlaylist,
    RenamePlaylist {
        id: i64,
    },
    ImportM3u,
    SaveQueueAsPlaylist,
    /// Path to a local image file to set as artwork for the given track ids.
    SetArtworkFromFile {
        track_ids: Vec<i64>,
    },
}

/// Context for a pending confirmation dialog.
pub struct ConfirmCtx {
    pub prompt: String,
    pub action: ConfirmAction,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    ClearQueue,
    DeletePlaylist { id: i64 },
    RemoveFromQueue { position: usize },
    RemoveFromPlaylist { playlist_id: i64, position: usize },
    LibraryCleanup,
}

/// Multi-select state for the current track list (US2).
///
/// Selection applies to whichever track list is currently focused in the
/// content panel. It is cleared whenever the user leaves the current view or
/// the library reloads (FR-016).
#[derive(Debug, Default, Clone)]
pub struct Selection {
    /// Whether selection mode is active (`v` toggles it).
    pub selection_mode: bool,
    /// Track IDs currently marked as selected.
    pub selected_ids: HashSet<i64>,
    /// Anchor row index for potential range operations.
    pub anchor: Option<usize>,
}

impl Selection {
    /// Clear all selected ids, the anchor, and exit selection mode.
    pub fn clear(&mut self) {
        self.selection_mode = false;
        self.selected_ids.clear();
        self.anchor = None;
    }

    /// Number of selected tracks.
    pub fn count(&self) -> usize {
        self.selected_ids.len()
    }

    /// True when a non-empty selection exists.
    pub fn is_active(&self) -> bool {
        !self.selected_ids.is_empty()
    }

    /// Toggle the membership of `id` in the selection.
    pub fn toggle(&mut self, id: i64) {
        if !self.selected_ids.remove(&id) {
            self.selected_ids.insert(id);
        }
    }
}

pub struct AppState {
    // Core services
    pub player: PlayerService,
    pub library: LibraryService,
    pub playlists: PlaylistService,
    pub search_svc: SearchService,
    pub metadata_edit: MetadataEditService,
    #[allow(dead_code)] // held to keep the service alive (background artwork downloads)
    pub artwork: ArtworkService,
    /// DB connection pool, used for direct artwork queries (US3).
    pub pool: DbPool,
    pub paths: AppPaths,
    pub picker: Picker,

    // Navigation
    pub nav: NavigationStack,
    pub sidebar_entry: SidebarEntry,

    // Player state (polled every 250ms)
    pub player_cache: PlayerStateCache,

    // Input mode
    pub input_mode: InputMode,
    pub command_input: String,
    pub command_completions: Vec<String>,
    pub text_input: Option<TextInputCtx>,
    pub confirm: Option<ConfirmCtx>,

    // UI overlays
    pub show_help: bool,
    pub show_stats: bool,
    pub stats_lines: Vec<(String, String)>,
    pub show_playlist_selector: bool,
    pub playlist_selector_state: ratatui::widgets::ListState,
    /// Tag editor overlay state; `Some` when the editor is open.
    pub tag_editor: Option<TagEditorState>,
    /// Online scrape picker overlay state; `Some` when open (US3).
    pub scrape_picker: Option<ScrapePickerState>,
    /// Artwork action menu overlay state; `Some` when open (US3).
    pub artwork_menu: Option<ArtworkMenuState>,
    /// Worker thread handle for async MusicBrainz / artwork calls (US3).
    pub async_worker: AsyncWorker,

    // Panel focus
    pub focused_panel: FocusedPanel,
    pub sidebar_cursor: usize,

    // Right panel queue scroll state (persists across frames for smooth scrolling)
    pub right_panel_queue_state: ratatui::widgets::ListState,

    // Queue track cache: resolved once when queue IDs change, not on every frame
    pub cached_queue_tracks: Vec<Track>,
    cached_queue_ids: Vec<i64>,

    // Sidebar playlist cache: refreshed on tick, used for the Playlists section
    pub sidebar_playlists: Vec<(i64, String)>,

    // Multi-select state for the current track list (US2)
    pub selection: Selection,

    // Status bar
    pub status: Option<StatusMessage>,

    // Set to true when background image loads are in progress; drives faster poll timeout
    pub has_pending_images: bool,

    // Queue filter (right panel)
    pub queue_filter: String,
    pub queue_filter_active: bool,

    // Player artwork + album name (refreshed when track changes)
    pub current_album_name: Option<String>,
    pub player_artwork: Option<StatefulProtocol>,
    player_artwork_track_id: Option<i64>,

    // Hit zones for player transport buttons (updated each frame)
    pub player_hit_zones: PlayerHitZones,
    // Hit zones for the toolbar buttons above the player (updated each frame)
    pub toolbar_hit_zones: ToolbarHitZones,

    // Click areas updated each frame by ui::draw (used for mouse hit detection)
    pub sidebar_area: Option<ratatui::layout::Rect>,
    pub right_queue_area: Option<ratatui::layout::Rect>,
    // Double-click detection: last click (col, row, instant)
    pub last_click: Option<(u16, u16, std::time::Instant)>,

    // Target pixel size for grid thumbnails (computed from font metrics at startup)
    pub tui_target: (u32, u32),
}

impl AppState {
    #[allow(clippy::too_many_arguments)] // services are injected individually by design
    pub fn new(
        player: PlayerService,
        library: LibraryService,
        playlists: PlaylistService,
        search_svc: SearchService,
        metadata_edit: MetadataEditService,
        artwork: ArtworkService,
        pool: DbPool,
        paths: AppPaths,
        picker: Picker,
        tui_target: (u32, u32),
    ) -> Self {
        let mut state = Self {
            player,
            library,
            playlists,
            search_svc,
            metadata_edit,
            artwork,
            pool,
            paths,
            picker,
            nav: NavigationStack::new(View::Library(LibraryState::default())),
            sidebar_entry: SidebarEntry::Tracks,
            player_cache: PlayerStateCache::default(),
            input_mode: InputMode::Normal,
            command_input: String::new(),
            command_completions: Vec::new(),
            text_input: None,
            confirm: None,
            show_help: false,
            show_stats: false,
            stats_lines: Vec::new(),
            show_playlist_selector: false,
            playlist_selector_state: ratatui::widgets::ListState::default(),
            tag_editor: None,
            scrape_picker: None,
            artwork_menu: None,
            async_worker: AsyncWorker::spawn(),
            focused_panel: FocusedPanel::Content,
            sidebar_cursor: 0,
            right_panel_queue_state: ratatui::widgets::ListState::default(),
            cached_queue_tracks: Vec::new(),
            cached_queue_ids: Vec::new(),
            sidebar_playlists: Vec::new(),
            selection: Selection::default(),
            status: None,
            has_pending_images: false,
            queue_filter: String::new(),
            queue_filter_active: false,
            current_album_name: None,
            player_artwork: None,
            player_artwork_track_id: None,
            player_hit_zones: PlayerHitZones::default(),
            toolbar_hit_zones: ToolbarHitZones::default(),
            sidebar_area: None,
            right_queue_area: None,
            last_click: None,
            tui_target,
        };
        state.reload_current_view();
        state.refresh_sidebar_playlists();
        state
    }

    /// Poll player state and clear stale status. Called every 500ms.
    /// Returns true if anything changed that needs a redraw.
    pub fn tick(&mut self) -> bool {
        let old_state = self.player_cache.state;
        let old_pos = self.player_cache.position as u64;
        let old_track = self.player_cache.current_track.as_ref().map(|t| t.id);
        let old_queue_len = self.player_cache.queue.len();
        let had_status = self.status.is_some();

        self.player_cache.poll(&self.player);

        // Auto-advance: when the current track finishes naturally, move to the
        // next one. `is_track_finished` only returns true while playing and past
        // the end, and suppresses itself while a new track is loading, so this
        // cannot double-skip. Mirrors the macOS GUI behaviour.
        if self.player.is_track_finished() {
            let _ = self.player.next();
            self.player_cache.poll(&self.player);
        }

        let status_cleared = if self
            .status
            .as_ref()
            .is_some_and(|s| s.set_at.elapsed().as_secs() >= 3)
        {
            self.status = None;
            true
        } else {
            false
        };

        let skipped = self.player_cache.skipped_track_ids.clone();
        self.apply_skipped_ids(&skipped);
        self.refresh_queue_cache();
        self.refresh_sidebar_playlists();
        self.refresh_player_artwork();

        // Only signal redraw if something visible changed
        let new_state = self.player_cache.state;
        let new_pos = self.player_cache.position as u64;
        let new_track = self.player_cache.current_track.as_ref().map(|t| t.id);
        let new_queue_len = self.player_cache.queue.len();

        old_state != new_state
            || old_pos != new_pos
            || old_track != new_track
            || old_queue_len != new_queue_len
            || status_cleared
            || (had_status && self.status.is_none())
    }

    fn refresh_player_artwork(&mut self) {
        let current_id = self.player_cache.current_track.as_ref().map(|t| t.id);
        if current_id == self.player_artwork_track_id {
            return;
        }
        self.player_artwork_track_id = current_id;
        let track = self.player_cache.current_track.clone();
        if let Some(ref track) = track
            && let Some(album_id) = track.album_id
            && let Ok(Some(album)) = self.library.get_album(album_id)
        {
            self.current_album_name = Some(album.title.clone());
            let tui_dir = crate::tui_artwork::tui_album_dir(&self.paths, self.tui_target);
            let path = album
                .online_artwork_path
                .as_ref()
                .map(|p| crate::tui_artwork::resolve_artwork_path(p, &tui_dir))
                .or_else(|| album.artwork_path.as_ref().cloned());
            self.player_artwork = path
                .and_then(|p| image::open(&p).ok())
                .map(|img| image::DynamicImage::ImageRgba8(img.to_rgba8()))
                .map(|img| self.picker.new_resize_protocol(img));
            return;
        }
        self.current_album_name = None;
        self.player_artwork = None;
    }

    /// Refresh sidebar playlist list; no-op if unchanged.
    pub fn refresh_sidebar_playlists(&mut self) {
        if let Ok(pls) = self.playlists.list_playlists() {
            let new: Vec<(i64, String)> = pls.into_iter().map(|p| (p.id, p.name)).collect();
            if new != self.sidebar_playlists {
                self.sidebar_playlists = new;
            }
        }
    }

    /// Navigate directly to a playlist by ID (for sidebar playlist clicks).
    pub fn navigate_to_playlist(&mut self, id: i64) {
        self.selection.clear();
        if let Ok(Some(pl)) = self.playlists.get_playlist(id) {
            let view = View::PlaylistDetail(PlaylistDetailState::new(pl, &self.library));
            self.nav.replace_root(view);
            self.sidebar_entry = SidebarEntry::Playlists;
        }
    }

    /// Re-resolve queue track objects only when the queue IDs have changed.
    pub fn refresh_queue_cache(&mut self) {
        if self.player_cache.queue == self.cached_queue_ids {
            return;
        }
        self.cached_queue_ids = self.player_cache.queue.clone();
        self.cached_queue_tracks = self
            .cached_queue_ids
            .iter()
            .filter_map(|&id| self.library.get_track(id).ok().flatten())
            .collect();
    }

    fn apply_skipped_ids(&mut self, skipped: &[i64]) {
        match self.nav.current_mut() {
            View::Library(s) => s.skipped_ids = skipped.to_vec(),
            View::AlbumDetail(s) => s.skipped_ids = skipped.to_vec(),
            View::GenreDetail(s) => s.skipped_ids = skipped.to_vec(),
            View::PlaylistDetail(s) => s.skipped_ids = skipped.to_vec(),
            _ => {}
        }
    }

    /// Cycle focus: Content → RightPanel → Sidebar → Content.
    pub fn cycle_focus(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Content => FocusedPanel::RightPanel,
            FocusedPanel::RightPanel => {
                self.sync_sidebar_cursor();
                FocusedPanel::Sidebar
            }
            FocusedPanel::Sidebar => FocusedPanel::Content,
        };
    }

    /// Cycle focus in reverse: Content → Sidebar → RightPanel → Content.
    pub fn cycle_focus_reverse(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Content => {
                self.sync_sidebar_cursor();
                FocusedPanel::Sidebar
            }
            FocusedPanel::Sidebar => FocusedPanel::RightPanel,
            FocusedPanel::RightPanel => FocusedPanel::Content,
        };
    }

    /// Sync sidebar cursor to the currently active view.
    pub fn sync_sidebar_cursor(&mut self) {
        let entry = self.nav.current().sidebar_entry();
        self.sidebar_cursor = match entry {
            SidebarEntry::Playlists => {
                // Point cursor at the active playlist if it's in the list
                let active_id = match self.nav.current() {
                    View::PlaylistDetail(s) => Some(s.playlist.id),
                    _ => None,
                };
                active_id
                    .and_then(|id| {
                        self.sidebar_playlists
                            .iter()
                            .position(|(pid, _)| *pid == id)
                    })
                    .map(|idx| SidebarEntry::all().len() + idx)
                    .unwrap_or(0)
            }
            other => SidebarEntry::all()
                .iter()
                .position(|e| *e == other)
                .unwrap_or(0),
        };
    }

    pub fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Some(StatusMessage {
            text: text.into(),
            kind,
            set_at: Instant::now(),
        });
    }

    pub fn navigate_to(&mut self, entry: SidebarEntry) {
        // Leaving the current view clears any active track selection (FR-016).
        self.selection.clear();
        self.sidebar_entry = entry;
        let view = match entry {
            SidebarEntry::Tracks => View::Library(LibraryState::default()),
            SidebarEntry::Albums => View::Albums(AlbumsState::default()),
            SidebarEntry::Artists => View::Artists(ArtistsState::default()),
            SidebarEntry::Genres => View::Genres(GenresState::default()),
            SidebarEntry::Playlists => View::Playlists(PlaylistsState::default()),
            SidebarEntry::Queue => View::Queue(QueueState::default()),
            SidebarEntry::Search => View::Search(SearchState::default()),
        };
        self.nav.replace_root(view);
        self.reload_current_view();
    }

    /// Run a DB search for the current view's filter and update search_results.
    /// Called on every filter keystroke (FTS5 is fast enough for interactive use).
    pub fn apply_view_search(&mut self) {
        let query = match self.nav.current() {
            View::Library(s) if s.filter_active && !s.filter.is_empty() => s.filter.clone(),
            View::Artists(s) if s.filter_active && !s.filter.is_empty() => s.filter.clone(),
            View::Albums(s) if s.filter_active && !s.filter.is_empty() => s.filter.clone(),
            View::Library(s) if s.filter.is_empty() => {
                let _ = s;
                String::new()
            }
            View::Artists(s) if s.filter.is_empty() => {
                let _ = s;
                String::new()
            }
            View::Albums(s) if s.filter.is_empty() => {
                let _ = s;
                String::new()
            }
            _ => return,
        };

        if query.is_empty() {
            match self.nav.current_mut() {
                View::Library(s) => {
                    s.search_results = None;
                    s.list_state
                        .select(if s.tracks.is_empty() { None } else { Some(0) });
                }
                View::Artists(s) => {
                    s.search_results = None;
                    s.list_state
                        .select(if s.artists.is_empty() { None } else { Some(0) });
                }
                View::Albums(s) => {
                    s.search_results = None;
                    s.selected = 0;
                    s.scroll_row = 0;
                }
                _ => {}
            }
            return;
        }

        let Ok(results) = self.search_svc.search(&query) else {
            return;
        };

        match self.nav.current_mut() {
            View::Library(s) => {
                s.list_state.select(if results.tracks.is_empty() {
                    None
                } else {
                    Some(0)
                });
                s.search_results = Some(results.tracks);
            }
            View::Artists(s) => {
                s.list_state.select(if results.artists.is_empty() {
                    None
                } else {
                    Some(0)
                });
                s.search_results = Some(results.artists);
            }
            View::Albums(s) => {
                s.selected = 0;
                s.scroll_row = 0;
                s.search_results = Some(results.albums);
            }
            _ => {}
        }
    }

    pub fn reload_current_view(&mut self) {
        match self.nav.current_mut() {
            View::Library(s) => s.load(&self.library),
            View::Albums(s) => s.load(&self.library),
            View::Artists(s) => s.load(&self.library),
            View::Genres(s) => s.load(&self.library),
            View::Playlists(s) => s.load(&self.playlists),
            _ => {}
        }
    }

    /// Track IDs currently visible in the focused track list, in display order.
    /// Returns an empty vec for views that are not track lists.
    pub fn current_visible_track_ids(&self) -> Vec<i64> {
        match self.nav.current() {
            View::Library(s) => s.visible_track_ids(),
            View::AlbumDetail(s) => s.visible_track_ids(),
            View::GenreDetail(s) => s.visible_track_ids(),
            View::PlaylistDetail(s) => s.visible_track_ids(),
            View::Search(s) => s.selected_track().map(|t| t.id).into_iter().collect(),
            _ => Vec::new(),
        }
    }

    /// Toggle selection membership for the highlighted track (selection mode only).
    pub fn toggle_selection_at_cursor(&mut self) {
        if !self.selection.selection_mode {
            return;
        }
        if let Some(id) = self.selected_track_id() {
            self.selection.toggle(id);
        }
    }

    /// Select every track in the current list (enables selection mode).
    pub fn select_all_current(&mut self) {
        let ids = self.current_visible_track_ids();
        if ids.is_empty() {
            return;
        }
        self.selection.selection_mode = true;
        self.selection.selected_ids = ids.into_iter().collect();
    }

    /// Add the current selection to `playlist_id`, reporting the outcome via status.
    pub fn add_selection_to_playlist(&mut self, playlist_id: i64) {
        let ids: Vec<i64> = self.selection.selected_ids.iter().copied().collect();
        if ids.is_empty() {
            return;
        }
        match self.playlists.add_tracks(playlist_id, ids) {
            Ok(result) => {
                self.set_status(
                    format!(
                        "{} added, {} already present",
                        result.added, result.already_present
                    ),
                    StatusKind::Success,
                );
                self.selection.clear();
            }
            Err(e) => self.set_status(format!("Error: {}", e), StatusKind::Error),
        }
    }

    /// Remove the current selection from the playlist shown in the detail view.
    pub fn remove_selection_from_playlist(&mut self) {
        let playlist_id = match self.nav.current() {
            View::PlaylistDetail(s) => s.playlist.id,
            _ => return,
        };
        let ids: Vec<i64> = self.selection.selected_ids.iter().copied().collect();
        if ids.is_empty() {
            return;
        }
        match self.playlists.remove_tracks(playlist_id, &ids) {
            Ok(count) => {
                self.set_status(
                    format!("Removed {count} track(s) from playlist"),
                    StatusKind::Success,
                );
                self.selection.clear();
                // Reload the playlist detail view so the change is reflected.
                if let Ok(Some(pl)) = self.playlists.get_playlist(playlist_id)
                    && let View::PlaylistDetail(s) = self.nav.current_mut()
                {
                    let sel = s.list_state.selected();
                    *s = PlaylistDetailState::new(pl, &self.library);
                    s.list_state.select(sel);
                }
            }
            Err(e) => self.set_status(format!("Error: {}", e), StatusKind::Error),
        }
    }

    /// Row index of the highlighted track in the current list view, if any.
    pub fn selected_track_index(&self) -> Option<usize> {
        match self.nav.current() {
            View::Library(s) => s.list_state.selected(),
            View::AlbumDetail(s) => s.list_state.selected(),
            View::GenreDetail(s) => s.list_state.selected(),
            View::PlaylistDetail(s) => s.list_state.selected(),
            _ => None,
        }
    }

    pub fn selected_track_id(&self) -> Option<i64> {
        match self.nav.current() {
            View::Library(s) => s.selected_track().map(|t| t.id),
            View::AlbumDetail(s) => s.selected_track().map(|t| t.id),
            View::GenreDetail(s) => s.selected_track().map(|t| t.id),
            View::PlaylistDetail(s) => s.selected_track().map(|t| t.id),
            View::Search(s) => s.selected_track().map(|t| t.id),
            _ => None,
        }
    }

    pub fn play_from_current_view(&mut self) {
        if let View::Queue(s) = self.nav.current() {
            let idx = s.list_state.selected().unwrap_or(0);
            let _ = self.player.jump_to_index(idx);
            if self.player_cache.state != PlaybackState::Playing {
                let _ = self.player.resume();
            }
            return;
        }
        let (ids, index) = match self.nav.current() {
            View::Library(s) => (s.visible_track_ids(), s.list_state.selected().unwrap_or(0)),
            View::AlbumDetail(s) => (s.visible_track_ids(), s.list_state.selected().unwrap_or(0)),
            View::GenreDetail(s) => (s.visible_track_ids(), s.list_state.selected().unwrap_or(0)),
            View::PlaylistDetail(s) => {
                (s.visible_track_ids(), s.list_state.selected().unwrap_or(0))
            }
            _ => return,
        };
        if ids.is_empty() {
            return;
        }
        crate::player::play_from_context(&mut self.player, ids, index);
    }

    pub fn add_selected_to_queue(&mut self) {
        if let Some(id) = self.selected_track_id() {
            match self.player.add_to_queue(vec![id]) {
                Ok(_) => self.set_status("Added to queue", StatusKind::Success),
                Err(e) => self.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
    }

    /// Add `count` random library tracks to the queue.
    pub fn add_random_to_queue(&mut self, count: usize) {
        match self.library.get_random_tracks(count) {
            Ok(ids) if !ids.is_empty() => {
                let n = ids.len();
                match self.player.add_to_queue(ids) {
                    Ok(_) => self
                        .set_status(format!("Added {n} random tracks to queue"), StatusKind::Success),
                    Err(e) => self.set_status(format!("Error: {e}"), StatusKind::Error),
                }
            }
            Ok(_) => self.set_status("No tracks available", StatusKind::Info),
            Err(e) => self.set_status(format!("Error: {e}"), StatusKind::Error),
        }
    }

    /// Export the currently open playlist to an M3U file.
    pub fn export_current_playlist(&mut self, path: std::path::PathBuf) {
        let pid = match self.nav.current() {
            View::PlaylistDetail(s) => Some(s.playlist.id),
            _ => None,
        };
        match pid {
            Some(id) => match self.playlists.export_m3u(id, &path) {
                Ok(_) => {
                    self.set_status(format!("Exported to {}", path.display()), StatusKind::Success)
                }
                Err(e) => self.set_status(format!("Error: {e}"), StatusKind::Error),
            },
            None => self.set_status("Open a playlist to export", StatusKind::Error),
        }
    }

    /// Rate the album currently open in album detail (0-5 stars).
    pub fn rate_current_album(&mut self, stars: u8) {
        let aid = match self.nav.current() {
            View::AlbumDetail(s) => Some(s.album.id),
            _ => None,
        };
        match aid {
            Some(id) => match self.library.rate_album(id, stars) {
                Ok(_) => self.set_status(format!("Rated album {stars}/5"), StatusKind::Success),
                Err(e) => self.set_status(format!("Error: {e}"), StatusKind::Error),
            },
            None => self.set_status("Open an album to rate it", StatusKind::Error),
        }
    }

    /// Compute library statistics and open the stats overlay.
    pub fn compute_stats(&mut self) {
        let albums = self
            .library
            .list_albums(None, None, None, None, None)
            .map(|v| v.len())
            .unwrap_or(0);
        let artists = self.library.list_artists().map(|v| v.len()).unwrap_or(0);

        let mut n_tracks = 0usize;
        let mut total = std::time::Duration::ZERO;
        let mut bytes = 0u64;
        if let Ok(sources) = self.library.list_sources() {
            for s in sources {
                if let Ok(tracks) = self.library.get_source_tracks(s.id) {
                    n_tracks += tracks.len();
                    for t in &tracks {
                        total += t.duration;
                        bytes += t.file_size;
                    }
                }
            }
        }

        let secs = total.as_secs();
        let (h, m) = (secs / 3600, (secs % 3600) / 60);
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        self.stats_lines = vec![
            ("Tracks".to_string(), n_tracks.to_string()),
            ("Albums".to_string(), albums.to_string()),
            ("Artists".to_string(), artists.to_string()),
            ("Total time".to_string(), format!("{h}h {m}m")),
            ("Library size".to_string(), format!("{gb:.2} GB")),
        ];
        self.show_stats = true;
    }

    /// Open the tag editor for the current target(s).
    ///
    /// If a multi-selection is active, opens the editor in album-level
    /// (multi-track) mode over the selected track IDs. Otherwise edits the
    /// highlighted row (single-track).
    pub fn open_tag_editor(&mut self) {
        if self.selection.is_active() {
            self.open_multi_tag_editor();
            return;
        }
        let Some(track_id) = self.selected_track_id() else {
            return;
        };
        match self.metadata_edit.current_track_update(track_id) {
            Ok(current) => {
                self.tag_editor = Some(crate::widgets::tag_editor::TagEditorState::single(
                    track_id, current,
                ));
            }
            Err(e) => self.set_status(format!("Cannot edit tags: {e}"), StatusKind::Error),
        }
    }

    /// Open the multi-track tag editor over the active selection.
    fn open_multi_tag_editor(&mut self) {
        // Preserve display order of the current list for stable prefill baseline.
        let ordered: Vec<i64> = self
            .current_visible_track_ids()
            .into_iter()
            .filter(|id| self.selection.selected_ids.contains(id))
            .collect();
        let ids: Vec<i64> = if ordered.is_empty() {
            self.selection.selected_ids.iter().copied().collect()
        } else {
            ordered
        };
        let mut currents = Vec::with_capacity(ids.len());
        for &id in &ids {
            match self.metadata_edit.current_track_update(id) {
                Ok(current) => currents.push(current),
                Err(e) => {
                    self.set_status(format!("Cannot edit tags: {e}"), StatusKind::Error);
                    return;
                }
            }
        }
        self.tag_editor = Some(crate::widgets::tag_editor::TagEditorState::multi(
            ids, &currents,
        ));
    }

    /// Persist the open tag editor and close it on success.
    ///
    /// Single target routes through `update_track`; multiple targets route
    /// through `update_tracks` (album-level only). After a successful save the
    /// current view is reloaded so the change appears without a rescan.
    pub fn save_tag_editor(&mut self) {
        let Some(editor) = self.tag_editor.as_ref() else {
            return;
        };
        if !editor.is_valid() {
            return; // save blocked while invalid (year field)
        }

        let targets = editor.targets.clone();
        let update = editor.build_update();

        let failed: Vec<i64> = if targets.len() == 1 {
            match self.metadata_edit.update_track(targets[0], &update) {
                Ok(r) if r.ok => Vec::new(),
                Ok(r) => vec![r.track_id],
                Err(e) => {
                    self.set_status(format!("Save failed: {e}"), StatusKind::Error);
                    return;
                }
            }
        } else {
            self.metadata_edit
                .update_tracks(&targets, &update)
                .into_iter()
                .filter(|r| !r.ok)
                .map(|r| r.track_id)
                .collect()
        };

        if failed.is_empty() {
            self.tag_editor = None;
            self.set_status("Tags saved", StatusKind::Success);
            self.reload_current_view_tracks();
        } else {
            let ids = failed
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            self.set_status(format!("Failed to save track(s): {ids}"), StatusKind::Error);
        }
    }

    /// Reload the current view's track data from the library so tag edits are
    /// reflected without a full rescan. Covers both the root list views and the
    /// pushed detail views.
    fn reload_current_view_tracks(&mut self) {
        match self.nav.current_mut() {
            View::Library(s) => s.load(&self.library),
            View::Albums(s) => s.load(&self.library),
            View::Artists(s) => s.load(&self.library),
            View::Genres(s) => s.load(&self.library),
            View::AlbumDetail(s) => s.reload(&self.library),
            View::GenreDetail(s) => s.reload(&self.library),
            View::PlaylistDetail(_) => {
                let pid = match self.nav.current() {
                    View::PlaylistDetail(s) => s.playlist.id,
                    _ => return,
                };
                if let Ok(Some(pl)) = self.playlists.get_playlist(pid)
                    && let View::PlaylistDetail(s) = self.nav.current_mut()
                {
                    let sel = s.list_state.selected();
                    *s = PlaylistDetailState::new(pl, &self.library);
                    s.list_state.select(sel);
                }
            }
            _ => {}
        }
    }

    pub fn rate_selected(&mut self, stars: u8) {
        if let Some(id) = self.selected_track_id() {
            match self.library.rate_track(id, stars) {
                Ok(_) => {
                    self.set_status(
                        format!(
                            "Rated: {}{}",
                            "★".repeat(stars as usize),
                            "☆".repeat(5 - stars as usize)
                        ),
                        StatusKind::Success,
                    );
                    self.reload_current_view();
                }
                Err(e) => self.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
    }

    // ── US3: online scrape + artwork ─────────────────────────────────────────

    /// Drain any completed async jobs, folding each into the UI state.
    ///
    /// Kept as a helper so the run loop can drive it without holding a borrow of
    /// `async_worker` across the `on_async_result` call. Returns true if at least
    /// one result was processed (caller should redraw).
    pub fn poll_async(&mut self) -> bool {
        let mut any = false;
        while let Some(result) = self.async_worker.try_recv() {
            self.on_async_result(result);
            any = true;
        }
        any
    }

    /// Fold a completed async result into the scrape picker / artwork menu.
    ///
    /// Stale results (whose `job_id` does not match the overlay currently waiting
    /// on them) are ignored.
    pub fn on_async_result(&mut self, result: AsyncResult) {
        // Scrape picker: match the job id and update status / candidates.
        if let Some(picker) = self.scrape_picker.as_mut()
            && let Some(status) = crate::async_worker::status_for(picker.job_id, &result)
        {
            match result.payload {
                AsyncPayload::Candidates(candidates) => picker.set_candidates(candidates),
                AsyncPayload::Failed(msg) => picker.set_failed(msg),
                AsyncPayload::Artwork(_) => picker.status = status,
            }
            return;
        }

        // Artwork menu: an online fetch resolved. Apply bytes to the targets.
        if self.artwork_menu.is_some() {
            match result.payload {
                AsyncPayload::Artwork(Some(bytes)) => {
                    let targets = self
                        .artwork_menu
                        .as_ref()
                        .map(|m| m.targets.clone())
                        .unwrap_or_default();
                    self.set_artwork_from_bytes(&targets, &bytes);
                    self.artwork_menu = None;
                }
                AsyncPayload::Artwork(None) => {
                    self.set_status("No artwork found online", StatusKind::Info);
                    self.artwork_menu = None;
                }
                AsyncPayload::Failed(msg) => {
                    self.set_status(format!("Artwork fetch failed: {msg}"), StatusKind::Error);
                    self.artwork_menu = None;
                }
                AsyncPayload::Candidates(_) => {}
            }
        }
    }

    /// Open the online scrape picker for the track currently in the tag editor.
    ///
    /// Uses the (possibly edited) title + artist from the editor as the query and
    /// submits a `ScrapeTrack` job, showing the picker in its searching state.
    pub fn open_scrape_for_current_edit(&mut self) {
        let Some(editor) = self.tag_editor.as_ref() else {
            return;
        };
        let title = editor.value(Field::Title).trim().to_string();
        let artist = editor.value(Field::Artist).trim().to_string();
        if title.is_empty() && artist.is_empty() {
            self.set_status("Nothing to search (empty title/artist)", StatusKind::Info);
            return;
        }
        let job_id = self
            .async_worker
            .submit(AsyncJob::ScrapeTrack { title, artist });
        self.scrape_picker = Some(ScrapePickerState::searching(job_id));
    }

    /// Open the artwork action menu.
    ///
    /// Targets are the active multi-selection if present, otherwise the single
    /// track currently in the tag editor.
    pub fn open_artwork_menu(&mut self) {
        let targets: Vec<i64> = if self.selection.is_active() {
            self.selection.selected_ids.iter().copied().collect()
        } else if let Some(editor) = self.tag_editor.as_ref() {
            editor.targets.clone()
        } else {
            Vec::new()
        };
        if targets.is_empty() {
            return;
        }
        self.artwork_menu = Some(ArtworkMenuState::new(targets));
    }

    /// Apply the accepted fields of the selected scrape candidate to the edited
    /// track, then close the picker and reload the current view.
    pub fn apply_scrape_candidate(&mut self) {
        let Some(picker) = self.scrape_picker.as_ref() else {
            return;
        };
        let Some(candidate) = picker.current() else {
            self.scrape_picker = None;
            return;
        };
        // Determine the single target track (scrape applies to the edited track).
        let Some(&track_id) = self.tag_editor.as_ref().and_then(|e| e.targets.first()) else {
            self.scrape_picker = None;
            return;
        };

        // Baseline: keep existing values, override only accepted fields that carry
        // a value in the candidate.
        let mut update = match self.metadata_edit.current_track_update(track_id) {
            Ok(u) => u,
            Err(e) => {
                self.set_status(format!("Cannot apply: {e}"), StatusKind::Error);
                self.scrape_picker = None;
                return;
            }
        };

        let accept = |f: CandidateField| picker.is_accepted(f) && picker.field_value(f).is_some();
        if accept(CandidateField::Title) {
            update.title = candidate.title.clone();
        }
        if accept(CandidateField::Artist) {
            update.artist_name = candidate.artist.clone();
        }
        if accept(CandidateField::AlbumArtist) {
            update.album_artist_name = candidate.album_artist.clone();
        }
        if accept(CandidateField::Album) {
            update.album_title = candidate.album.clone();
        }
        if accept(CandidateField::Year) {
            update.year = candidate.year;
        }
        if accept(CandidateField::Genre) {
            // GENRE (C1): the tag model is single-value; take the first genre only.
            update.genre_names = candidate.genres.first().cloned().into_iter().collect();
        }
        if accept(CandidateField::TrackNumber) {
            update.track_number = candidate.track_number;
        }

        match self.metadata_edit.update_track(track_id, &update) {
            Ok(r) if r.ok => {
                self.set_status("Applied online metadata", StatusKind::Success);
                self.reload_current_view_tracks();
                // Refresh the tag editor prefill so the user sees the applied values.
                if let Ok(current) = self.metadata_edit.current_track_update(track_id) {
                    self.tag_editor = Some(TagEditorState::single(track_id, current));
                }
            }
            Ok(_) => self.set_status("Apply failed", StatusKind::Error),
            Err(e) => self.set_status(format!("Apply failed: {e}"), StatusKind::Error),
        }
        self.scrape_picker = None;
    }

    /// Set artwork for `track_ids` from a local image file (mirrors
    /// `ffi.rs::set_track_artwork_from_path`): validate size, hash, copy into the
    /// artwork cache, and update the DB for each target.
    pub fn set_artwork_from_file(&mut self, track_ids: &[i64], path: PathBuf) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                self.set_status(format!("Cannot read image: {e}"), StatusKind::Error);
                return;
            }
        };
        const MAX_SIZE: u64 = 10 * 1024 * 1024;
        let file_size = metadata.len();
        if file_size > MAX_SIZE {
            let size_mb = file_size as f64 / (1024.0 * 1024.0);
            self.set_status(
                format!("Image exceeds 10 MB limit ({size_mb:.1} MB)"),
                StatusKind::Error,
            );
            return;
        }

        let path_str = path.to_string_lossy();
        let mut hasher = DefaultHasher::new();
        path_str.hash(&mut hasher);
        file_size.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        let cache_dir = self.paths.artwork_cache_dir();
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            self.set_status(format!("Cannot create cache dir: {e}"), StatusKind::Error);
            return;
        }
        let dest_path = cache_dir.join(format!("{hash}.jpg"));
        if let Err(e) = std::fs::copy(&path, &dest_path) {
            self.set_status(format!("Cannot copy image: {e}"), StatusKind::Error);
            return;
        }
        let dest_str = dest_path.to_string_lossy().to_string();

        self.apply_artwork_path(track_ids, &dest_str, &hash);
    }

    /// Set artwork for `track_ids` from raw image bytes (downloaded online):
    /// write to the artwork cache then update the DB for each target.
    fn set_artwork_from_bytes(&mut self, track_ids: &[i64], bytes: &[u8]) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        let cache_dir = self.paths.artwork_cache_dir();
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            self.set_status(format!("Cannot create cache dir: {e}"), StatusKind::Error);
            return;
        }
        let dest_path = cache_dir.join(format!("{hash}.jpg"));
        if let Err(e) = std::fs::write(&dest_path, bytes) {
            self.set_status(format!("Cannot write artwork: {e}"), StatusKind::Error);
            return;
        }
        let dest_str = dest_path.to_string_lossy().to_string();

        self.apply_artwork_path(track_ids, &dest_str, &hash);
    }

    /// Write an artwork path/hash to the DB for each target track and report.
    fn apply_artwork_path(&mut self, track_ids: &[i64], dest_str: &str, hash: &str) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                self.set_status(format!("DB error: {e}"), StatusKind::Error);
                return;
            }
        };
        let mut failed = 0usize;
        for &id in track_ids {
            if tornade_core::db::queries::set_track_artwork(&conn, id, dest_str, hash).is_err() {
                failed += 1;
            }
        }
        if failed == 0 {
            self.set_status("Artwork set", StatusKind::Success);
            self.player_artwork_track_id = None; // force artwork refresh on next tick
        } else {
            self.set_status(
                format!("Failed to set artwork for {failed} track(s)"),
                StatusKind::Error,
            );
        }
    }

    /// Remove artwork for each target track.
    pub fn remove_artwork(&mut self, track_ids: &[i64]) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                self.set_status(format!("DB error: {e}"), StatusKind::Error);
                return;
            }
        };
        let mut failed = 0usize;
        for &id in track_ids {
            if tornade_core::db::queries::remove_track_artwork(&conn, id).is_err() {
                failed += 1;
            }
        }
        if failed == 0 {
            self.set_status("Artwork removed", StatusKind::Success);
            self.player_artwork_track_id = None;
        } else {
            self.set_status(
                format!("Failed to remove artwork for {failed} track(s)"),
                StatusKind::Error,
            );
        }
    }
}
