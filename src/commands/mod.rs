pub mod completions;

use crate::utils::expand_tilde;
use crate::views::SidebarEntry;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum Command {
    Scan { path: PathBuf },
    Cleanup,
    Rate { stars: u8 },
    RateAlbum { stars: u8 },
    QueueAdd,
    QueueClear,
    QueueRandom { count: usize },
    PlaylistCreate { name: String },
    PlaylistDelete { name: String },
    PlaylistAdd { name: String },
    Import { path: PathBuf },
    Export { path: PathBuf },
    GoToArtist,
    Refresh,
    Stats,
    Prefs,
    Visualizer,
    FetchArtwork,
    ArtworkToggle { mode: String },
    SourceAdd { path: PathBuf },
    SourceRemove { id: i64 },
    Sort { field: String },
    Seek { position_str: String },
    Help,
    Navigate(SidebarEntry),
    Unknown(String),
}

impl Command {
    pub fn parse(input: &str) -> Self {
        let input = input.trim();
        if input.is_empty() {
            return Self::Unknown(String::new());
        }

        // Split into command + rest
        let (cmd, rest) = match input.split_once(' ') {
            Some((c, r)) => (c.trim(), r.trim()),
            None => (input, ""),
        };

        match cmd.to_lowercase().as_str() {
            "scan" => {
                if rest.is_empty() {
                    Self::Unknown(
                        "scan requires a path: :scan <path|music|downloads|documents|desktop>"
                            .to_string(),
                    )
                } else {
                    // Quick-access presets expand to the matching home folder.
                    let preset = match rest.to_lowercase().as_str() {
                        "music" => Some("~/Music"),
                        "downloads" => Some("~/Downloads"),
                        "documents" => Some("~/Documents"),
                        "desktop" => Some("~/Desktop"),
                        _ => None,
                    };
                    Self::Scan {
                        path: expand_tilde(preset.unwrap_or(rest)),
                    }
                }
            }
            "cleanup" => Self::Cleanup,
            "rate" => {
                let lower = rest.to_lowercase();
                if let Some(n) = lower.strip_prefix("album").map(str::trim) {
                    match n.parse::<u8>() {
                        Ok(n) if n <= 5 => Self::RateAlbum { stars: n },
                        _ => {
                            Self::Unknown("rate album requires 0-5: :rate album <0-5>".to_string())
                        }
                    }
                } else {
                    match rest.parse::<u8>() {
                        Ok(n) if n <= 5 => Self::Rate { stars: n },
                        _ => Self::Unknown("rate requires 0-5: :rate <0-5>".to_string()),
                    }
                }
            }
            "queue" => {
                let lower = rest.to_lowercase();
                if let Some(n) = lower.strip_prefix("random").map(str::trim) {
                    let count = n.parse::<usize>().unwrap_or(20).clamp(1, 500);
                    Self::QueueRandom { count }
                } else {
                    match lower.as_str() {
                        "add" => Self::QueueAdd,
                        "clear" => Self::QueueClear,
                        _ => Self::Navigate(SidebarEntry::Queue),
                    }
                }
            }
            "playlist" => {
                let (subcmd, name) = rest.split_once(' ').unwrap_or((rest, ""));
                match subcmd.to_lowercase().as_str() {
                    "create" if !name.is_empty() => Self::PlaylistCreate {
                        name: name.to_string(),
                    },
                    "delete" if !name.is_empty() => Self::PlaylistDelete {
                        name: name.to_string(),
                    },
                    "add" if !name.is_empty() => Self::PlaylistAdd {
                        name: name.to_string(),
                    },
                    _ => Self::Unknown(format!("unknown playlist subcommand: {}", rest)),
                }
            }
            "import" => {
                if rest.is_empty() {
                    Self::Unknown("import requires a path: :import <path>".to_string())
                } else {
                    Self::Import {
                        path: expand_tilde(rest),
                    }
                }
            }
            "export" => {
                if rest.is_empty() {
                    Self::Unknown("export requires a path: :export <path>".to_string())
                } else {
                    Self::Export {
                        path: expand_tilde(rest),
                    }
                }
            }
            "artist" => Self::GoToArtist,
            "refresh" => Self::Refresh,
            "stats" => Self::Stats,
            "fetchart" => Self::FetchArtwork,
            "artwork" => Self::ArtworkToggle {
                mode: rest.to_lowercase(),
            },
            "source" => {
                let (subcmd, arg) = rest.split_once(' ').unwrap_or((rest, ""));
                match subcmd.to_lowercase().as_str() {
                    "add" if !arg.is_empty() => Self::SourceAdd {
                        path: expand_tilde(arg),
                    },
                    "remove" | "rm" => match arg.trim().parse::<i64>() {
                        Ok(id) => Self::SourceRemove { id },
                        _ => Self::Unknown(
                            "source remove needs an id: :source remove <id>".to_string(),
                        ),
                    },
                    _ => {
                        Self::Unknown("usage: :source add <path> | :source remove <id>".to_string())
                    }
                }
            }
            "prefs" | "preferences" | "settings" => Self::Prefs,
            "vis" | "vu" | "visualizer" => Self::Visualizer,
            "sort" => {
                if rest.is_empty() {
                    Self::Unknown(
                        "sort <title|artist|duration|rating|plays|lastplayed>".to_string(),
                    )
                } else {
                    Self::Sort {
                        field: rest.to_string(),
                    }
                }
            }
            "seek" => {
                if rest.is_empty() {
                    Self::Unknown("seek requires a position: :seek <mm:ss>".to_string())
                } else {
                    Self::Seek {
                        position_str: rest.to_string(),
                    }
                }
            }
            "help" => Self::Help,
            "tracks" => Self::Navigate(SidebarEntry::Tracks),
            "albums" => Self::Navigate(SidebarEntry::Albums),
            "artists" => Self::Navigate(SidebarEntry::Artists),
            "genres" => Self::Navigate(SidebarEntry::Genres),
            "playlists" => Self::Navigate(SidebarEntry::Playlists),
            "search" => Self::Navigate(SidebarEntry::Search),
            other => Self::Unknown(format!("unknown command: {}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scan() {
        assert!(matches!(
            Command::parse("scan ~/Music"),
            Command::Scan { .. }
        ));
    }

    #[test]
    fn parse_scan_no_path() {
        assert!(matches!(Command::parse("scan"), Command::Unknown(_)));
    }

    #[test]
    fn parse_rate_valid() {
        assert_eq!(Command::parse("rate 4"), Command::Rate { stars: 4 });
        assert_eq!(Command::parse("rate 0"), Command::Rate { stars: 0 });
        assert_eq!(Command::parse("rate 5"), Command::Rate { stars: 5 });
    }

    #[test]
    fn parse_rate_out_of_range() {
        assert!(matches!(Command::parse("rate 6"), Command::Unknown(_)));
        assert!(matches!(Command::parse("rate abc"), Command::Unknown(_)));
    }

    #[test]
    fn parse_queue_add() {
        assert_eq!(Command::parse("queue add"), Command::QueueAdd);
    }

    #[test]
    fn parse_queue_clear() {
        assert_eq!(Command::parse("queue clear"), Command::QueueClear);
    }

    #[test]
    fn parse_playlist_create() {
        assert_eq!(
            Command::parse("playlist create My Mix"),
            Command::PlaylistCreate {
                name: "My Mix".to_string()
            }
        );
    }

    #[test]
    fn parse_playlist_no_name() {
        assert!(matches!(
            Command::parse("playlist create"),
            Command::Unknown(_)
        ));
    }

    #[test]
    fn parse_seek() {
        assert_eq!(
            Command::parse("seek 1:30"),
            Command::Seek {
                position_str: "1:30".to_string()
            }
        );
    }

    #[test]
    fn parse_help() {
        assert_eq!(Command::parse("help"), Command::Help);
    }

    #[test]
    fn parse_navigate() {
        assert_eq!(
            Command::parse("albums"),
            Command::Navigate(SidebarEntry::Albums)
        );
        assert_eq!(
            Command::parse("tracks"),
            Command::Navigate(SidebarEntry::Tracks)
        );
    }

    #[test]
    fn parse_unknown() {
        assert!(matches!(Command::parse("foobar"), Command::Unknown(_)));
        assert!(matches!(Command::parse(""), Command::Unknown(_)));
    }

    #[test]
    fn parse_import() {
        assert!(matches!(
            Command::parse("import /path/to/file.m3u"),
            Command::Import { .. }
        ));
    }

    #[test]
    fn parse_export() {
        assert!(matches!(
            Command::parse("export ~/mix.m3u"),
            Command::Export { .. }
        ));
        assert!(matches!(Command::parse("export"), Command::Unknown(_)));
    }

    #[test]
    fn parse_rate_album() {
        assert_eq!(
            Command::parse("rate album 4"),
            Command::RateAlbum { stars: 4 }
        );
        assert!(matches!(
            Command::parse("rate album 9"),
            Command::Unknown(_)
        ));
    }

    #[test]
    fn parse_queue_random() {
        assert_eq!(
            Command::parse("queue random 10"),
            Command::QueueRandom { count: 10 }
        );
        assert_eq!(
            Command::parse("queue random"),
            Command::QueueRandom { count: 20 }
        );
    }

    #[test]
    fn parse_go_to_artist() {
        assert_eq!(Command::parse("artist"), Command::GoToArtist);
    }

    #[test]
    fn parse_stats() {
        assert_eq!(Command::parse("stats"), Command::Stats);
    }

    #[test]
    fn parse_sort() {
        assert_eq!(
            Command::parse("sort artist"),
            Command::Sort {
                field: "artist".to_string()
            }
        );
        assert!(matches!(Command::parse("sort"), Command::Unknown(_)));
    }

    #[test]
    fn parse_scan_preset() {
        assert!(matches!(Command::parse("scan music"), Command::Scan { .. }));
        assert!(matches!(
            Command::parse("scan downloads"),
            Command::Scan { .. }
        ));
    }
}
