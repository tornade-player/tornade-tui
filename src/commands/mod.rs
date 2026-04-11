pub mod completions;

use crate::utils::expand_tilde;
use crate::views::SidebarEntry;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum Command {
    Scan { path: PathBuf },
    Cleanup,
    Rate { stars: u8 },
    QueueAdd,
    QueueClear,
    PlaylistCreate { name: String },
    PlaylistDelete { name: String },
    PlaylistAdd { name: String },
    Import { path: PathBuf },
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
                    Self::Unknown("scan requires a path: :scan <path>".to_string())
                } else {
                    Self::Scan {
                        path: expand_tilde(rest),
                    }
                }
            }
            "cleanup" => Self::Cleanup,
            "rate" => match rest.parse::<u8>() {
                Ok(n) if n <= 5 => Self::Rate { stars: n },
                _ => Self::Unknown("rate requires 0-5: :rate <0-5>".to_string()),
            },
            "queue" => match rest.to_lowercase().as_str() {
                "add" => Self::QueueAdd,
                "clear" => Self::QueueClear,
                _ => Self::Navigate(SidebarEntry::Queue),
            },
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
}
