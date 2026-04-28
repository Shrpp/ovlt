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
        .title(" Roles ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if app.roles_loading {
        let loading = ratatui::widgets::Paragraph::new("Loading…").block(block);
        frame.render_widget(loading, area);
        return;
    }

    if app.roles.is_empty() {
        let empty = ratatui::widgets::Paragraph::new("No roles — press n to create one")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(empty, area);
        return;
    }

    list_state.select(Some(app.role_selected));

    let items: Vec<ListItem> = app
        .roles
        .iter()
        .map(|r| {
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<24}", r.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", r.description),
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
