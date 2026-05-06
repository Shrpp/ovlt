use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

const SECTIONS: &[&str] = &[
    "Password Policy",
    "Lockout",
    "Token TTL",
    "Registration",
    "SMTP",
];

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let s = &app.settings;

    let outer = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(outer, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    // Total rows: 1 blank + 5 sections + 1 blank + 1 hint
    let section_count = SECTIONS.len();
    let mut constraints = vec![Constraint::Length(1)]; // top padding
    for _ in 0..section_count {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(0)); // spacer
    constraints.push(Constraint::Length(1)); // hint

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, name) in SECTIONS.iter().enumerate() {
        let is_sel = i == s.section_selected;
        let (bullet, bullet_color) = if is_sel {
            ("▶", Color::Cyan)
        } else {
            ("○", Color::DarkGray)
        };
        let label_color = if is_sel {
            Color::White
        } else {
            Color::DarkGray
        };
        let line = Line::from(vec![
            Span::styled(format!("   {bullet} "), Style::default().fg(bullet_color)),
            Span::styled(*name, Style::default().fg(label_color)),
        ]);
        frame.render_widget(Paragraph::new(line), chunks[i + 1]);
    }

    let hint_idx = section_count + 2;
    let hint = Paragraph::new(Span::styled(
        "↑↓ Navigate   Enter Edit",
        Style::default().fg(Color::DarkGray),
    ))
    .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[hint_idx]);
}
