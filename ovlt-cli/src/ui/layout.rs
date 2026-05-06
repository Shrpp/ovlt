use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, Focus, Tab};

/// Returns (sidebar, content, header, statusbar)
pub fn split_areas(frame: &Frame) -> (Rect, Rect, Rect, Rect) {
    let size = frame.area();
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // 1-line header + 2 borders
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(0)])
        .split(outer[1]);

    (body[0], body[1], outer[0], outer[2])
}

/// Splits content into (tab_bar 1 line, content_body)
pub fn split_content(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    (chunks[0], chunks[1])
}

pub fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let brand = Paragraph::new(Line::from(vec![
        Span::styled(
            "ovlt",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "/",
            Style::default()
                .fg(Color::Rgb(0, 255, 255))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(brand, inner);

    let connected = app.health_status.as_deref().unwrap_or("connecting…");
    let dot_color = if app.health_error.is_some() {
        Color::Red
    } else if app.health_status.is_some() {
        Color::Green
    } else {
        Color::Yellow
    };
    let status = Paragraph::new(Line::from(vec![
        Span::styled("● ", Style::default().fg(dot_color)),
        Span::styled(connected, Style::default().fg(dot_color)),
    ]))
    .alignment(Alignment::Right);
    frame.render_widget(status, inner);
}

pub fn render_tenant_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = if app.tenants.is_empty() {
        vec![ListItem::new("  (empty)").style(Style::default().fg(Color::DarkGray))]
    } else {
        app.tenants
            .iter()
            .map(|t| ListItem::new(format!(" {}", t.name)))
            .collect()
    };

    let mut state = ListState::default();
    if !app.tenants.is_empty() {
        state.select(Some(app.tenant_selected));
    }

    let highlight_style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Tenants ")
        .border_style(border_style);

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style)
        .highlight_symbol("▶");

    frame.render_stateful_widget(list, area, &mut state);
}

pub fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Content;
    let selected = match app.tab {
        Tab::Clients => 0,
        Tab::Users => 1,
        Tab::Roles => 2,
        Tab::Permissions => 3,
        Tab::Sessions => 4,
        Tab::Settings => 5,
        Tab::IdentityProviders => 6,
        Tab::AuditLog => 7,
    };
    let base_style = if focused {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let highlight_style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let titles: Vec<Line> = [
        "Clients",
        "Users",
        "Roles",
        "Permissions",
        "Sessions",
        "Settings",
        "IdP",
        "Audit",
    ]
    .iter()
    .map(|t| Line::from(*t))
    .collect();
    let tabs = Tabs::new(titles)
        .select(selected)
        .style(base_style)
        .highlight_style(highlight_style)
        .divider(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
    frame.render_widget(tabs, area);
}
