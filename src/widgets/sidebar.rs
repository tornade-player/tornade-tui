use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Padding},
};
use crate::utils::truncate;
use crate::views::SidebarEntry;

/// Library entries shown in the sidebar, in display order.
const LIBRARY_ENTRIES: [SidebarEntry; 5] = [
    SidebarEntry::Search,
    SidebarEntry::Tracks,
    SidebarEntry::Albums,
    SidebarEntry::Artists,
    SidebarEntry::Genres,
];

pub fn render(
    frame: &mut Frame,
    area: Rect,
    active_entry: SidebarEntry,
    active_playlist_id: Option<i64>,
    focused: bool,
    cursor: usize,
    playlists: &[(i64, String)],
) {
    // Content width: area - 2 (borders) - 2 (h padding) - 2 (glyph cols) - 1 (space after glyph)
    let label_w = (area.width as usize).saturating_sub(7);

    let mut items: Vec<ListItem> = Vec::new();
    let mut sel_idx: usize = 0;

    // ── "Library" header ──────────────────────────────────────────────────────
    items.push(header_item("Library"));

    for entry in &LIBRARY_ENTRIES {
        let is_active = *entry == active_entry
            && !matches!(active_entry, SidebarEntry::Playlists | SidebarEntry::Queue);
        let is_cursor = focused && sel_idx == cursor;
        items.push(nav_item(entry.glyph(), entry.label(), entry.shortcut(), is_active, is_cursor));
        items.push(blank());
        sel_idx += 1;
    }

    // ── "Playlists" header ────────────────────────────────────────────────────
    items.push(blank()); // extra gap before section header
    items.push(header_item("Playlists"));
    items.push(blank());

    if playlists.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "No playlists",
            Style::default().fg(Color::DarkGray),
        ))));
    } else {
        for (id, name) in playlists {
            let is_active = active_playlist_id == Some(*id);
            let is_cursor = focused && sel_idx == cursor;
            let glyph = SidebarEntry::Playlists.glyph();
            items.push(nav_item(glyph, &truncate(name, label_w), None, is_active, is_cursor));
            items.push(blank());
            sel_idx += 1;
        }
    }

    let bg = Color::Rgb(22, 24, 32);
    let block = Block::default()
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(bg));
    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn header_item(label: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        label.to_string(),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )))
}

fn blank() -> ListItem<'static> {
    ListItem::new(Line::from(""))
}

fn nav_item(glyph: &str, name: &str, shortcut: Option<u8>, active: bool, cursor: bool) -> ListItem<'static> {
    let suffix = match shortcut {
        Some(n) => format!(" [{}]", n),
        None => String::new(),
    };
    let text = format!("{} {}{}", glyph, name, suffix);
    let fg = if active {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let style = if cursor { fg.bg(Color::DarkGray) } else { fg };
    ListItem::new(Line::from(Span::styled(text, style)))
}
