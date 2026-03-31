pub mod library;
pub mod albums;
pub mod album_detail;
pub mod artists;
pub mod artist_detail;
pub mod genres;
pub mod genre_detail;
pub mod playlists;
pub mod playlist_detail;
pub mod queue;
pub mod search;
pub mod scan;

pub use library::LibraryState;
pub use albums::AlbumsState;
pub use album_detail::AlbumDetailState;
pub use artists::ArtistsState;
pub use artist_detail::ArtistDetailState;
pub use genres::GenresState;
pub use genre_detail::GenreDetailState;
pub use playlists::PlaylistsState;
pub use playlist_detail::PlaylistDetailState;
pub use queue::QueueState;
pub use search::SearchState;
pub use scan::ScanState;

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
            Self::Tracks => "1  Tracks",
            Self::Albums => "2  Albums",
            Self::Artists => "3  Artists",
            Self::Genres => "4  Genres",
            Self::Search => "5  Search",
            Self::Playlists => "Playlists",
            Self::Queue => "Queue",
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
        &[Self::Tracks, Self::Albums, Self::Artists, Self::Genres, Self::Search]
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
