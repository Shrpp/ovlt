use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect, list_state: &mut ListState) {
    let block = Block::default()
        .title(" Active Sessions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if app.sessions_loading {
        let loading = ratatui::widgets::Paragraph::new("Loading…").block(block);
        frame.render_widget(loading, area);
        return;
    }

    if app.sessions.is_empty() {
        let empty = ratatui::widgets::Paragraph::new("No active sessions")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(empty, area);
        return;
    }

    list_state.select(Some(app.session_selected));

    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|s| {
            let last_seen = s.last_seen_at.get(..16).unwrap_or(&s.last_seen_at);
            let ip = s.ip.as_deref().unwrap_or("—");
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<32}", s.email),
                    Style::default().fg(Color::White),
                ),
                Span::styled(format!(" {:<16}", ip), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!(" last: {last_seen}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, list_state);
}
