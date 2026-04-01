use std::time::Instant;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use tornade_core::{
    models::Track,
    services::{ArtworkService, LibraryService, PlaybackState, PlaylistService, PlayerService, SearchService},
    utils::AppPaths,
};
use crate::{
    navigation::NavigationStack,
    player::PlayerStateCache,
    views::{
        View, SidebarEntry,
        LibraryState, AlbumsState, ArtistsState, GenresState, PlaylistsState,
        PlaylistDetailState, QueueState, SearchState,
    },
    views::queue::ToolbarHitZones,
    widgets::player_bar::PlayerHitZones,
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
pub enum StatusKind { Info, Success, Error }

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
    RenamePlaylist { id: i64 },
    ImportM3u,
    SaveQueueAsPlaylist,
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

pub struct AppState {
    // Core services
    pub player: PlayerService,
    pub library: LibraryService,
    pub playlists: PlaylistService,
    pub search_svc: SearchService,
    pub artwork: ArtworkService,
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
    pub show_playlist_selector: bool,
    pub playlist_selector_state: ratatui::widgets::ListState,

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
}

impl AppState {
    pub fn new(
        player: PlayerService,
        library: LibraryService,
        playlists: PlaylistService,
        search_svc: SearchService,
        artwork: ArtworkService,
        paths: AppPaths,
        picker: Picker,
    ) -> Self {
        let mut state = Self {
            player,
            library,
            playlists,
            search_svc,
            artwork,
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
            show_playlist_selector: false,
            playlist_selector_state: ratatui::widgets::ListState::default(),
            focused_panel: FocusedPanel::Content,
            sidebar_cursor: 0,
            right_panel_queue_state: ratatui::widgets::ListState::default(),
            cached_queue_tracks: Vec::new(),
            cached_queue_ids: Vec::new(),
            sidebar_playlists: Vec::new(),
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
        };
        state.reload_current_view();
        state.refresh_sidebar_playlists();
        state
    }

    /// Poll player state and clear stale status. Called every 500ms.
    pub fn tick(&mut self) {
        self.player_cache.poll(&self.player);
        if let Some(ref s) = self.status {
            if s.set_at.elapsed().as_secs() >= 3 {
                self.status = None;
            }
        }
        let skipped = self.player_cache.skipped_track_ids.clone();
        self.apply_skipped_ids(&skipped);
        self.refresh_queue_cache();
        self.refresh_sidebar_playlists();
        self.refresh_player_artwork();
    }

    fn refresh_player_artwork(&mut self) {
        let current_id = self.player_cache.current_track.as_ref().map(|t| t.id);
        if current_id == self.player_artwork_track_id {
            return;
        }
        self.player_artwork_track_id = current_id;
        let track = self.player_cache.current_track.clone();
        if let Some(ref track) = track {
            if let Some(album_id) = track.album_id {
                if let Ok(Some(album)) = self.library.get_album(album_id) {
                    self.current_album_name = Some(album.title.clone());
                    let path = album.online_artwork_path.as_ref()
                        .or(album.artwork_path.as_ref())
                        .cloned();
                    self.player_artwork = path
                        .and_then(|p| image::open(p).ok())
                        .map(|img| self.picker.new_resize_protocol(img));
                    return;
                }
            }
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
        self.cached_queue_tracks = self.cached_queue_ids.iter()
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
                    .and_then(|id| self.sidebar_playlists.iter().position(|(pid, _)| *pid == id))
                    .map(|idx| SidebarEntry::all().len() + idx)
                    .unwrap_or(0)
            }
            other => SidebarEntry::all().iter().position(|e| *e == other).unwrap_or(0),
        };
    }

    pub fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Some(StatusMessage { text: text.into(), kind, set_at: Instant::now() });
    }

    pub fn navigate_to(&mut self, entry: SidebarEntry) {
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
            View::Albums(s)  if s.filter_active && !s.filter.is_empty() => s.filter.clone(),
            View::Library(s) if s.filter.is_empty() => { let _ = s; String::new() }
            View::Artists(s) if s.filter.is_empty() => { let _ = s; String::new() }
            View::Albums(s)  if s.filter.is_empty() => { let _ = s; String::new() }
            _ => return,
        };

        if query.is_empty() {
            match self.nav.current_mut() {
                View::Library(s) => { s.search_results = None; s.list_state.select(if s.tracks.is_empty() { None } else { Some(0) }); }
                View::Artists(s) => { s.search_results = None; s.list_state.select(if s.artists.is_empty() { None } else { Some(0) }); }
                View::Albums(s)  => { s.search_results = None; s.selected = 0; s.scroll_row = 0; }
                _ => {}
            }
            return;
        }

        let Ok(results) = self.search_svc.search(&query) else { return };

        match self.nav.current_mut() {
            View::Library(s) => {
                s.list_state.select(if results.tracks.is_empty() { None } else { Some(0) });
                s.search_results = Some(results.tracks);
            }
            View::Artists(s) => {
                s.list_state.select(if results.artists.is_empty() { None } else { Some(0) });
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
        match self.nav.current() {
            View::Queue(s) => {
                let idx = s.list_state.selected().unwrap_or(0);
                let _ = self.player.jump_to_index(idx);
                if self.player_cache.state != PlaybackState::Playing {
                    let _ = self.player.resume();
                }
                return;
            }
            _ => {}
        }
        let (ids, index) = match self.nav.current() {
            View::Library(s) => (s.visible_track_ids(), s.list_state.selected().unwrap_or(0)),
            View::AlbumDetail(s) => (s.visible_track_ids(), s.list_state.selected().unwrap_or(0)),
            View::GenreDetail(s) => (s.visible_track_ids(), s.list_state.selected().unwrap_or(0)),
            View::PlaylistDetail(s) => (s.visible_track_ids(), s.list_state.selected().unwrap_or(0)),
            _ => return,
        };
        if ids.is_empty() { return; }
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

    pub fn rate_selected(&mut self, stars: u8) {
        if let Some(id) = self.selected_track_id() {
            match self.library.rate_track(id, stars) {
                Ok(_) => {
                    self.set_status(
                        format!("Rated: {}{}", "★".repeat(stars as usize), "☆".repeat(5 - stars as usize)),
                        StatusKind::Success,
                    );
                    self.reload_current_view();
                }
                Err(e) => self.set_status(format!("Error: {}", e), StatusKind::Error),
            }
        }
    }
}
