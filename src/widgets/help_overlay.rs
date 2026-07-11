use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem},
};

pub fn render(frame: &mut Frame) {
    let area = centered_rect(80, 85, frame.area());
    frame.render_widget(Clear, area);

    let sections: &[(&str, &[(&str, &str)])] = &[
        (
            "Playback",
            &[
                ("Space", "Play / Pause"),
                ("n", "Next track"),
                ("N", "Previous track"),
                ("]", "Seek +10s"),
                ("[", "Seek -10s"),
                ("+", "Volume up"),
                ("-", "Volume down"),
                ("S", "Toggle shuffle"),
                ("R", "Cycle repeat (Off / All / One)"),
            ],
        ),
        (
            "Navigation",
            &[
                (
                    "1–7",
                    "Switch view (Tracks/Albums/Artists/Genres/Playlists/Queue/Search)",
                ),
                ("j / ↓", "Move down"),
                ("k / ↑", "Move up"),
                ("Ctrl+D", "Page down"),
                ("Ctrl+U", "Page up"),
                ("g", "Jump to top"),
                ("G", "Jump to bottom"),
                ("Enter", "Select / play"),
                ("q / ESC", "Back"),
                ("?", "Toggle this help"),
            ],
        ),
        (
            "Queue",
            &[
                ("a", "Add selected track to queue"),
                ("J", "Move track down (in Queue view)"),
                ("K", "Move track up (in Queue view)"),
                ("x", "Remove track from queue"),
                ("X", "Clear entire queue"),
            ],
        ),
        (
            "Playlists",
            &[
                ("A", "Add track to playlist"),
                ("c", "Create playlist (in Playlists view)"),
                ("r", "Rename playlist (in Playlists view)"),
                ("d", "Delete playlist (in Playlists view)"),
                ("i", "Import M3U (in Playlists view)"),
                ("J / K", "Reorder track (in Playlist detail)"),
                ("x", "Remove track from playlist"),
            ],
        ),
        (
            "Library",
            &[
                ("s", "Scan music folder"),
                ("/", "Filter current view"),
                ("0–5", "Set rating on selected track"),
            ],
        ),
        (
            "Edit tags & selection",
            &[
                ("e", "Edit tags (selection if any, else current track)"),
                ("Tab / Shift+Tab", "Move between fields (in editor)"),
                ("Ctrl+F", "Fetch metadata online (in editor)"),
                ("Ctrl+I", "Manage cover artwork (in editor)"),
                ("v", "Toggle multi-select mode"),
                ("Space", "Mark / unmark track (in select mode)"),
                ("a", "Select all (in select mode)"),
                ("c / ESC", "Clear selection"),
                ("A", "Add selection to playlist"),
                ("D", "Remove selection from playlist (in playlist)"),
            ],
        ),
        (
            "Media keys",
            &[(
                "⏯ / ⏭ / ⏮",
                "Hardware play-pause / next / previous (where supported)",
            )],
        ),
        (
            "Commands (:)",
            &[
                (":scan <path>", "Scan a folder"),
                (":scan music|downloads|…", "Scan a quick-access folder"),
                (":stats", "Show library statistics"),
                (":refresh", "Reload the current view from the database"),
                (":sort <field>", "Sort tracks (title/artist/duration/rating/plays)"),
                (":cleanup", "Remove missing files from library"),
                (":rate <0-5>", "Rate selected track"),
                (":rate album <0-5>", "Rate the open album"),
                (":queue add", "Add selected track to queue"),
                (":queue random [N]", "Add N random tracks to queue"),
                (":queue clear", "Clear the queue"),
                (":playlist create <name>", "Create playlist"),
                (":playlist delete <name>", "Delete playlist"),
                (":playlist add <name>", "Add track to playlist"),
                (":import <path>", "Import M3U file"),
                (":export <path>", "Export open playlist to M3U"),
                (":artist  (or o)", "Go to the selected track's artist"),
                (":seek <mm:ss>", "Seek to position"),
                (":help", "Show this help"),
                (":tracks / :albums / …", "Navigate to view"),
            ],
        ),
    ];

    // Split into two columns
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let half = sections.len() / 2 + sections.len() % 2;
    for (col_idx, section_slice) in [&sections[..half], &sections[half..]].iter().enumerate() {
        let mut items: Vec<ListItem> = Vec::new();
        for (section_name, entries) in *section_slice {
            items.push(ListItem::new(Line::from(Span::styled(
                *section_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ))));
            for (key, action) in *entries {
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {:>22}  ", key),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(*action, Style::default().fg(Color::Gray)),
                ])));
            }
            items.push(ListItem::new(Line::from("")));
        }
        let list = List::new(items);
        frame.render_widget(list, cols[col_idx]);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let height = r.height * percent_y / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}
