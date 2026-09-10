use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState},
};

pub fn render(frame: &mut Frame, input: &str, completions: &[String], selected: usize, area: Rect) {
    let max_visible = 8usize.min(completions.len());
    let list_height = if completions.is_empty() {
        0
    } else {
        max_visible as u16 + 1 // + separator line
    };
    let height = 3 + list_height; // border(2) + input line(1) + completions
    let modal_area = centered_rect(60, height, area);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Command ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }));

    let input_line = Line::from(vec![
        Span::styled(
            ":",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(input, Style::default().fg(Color::White)),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(ratatui::widgets::Paragraph::new(input_line), chunks[0]);

    if !completions.is_empty() {
        let items: Vec<ListItem> = completions
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let style = if i == selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(Line::from(Span::styled(format!(" :{} ", c), style)))
            })
            .collect();
        let list = List::new(items).block(Block::default());
        // Keep the selected row scrolled into view; ratatui's ListState offset
        // handles the actual scrolling, no scrollbar widget needed.
        let mut state = ListState::default().with_selected(Some(selected));
        frame.render_stateful_widget(list, chunks[1], &mut state);
    }
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 3; // sit in the upper-middle, LazyVim-style
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}
