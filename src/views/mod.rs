use std::sync::atomic::AtomicUsize;

/// Global cap on concurrent background image-decode threads across all views.
/// Prevents thread accumulation when the user navigates between image views rapidly
/// (each new view state has empty loading_ids but old threads from the previous
/// state are still running, so without a global cap they compound).
pub static ACTIVE_IMAGE_THREADS: AtomicUsize = AtomicUsize::new(0);
pub const MAX_IMAGE_THREADS: usize = 4;

pub mod album_detail;
pub mod albums;
pub mod artist_detail;
pub mod artists;
pub mod genre_detail;
pub mod genres;
pub mod library;
pub mod playlist_detail;
pub mod playlists;
pub mod queue;
#[cfg(test)]
mod render_tests;
pub mod scan;
pub mod search;

pub use album_detail::AlbumDetailState;
pub use albums::AlbumsState;
pub use artist_detail::ArtistDetailState;
pub use artists::ArtistsState;
pub use genre_detail::GenreDetailState;
pub use genres::GenresState;
pub use library::LibraryState;
pub use playlist_detail::PlaylistDetailState;
pub use playlists::PlaylistsState;
pub use queue::QueueState;
pub use scan::ScanState;
pub use search::SearchState;

/// All possible views that can appear on the navigation stack.
pub enum View {
    Library(LibraryState),
    Albums(AlbumsState),
    AlbumDetail(AlbumDetailState),
    Artists(ArtistsState),
    ArtistDetail(ArtistDetailState),
    Genres(GenresState),
    GenreDetail(GenreDetailState),
    Playlists(PlaylistsState),
    PlaylistDetail(PlaylistDetailState),
    Queue(QueueState),
    Search(SearchState),
    Scan(ScanState),
}

/// Top-level sidebar entries — the 7 root views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarEntry {
    Tracks,
    Albums,
    Artists,
    Genres,
    Playlists,
    Queue,
    Search,
}

impl SidebarEntry {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::Genres => "Genres",
            Self::Search => "Search",
            Self::Playlists => "Playlists",
            Self::Queue => "Queue",
        }
    }

    /// Nerd Font glyph for this entry (nf-fa-* range, single codepoint).
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Search => "\u{f002}",    // nf-fa-search
            Self::Tracks => "\u{f001}",    // nf-fa-music
            Self::Albums => "\u{f51f}",    // nf-fa-compact_disc
            Self::Artists => "\u{f007}",   // nf-fa-user
            Self::Genres => "\u{f02b}",    // nf-fa-tag
            Self::Playlists => "\u{f03a}", // nf-fa-list
            Self::Queue => "\u{f0cb}",     // nf-fa-list_ol
        }
    }

    /// Keyboard shortcut digit for this entry, if any.
    pub fn shortcut(&self) -> Option<u8> {
        match self {
            Self::Tracks => Some(1),
            Self::Albums => Some(2),
            Self::Artists => Some(3),
            Self::Genres => Some(4),
            Self::Search => Some(5),
            Self::Playlists | Self::Queue => None,
        }
    }

    pub fn from_digit(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Tracks),
            2 => Some(Self::Albums),
            3 => Some(Self::Artists),
            4 => Some(Self::Genres),
            5 => Some(Self::Search),
            _ => None,
        }
    }

    /// Library entries in sidebar order. Playlists are shown
    /// separately as dynamic items below the "Playlists" header.
    pub fn all() -> &'static [SidebarEntry] {
        &[
            Self::Tracks,
            Self::Albums,
            Self::Artists,
            Self::Genres,
            Self::Search,
        ]
    }
}

impl View {
    pub fn sidebar_entry(&self) -> SidebarEntry {
        match self {
            View::Library(_) => SidebarEntry::Tracks,
            View::Albums(_) | View::AlbumDetail(_) => SidebarEntry::Albums,
            View::Artists(_) | View::ArtistDetail(_) => SidebarEntry::Artists,
            View::Genres(_) | View::GenreDetail(_) => SidebarEntry::Genres,
            View::Playlists(_) | View::PlaylistDetail(_) => SidebarEntry::Playlists,
            View::Queue(_) => SidebarEntry::Queue,
            View::Search(_) => SidebarEntry::Search,
            View::Scan(_) => SidebarEntry::Tracks,
        }
    }
}
