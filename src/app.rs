use std::time::Instant;
use tornade_core::{
    services::{ArtworkService, LibraryService, PlaybackState, PlaylistService, PlayerService, SearchService},
};
use crate::{
    navigation::NavigationStack,
    player::PlayerStateCache,
    views::{
        View, SidebarEntry,
        LibraryState, AlbumsState, ArtistsState, GenresState, PlaylistsState, QueueState, SearchState,
    },
};

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

    // Status bar
    pub status: Option<StatusMessage>,
}

impl AppState {
    pub fn new(
        player: PlayerService,
        library: LibraryService,
        playlists: PlaylistService,
        search_svc: SearchService,
        artwork: ArtworkService,
    ) -> Self {
        let mut state = Self {
            player,
            library,
            playlists,
            search_svc,
            artwork,
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
            status: None,
        };
        state.reload_current_view();
        state
    }

    /// Poll player state and clear stale status. Called every 250ms.
    pub fn tick(&mut self) {
        self.player_cache.poll(&self.player);
        if let Some(ref s) = self.status {
            if s.set_at.elapsed().as_secs() >= 3 {
                self.status = None;
            }
        }
        let skipped = self.player_cache.skipped_track_ids.clone();
        self.apply_skipped_ids(&skipped);
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
