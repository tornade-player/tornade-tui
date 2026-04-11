use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, Paragraph},
};

pub fn render(frame: &mut Frame, area: Rect, input: &str, completions: &[String]) {
    // Command input line (bottom of area)
    let input_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    let prompt = Paragraph::new(Line::from(vec![
        Span::styled(
            ":",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(input),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]));
    frame.render_widget(prompt, input_area);

    // Completions popup above input
    if !completions.is_empty() {
        let max_items = 5usize.min(completions.len());
        let popup_height = max_items as u16 + 2;
        let popup_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(1 + popup_height),
            width: (area.width / 2).max(30),
            height: popup_height,
        };
        frame.render_widget(Clear, popup_area);
        let items: Vec<ListItem> = completions[..max_items]
            .iter()
            .map(|c| {
                ListItem::new(Line::from(Span::styled(
                    c.as_str(),
                    Style::default().fg(Color::Cyan),
                )))
            })
            .collect();
        let list = List::new(items).block(Block::default());
        frame.render_widget(list, popup_area);
    }
}
