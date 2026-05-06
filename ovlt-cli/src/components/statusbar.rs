use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Focus, Modal, Tab};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let hints_str = match (&app.focus, &app.tab, &app.modal) {
        (_, _, Modal::EditUser { .. }) => "Tab Next  Enter Save  Esc Cancel",
        (_, _, Modal::EditSettings(_)) => "Tab Next  Enter Save  Esc Cancel",
        (_, _, Modal::CreateTenant { .. }) => "Tab Next  Enter Create  Esc Cancel",
        (_, _, Modal::ConfirmDelete { .. }) => "y Confirm  Any Cancel",
        (_, _, Modal::QuickStart(_)) => "Tab Next  Enter Continue  Esc Cancel",
        (_, _, Modal::PostCreateTenant { .. }) => "Enter Setup wizard  Esc Skip",
        (_, _, Modal::ShowSecret { .. }) | (_, _, Modal::Error(_)) => "Any Close",
        (_, _, Modal::ClientRoles { .. }) => {
            "↑↓ Navigate  Space Toggle  Enter Save  Esc Cancel"
        }
        (_, _, Modal::None) => match &app.focus {
            Focus::Sidebar => "↑↓ Navigate  Enter Open  n New  r Refresh  q Quit",
            Focus::Content => match &app.tab {
                Tab::Settings => "↑↓ Navigate  Enter Edit  r Refresh  Esc Back",
                Tab::Sessions => "↑↓ Navigate  d Delete  r Refresh  Esc Back",
                Tab::AuditLog => "↑↓ Navigate  r Refresh  Esc Back",
                Tab::Users => {
                    "↑↓ Navigate  n New  e Edit  d Deactivate  r Refresh  Esc Back"
                }
                Tab::Clients => {
                    "↑↓ Navigate  n New  e Edit  l Roles  d Delete  r Refresh  Esc Back"
                }
                _ => "↑↓ Navigate  n New  e Edit  d Delete  r Refresh  Esc Back",
            },
        },
        _ => "",
    };

    let mut spans: Vec<Span> = vec![Span::styled(
        hints_str,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];

    if let Some(msg) = app.status_msg.as_deref() {
        spans.push(Span::styled(
            format!("  ● {msg}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}
