use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
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
    // inner width (minus borders) for label truncation
    let label_w = (area.width as usize).saturating_sub(4); // 2 borders + 3 indent

    let mut items: Vec<ListItem> = Vec::new();
    let mut sel_idx: usize = 0;

    // ── "Library" header ──────────────────────────────────────────────────────
    items.push(header_item(" Library"));

    for entry in &LIBRARY_ENTRIES {
        let is_active = *entry == active_entry
            && !matches!(active_entry, SidebarEntry::Playlists | SidebarEntry::Queue);
        let is_cursor = focused && sel_idx == cursor;
        items.push(nav_item(entry.label(), label_w, is_active, is_cursor));
        sel_idx += 1;
    }

    // ── "Playlists" header ────────────────────────────────────────────────────
    items.push(header_item(" Playlists"));

    if playlists.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "   No playlists",
            Style::default().fg(Color::DarkGray),
        ))));
    } else {
        for (id, name) in playlists {
            let is_active = active_playlist_id == Some(*id);
            let is_cursor = focused && sel_idx == cursor;
            items.push(nav_item(&truncate(name, label_w), label_w, is_active, is_cursor));
            sel_idx += 1;
        }
    }

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Tornade ")
            .border_style(border_style),
    );
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

fn nav_item(label: &str, _max_w: usize, active: bool, cursor: bool) -> ListItem<'static> {
    let text = format!("   {}", label);
    let fg = if active {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let style = if cursor { fg.bg(Color::DarkGray) } else { fg };
    ListItem::new(Line::from(Span::styled(text, style)))
}
