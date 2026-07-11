use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
};

/// Render the library-statistics overlay (`:stats`). `lines` is a list of
/// (label, value) pairs. Any key dismisses it.
pub fn render(frame: &mut Frame, lines: &[(String, String)]) {
    render_titled(frame, " Library statistics ", lines);
}

/// Render a titled key/value overlay (shared by `:stats` and `:prefs`).
pub fn render_titled(frame: &mut Frame, title: &str, lines: &[(String, String)]) {
    let width = lines
        .iter()
        .map(|(l, v)| l.len() + v.len() + 6)
        .max()
        .unwrap_or(40)
        .clamp(30, 90) as u16;
    let area = centered_rect(width, (lines.len() + 4) as u16, frame.area());
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = lines
        .iter()
        .map(|(label, value)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {label:<14}"), Style::default().fg(Color::Gray)),
                Span::styled(
                    value.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.to_string())
        .title_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(List::new(items).block(block), area);
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    Rect {
        x: r.x + (r.width.saturating_sub(w)) / 2,
        y: r.y + (r.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}
