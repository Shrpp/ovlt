use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, AppMode};

pub fn render(frame: &mut Frame, app: &App) {
    let AppMode::Login {
        email,
        password,
        slug,
        slug_idx,
        field,
        error,
    } = &app.mode
    else {
        return;
    };

    let opts = &app.tenant_options;
    let has_opts = !opts.is_empty();

    let size = frame.area();

    let box_w: u16 = 52;
    // List always visible when tenants are available.
    let dropdown_visible = has_opts;
    let dropdown_rows = opts.len().min(8) as u16;
    // +1 for the bottom border of the list block
    let list_height = dropdown_rows + 1;
    let box_h: u16 = 19 + if dropdown_visible { list_height } else { 0 };
    let area = Rect {
        x: size.x + size.width.saturating_sub(box_w) / 2,
        y: size.y + size.height.saturating_sub(box_h) / 2,
        width: box_w.min(size.width),
        height: box_h.min(size.height),
    };

    frame.render_widget(Clear, area);

    let border_block = Block::default()
        .title(" OVTL Admin ")
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
        Constraint::Length(3), // tenant selector
    ];
    if dropdown_visible {
        constraints.push(Constraint::Length(list_height)); // items + bottom border
    }
    constraints.push(Constraint::Length(1)); // spacer
    constraints.push(Constraint::Length(1)); // error
    constraints.push(Constraint::Min(1));    // hints

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
    let email_val = if email_active { format!("{email}█") } else { email.clone() };
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
    let pass_val = if pass_active { format!("{masked}█") } else { masked };
    frame.render_widget(
        Paragraph::new(pass_val).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Password")
                .border_style(border_style(pass_active)),
        ),
        chunks[3],
    );

    // Tenant selector
    let tenant_active = *field == 2;
    let tenant_title = if tenant_active && has_opts {
        "Tenant  ↑/↓"
    } else {
        "Tenant"
    };
    let tenant_display = if has_opts {
        // Show selected name + slug
        let (s, n) = &opts[*slug_idx];
        format!("{n}  ({s})")
    } else {
        // Fallback: editable text
        if tenant_active {
            format!("{slug}█")
        } else {
            slug.clone()
        }
    };
    // When list is visible, drop the bottom border so the list connects seamlessly.
    let tenant_borders = if dropdown_visible {
        Borders::LEFT | Borders::RIGHT | Borders::TOP
    } else {
        Borders::ALL
    };
    frame.render_widget(
        Paragraph::new(tenant_display).block(
            Block::default()
                .borders(tenant_borders)
                .title(tenant_title)
                .border_style(border_style(tenant_active)),
        ),
        chunks[4],
    );

    // Tenant list — always visible when tenants are available.
    let mut chunk_offset = 5usize;
    if dropdown_visible {
        let tenant_active = *field == 2;
        let visible_start = (*slug_idx).saturating_sub(dropdown_rows.saturating_sub(1) as usize);
        let items: Vec<ListItem> = opts
            .iter()
            .enumerate()
            .skip(visible_start)
            .take(dropdown_rows as usize)
            .map(|(i, (s, n))| {
                let selected = i == *slug_idx;
                let bullet = if selected { "●" } else { "○" };
                let (name_style, slug_style) = if selected {
                    (
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::DarkGray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::DarkGray),
                    )
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {bullet} "), Style::default().fg(if selected { Color::Cyan } else { Color::DarkGray })),
                    Span::styled(n.as_str(), name_style),
                    Span::styled(format!("  {s}"), slug_style),
                ]))
            })
            .collect();

        let list_border_style = if tenant_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let list = List::new(items).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(list_border_style),
        );
        frame.render_widget(list, chunks[chunk_offset]);
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

    let nav_hint = if has_opts {
        vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
            Span::styled(" Tenant   ", Style::default().fg(Color::DarkGray)),
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
