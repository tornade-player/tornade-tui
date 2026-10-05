/// A command offered in command mode, with the aliases accepted by `Command::parse`.
struct Entry {
    command: &'static str,
    aliases: &'static [&'static str],
}

const fn entry(command: &'static str, aliases: &'static [&'static str]) -> Entry {
    Entry { command, aliases }
}

/// All commands available in command mode (leading colon included).
static COMMANDS: &[Entry] = &[
    entry(":scan", &[]),
    entry(":scan music", &[]),
    entry(":scan downloads", &[]),
    entry(":scan documents", &[]),
    entry(":scan desktop", &[]),
    entry(":cleanup", &[]),
    entry(":rate", &[]),
    entry(":rate album", &[]),
    entry(":queue add", &[]),
    entry(":queue clear", &[]),
    entry(":queue random", &[]),
    entry(":playlist create", &[]),
    entry(":playlist delete", &[]),
    entry(":playlist add", &[]),
    entry(":import", &[]),
    entry(":export", &[]),
    entry(":artist", &[]),
    entry(":refresh", &[]),
    entry(":stats", &[]),
    entry(":prefs", &[":preferences", ":settings"]),
    entry(":fetchart", &[]),
    entry(":artwork", &[]),
    entry(":artwork on", &[]),
    entry(":artwork off", &[]),
    entry(":source add", &[]),
    entry(":source remove", &[":source rm"]),
    entry(":sort", &[]),
    entry(":seek", &[]),
    entry(":help", &[]),
    entry(":tracks", &[]),
    entry(":albums", &[]),
    entry(":artists", &[]),
    entry(":genres", &[]),
    entry(":playlists", &[]),
    entry(":queue", &[]),
    entry(":search", &[]),
    entry(":q", &[]),
    entry(":qa", &[]),
];

/// A suggestion shown in command mode.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    /// Text put in the input when accepted (canonical command, no leading colon).
    pub insert: String,
    /// Text displayed in the list: the command followed by its aliases, if any.
    pub label: String,
}

/// Return all commands whose name or alias starts with `partial`
/// (case-insensitive, without leading colon).
pub fn complete(partial: &str) -> Vec<Completion> {
    let lower = format!(":{}", partial.to_lowercase().trim_start_matches(':'));
    COMMANDS
        .iter()
        .filter(|e| {
            e.command.starts_with(&lower) || e.aliases.iter().any(|a| a.starts_with(&lower))
        })
        .map(|e| Completion {
            insert: e.command[1..].to_string(),
            label: if e.aliases.is_empty() {
                e.command[1..].to_string()
            } else {
                format!("{} (alias: {})", &e.command[1..], e.aliases.join(", "))
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inserts(partial: &str) -> Vec<String> {
        complete(partial).into_iter().map(|c| c.insert).collect()
    }

    #[test]
    fn complete_scan() {
        assert!(inserts("sca").contains(&"scan".to_string()));
    }

    #[test]
    fn complete_queue() {
        assert!(inserts("queue").iter().any(|r| r.starts_with("queue")));
    }

    #[test]
    fn complete_empty_returns_all() {
        assert_eq!(complete("").len(), COMMANDS.len());
    }

    #[test]
    fn complete_no_match_returns_empty() {
        assert!(complete("zzznomatch").is_empty());
    }

    #[test]
    fn complete_alias_suggests_canonical_command() {
        let results = complete("sett");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].insert, "prefs");
        assert!(results[0].label.contains(":settings"));
        assert!(results[0].label.contains(":preferences"));
    }

    #[test]
    fn complete_source_rm_alias() {
        assert_eq!(inserts("source rm"), vec!["source remove".to_string()]);
    }
}
