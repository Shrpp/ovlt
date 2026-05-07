use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, AppMode};

pub fn render(frame: &mut Frame, app: &App) {
    if let AppMode::MfaChallenge { code, error, .. } = &app.mode {
        render_mfa_challenge(frame, code, error.as_deref());
        return;
    }

    let AppMode::Login {
        email,
        password,
        field,
        error,
        tenant_choices,
        tenant_choice_idx,
    } = &app.mode
    else {
        return;
    };

    let size = frame.area();

    let choice_rows = tenant_choices
        .as_ref()
        .map(|t| t.len().min(8) as u16)
        .unwrap_or(0);
    let choice_visible = choice_rows > 0;
    let choice_height = if choice_visible { choice_rows + 2 } else { 0 }; // +2 for borders

    let box_w: u16 = 52;
    let box_h: u16 = 15 + choice_height;
    let area = Rect {
        x: size.x + size.width.saturating_sub(box_w) / 2,
        y: size.y + size.height.saturating_sub(box_h) / 2,
        width: box_w.min(size.width),
        height: box_h.min(size.height),
    };

    frame.render_widget(Clear, area);

    let border_block = Block::default()
        .title(Line::from(vec![
            Span::raw(" "),
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
            Span::raw(" "),
        ]))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(border_block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    let mut constraints = vec![
        Constraint::Length(1), // subtitle
        Constraint::Length(1), // spacer
        Constraint::Length(3), // email
        Constraint::Length(3), // password
    ];
    if choice_visible {
        constraints.push(Constraint::Length(choice_height));
    }
    constraints.push(Constraint::Length(1)); // spacer
    constraints.push(Constraint::Length(1)); // error
    constraints.push(Constraint::Min(1)); // hints

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let subtitle = Paragraph::new("Sign in to continue")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(subtitle, chunks[0]);

    let border_style = |active: bool| {
        if active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    // Email
    let email_active = *field == 0;
    let email_val = if email_active {
        format!("{email}█")
    } else {
        email.clone()
    };
    frame.render_widget(
        Paragraph::new(email_val).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Email")
                .border_style(border_style(email_active)),
        ),
        chunks[2],
    );

    // Password
    let pass_active = *field == 1;
    let masked = "•".repeat(password.len());
    let pass_val = if pass_active {
        format!("{masked}█")
    } else {
        masked
    };
    frame.render_widget(
        Paragraph::new(pass_val).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Password")
                .border_style(border_style(pass_active)),
        ),
        chunks[3],
    );

    // Tenant choice list (shown when server returns multiple tenants)
    let mut chunk_offset = 4usize;
    if choice_visible {
        if let Some(choices) = tenant_choices {
            let items: Vec<ListItem> = choices
                .iter()
                .enumerate()
                .map(|(i, (slug, name))| {
                    let selected = i == *tenant_choice_idx;
                    let bullet = if selected { "●" } else { "○" };
                    let (name_style, slug_style) = if selected {
                        (
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            Style::default().fg(Color::DarkGray),
                        )
                    } else {
                        (
                            Style::default().fg(Color::White),
                            Style::default().fg(Color::DarkGray),
                        )
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!(" {bullet} "),
                            Style::default().fg(if selected {
                                Color::Cyan
                            } else {
                                Color::DarkGray
                            }),
                        ),
                        Span::styled(name.as_str(), name_style),
                        Span::styled(format!("  {slug}"), slug_style),
                    ]))
                })
                .collect();

            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Multiple accounts found — pick one")
                    .border_style(Style::default().fg(Color::Yellow)),
            );
            frame.render_widget(list, chunks[chunk_offset]);
        }
        chunk_offset += 1;
    }

    let err_idx = chunk_offset + 1;
    let hint_idx = chunk_offset + 2;

    if let Some(err) = error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                err.as_str(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            chunks[err_idx],
        );
    }

    let nav_hint = if choice_visible {
        vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
            Span::styled(" Pick tenant   ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::styled(" Login   ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
        ]
    } else {
        vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::styled(" Login   ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
        ]
    };
    frame.render_widget(
        Paragraph::new(Line::from(nav_hint)).alignment(Alignment::Center),
        chunks[hint_idx],
    );
}

fn render_mfa_challenge(frame: &mut Frame, code: &str, error: Option<&str>) {
    let size = frame.area();
    let box_w: u16 = 48;
    let box_h: u16 = 13;
    let area = Rect {
        x: size.x + size.width.saturating_sub(box_w) / 2,
        y: size.y + size.height.saturating_sub(box_h) / 2,
        width: box_w.min(size.width),
        height: box_h.min(size.height),
    };

    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .title(" Two-Factor Authentication ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
        area,
    );

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // description
            Constraint::Length(3), // code input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // error
            Constraint::Min(1),    // hints
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new("Enter the 6-digit code from your authenticator app.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[0],
    );

    let code_display = format!("{code}█");
    frame.render_widget(
        Paragraph::new(code_display).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Code")
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        chunks[1],
    );

    if let Some(err) = error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                err,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            chunks[3],
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::styled(" Verify   ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::styled(" Back", Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(Alignment::Center),
        chunks[4],
    );
}
