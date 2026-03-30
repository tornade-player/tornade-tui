use ratatui::{Frame, layout::Rect, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, Clear, Paragraph}};

pub fn render(frame: &mut Frame, prompt: &str) {
    let area = centered_rect(50, 7, frame.area());
    frame.render_widget(Clear, area);
    let content = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(prompt, Style::default().add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [y] Confirm  ", Style::default().fg(Color::Green)),
            Span::styled("  [n] Cancel  ", Style::default().fg(Color::Red)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Confirm ").border_style(Style::default().fg(Color::Yellow)));
    frame.render_widget(content, area);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: width.min(r.width), height: height.min(r.height) }
}
