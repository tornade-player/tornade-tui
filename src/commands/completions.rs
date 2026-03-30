/// All top-level command prefixes available in command mode.
pub static COMMAND_LIST: &[&str] = &[
    ":scan",
    ":cleanup",
    ":rate",
    ":queue add",
    ":queue clear",
    ":playlist create",
    ":playlist delete",
    ":playlist add",
    ":import",
    ":seek",
    ":help",
    ":tracks",
    ":albums",
    ":artists",
    ":genres",
    ":playlists",
    ":queue",
    ":search",
];

/// Return all commands that start with `partial` (case-insensitive, without leading colon).
pub fn complete(partial: &str) -> Vec<String> {
    let lower = format!(":{}", partial.to_lowercase().trim_start_matches(':'));
    COMMAND_LIST
        .iter()
        .filter(|c| c.to_lowercase().starts_with(&lower))
        .map(|c| c[1..].to_string()) // strip leading colon for display
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_scan() {
        let results = complete("sca");
        assert!(results.contains(&"scan".to_string()));
    }

    #[test]
    fn complete_queue() {
        let results = complete("queue");
        assert!(results.iter().any(|r| r.starts_with("queue")));
    }

    #[test]
    fn complete_empty_returns_all() {
        let results = complete("");
        assert_eq!(results.len(), COMMAND_LIST.len());
    }

    #[test]
    fn complete_no_match_returns_empty() {
        let results = complete("zzznomatch");
        assert!(results.is_empty());
    }
}
