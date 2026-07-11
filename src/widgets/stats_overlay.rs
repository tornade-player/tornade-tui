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
    let area = centered_rect(40, (lines.len() + 4) as u16, frame.area());
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
        .title(" Library statistics ")
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
