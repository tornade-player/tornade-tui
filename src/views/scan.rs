use std::path::PathBuf;
use ratatui::{Frame, layout::{Alignment, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Gauge, Paragraph}};
use tornade_core::services::ScanProgress;
use crate::utils::truncate;

pub struct ScanState {
    pub path: PathBuf,
    pub progress: Option<ScanProgress>,
    pub is_complete: bool,
    pub error: Option<String>,
}

impl ScanState {
    pub fn new(path: PathBuf) -> Self {
        Self { path, progress: None, is_complete: false, error: None }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let path_str = self.path.to_string_lossy();
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled("Scanning library", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(truncate(&path_str, 60), Style::default().fg(Color::Gray))),
            Line::from(""),
        ];

        if let Some(ref err) = self.error {
            lines.push(Line::from(Span::styled(format!("Error: {}", err), Style::default().fg(Color::Red))));
        } else if self.is_complete {
            lines.push(Line::from(Span::styled("Scan complete!", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
            if let Some(ref p) = self.progress {
                lines.push(Line::from(Span::styled(format!("{} files processed", p.processed_files), Style::default().fg(Color::Gray))));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Press  q  to return to library", Style::default().fg(Color::DarkGray))));
        } else if let Some(ref p) = self.progress {
            lines.push(Line::from(Span::raw(format!("{} / {} files", p.processed_files, p.total_files))));
            if let Some(ref cur) = p.current_file {
                lines.push(Line::from(Span::styled(
                    truncate(&cur.to_string_lossy(), 60),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled("Starting scan...", Style::default().fg(Color::DarkGray))));
        }

        let para = Paragraph::new(lines)
            .block(Block::default())
            .alignment(Alignment::Center);
        frame.render_widget(para, area);

        // Progress gauge
        if let Some(ref p) = self.progress {
            if p.total_files > 0 && !self.is_complete {
                let pct = (p.processed_files as f64 / p.total_files as f64 * 100.0) as u16;
                let gauge_area = ratatui::layout::Rect {
                    x: area.x + 4,
                    y: area.y + area.height.saturating_sub(5),
                    width: area.width.saturating_sub(8),
                    height: 1,
                };
                let gauge = Gauge::default()
                    .gauge_style(Style::default().fg(Color::Cyan))
                    .percent(pct);
                frame.render_widget(gauge, gauge_area);
            }
        }
    }
}
