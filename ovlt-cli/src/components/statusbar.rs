use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, hints: &[(&str, &str)], status: Option<&str>) {
    let mut spans: Vec<Span> = vec![];

    for (key, desc) in hints {
        spans.push(Span::styled(
            format!("[{key}]"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {desc}  ")));
    }

    if let Some(msg) = status {
        spans.push(Span::styled(
            format!("  ● {msg}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}
