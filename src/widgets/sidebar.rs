use ratatui::{Frame, layout::Rect, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, List, ListItem}};
use crate::views::SidebarEntry;

pub fn render(frame: &mut Frame, area: Rect, active: SidebarEntry) {
    let items: Vec<ListItem> = SidebarEntry::all().iter().enumerate().map(|(i, entry)| {
        let is_active = *entry == active;
        let label = format!(" {}  {}", i + 1, entry.label());
        let style = if is_active {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        ListItem::new(Line::from(Span::styled(label, style)))
    }).collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Tornade "));
    frame.render_widget(list, area);
}
