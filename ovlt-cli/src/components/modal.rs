use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

/// Center a rect of given width/height within parent.
pub fn centered_rect(width: u16, height: u16, parent: Rect) -> Rect {
    let x = parent.x + parent.width.saturating_sub(width) / 2;
    let y = parent.y + parent.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(parent.width),
        height: height.min(parent.height),
    }
}

pub fn render_confirm(frame: &mut Frame, label: &str) {
    let area = centered_rect(50, 7, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let text = vec![
        Line::from(""),
        Line::from(format!("Delete: {label}")),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[y]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes   "),
            Span::styled("[n]", Style::default().fg(Color::Green)),
            Span::raw(" No"),
        ]),
    ];

    let para = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(para, area);
}

pub fn render_error(frame: &mut Frame, msg: &str) {
    let area = centered_rect(60, 7, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Error ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let para = Paragraph::new(msg)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(para, area);
}

pub fn render_secret(frame: &mut Frame, client_id: &str, secret: &str) {
    let area = centered_rect(70, 9, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Client Secret (shown once) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let text = vec![
        Line::from(""),
        Line::from(format!("Client ID: {client_id}")),
        Line::from(""),
        Line::from(Span::styled(
            secret,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[Enter] Close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let para = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(para, area);
}

/// Comprehensive user editor: email, password, is_active, roles (toggle), permissions (read-only).
/// field: 0=email, 1=password, 2=is_active, 3=roles section
#[allow(clippy::too_many_arguments)]
pub fn render_edit_user(
    frame: &mut Frame,
    email: &str,
    password: &str,
    is_active: bool,
    all_roles: &[(String, String, bool)],
    permissions: &[String],
    passkeys: &[crate::api::PasskeyInfo],
    field: usize,
    role_selected: usize,
    passkey_selected: usize,
) {
    let roles_visible = all_roles.len().clamp(1, 5) as u16;
    let perms_visible = permissions.len().clamp(1, 4) as u16;
    let pk_visible = passkeys.len().clamp(1, 3) as u16;
    let height = 3 + 3 + 2 + 1 + roles_visible + 1 + perms_visible + 1 + pk_visible + 1 + 2;
    let area = centered_rect(66, height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Edit User ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width - 4,
        height: area.height - 2,
    };

    let constraints = vec![
        Constraint::Length(3),             // email field
        Constraint::Length(3),             // password field
        Constraint::Length(2),             // is_active + separator
        Constraint::Length(1),             // roles header
        Constraint::Length(roles_visible), // roles list
        Constraint::Length(1),             // permissions header
        Constraint::Length(perms_visible), // permissions
        Constraint::Length(1),             // passkeys header
        Constraint::Length(pk_visible),    // passkeys list
        Constraint::Length(1),             // hint
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Email
    let email_active = field == 0;
    let email_block = Block::default()
        .title("Email")
        .borders(Borders::ALL)
        .border_style(if email_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });
    let email_display = if email_active {
        format!("{email}█")
    } else {
        email.to_string()
    };
    frame.render_widget(Paragraph::new(email_display).block(email_block), chunks[0]);

    // Password
    let pw_active = field == 1;
    let pw_block = Block::default()
        .title("Password (leave blank to keep)")
        .borders(Borders::ALL)
        .border_style(if pw_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });
    let pw_display = if pw_active {
        format!("{}█", "•".repeat(password.len()))
    } else {
        "•".repeat(password.len())
    };
    frame.render_widget(Paragraph::new(pw_display).block(pw_block), chunks[1]);

    // is_active + separator
    let status_active = field == 2;
    let status_color = if is_active { Color::Green } else { Color::Red };
    let status_text = if is_active {
        "● active"
    } else {
        "○ inactive"
    };
    let active_indicator = if status_active { " ◀ focused" } else { "" };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Status  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  [Space]{active_indicator}"),
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        chunks[2],
    );

    // Roles header
    let roles_active = field == 3;
    let roles_label = if roles_active {
        "── Roles [Space=toggle] ──"
    } else {
        "── Roles ──"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            roles_label,
            Style::default().fg(if roles_active {
                Color::Cyan
            } else {
                Color::DarkGray
            }),
        )),
        chunks[3],
    );

    // Roles list
    let visible_start = role_selected.saturating_sub(roles_visible.saturating_sub(1) as usize);
    let role_items: Vec<ListItem> = all_roles
        .iter()
        .skip(visible_start)
        .take(roles_visible as usize)
        .enumerate()
        .map(|(i, (_id, name, assigned))| {
            let actual_idx = visible_start + i;
            let is_sel = roles_active && actual_idx == role_selected;
            let bullet = if *assigned { "●" } else { "○" };
            let color = if *assigned {
                Color::Cyan
            } else {
                Color::DarkGray
            };
            let bg = if is_sel {
                Color::DarkGray
            } else {
                Color::Reset
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{bullet} "), Style::default().fg(color)),
                Span::styled(name.as_str(), Style::default().fg(Color::White).bg(bg)),
            ]))
        })
        .collect();
    let roles_list = List::new(role_items);
    let mut dummy_state = ListState::default();
    frame.render_stateful_widget(roles_list, chunks[4], &mut dummy_state);

    // Permissions header
    frame.render_widget(
        Paragraph::new(Span::styled(
            "── Permissions (from roles, read-only) ──",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[5],
    );

    // Permissions list
    let perm_items: Vec<ListItem> = if permissions.is_empty() {
        vec![ListItem::new(Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        permissions
            .iter()
            .take(perms_visible as usize)
            .map(|p| {
                ListItem::new(Line::from(vec![
                    Span::styled("  ● ", Style::default().fg(Color::Yellow)),
                    Span::styled(p.as_str(), Style::default().fg(Color::White)),
                ]))
            })
            .collect()
    };
    let mut dummy_state2 = ListState::default();
    frame.render_stateful_widget(List::new(perm_items), chunks[6], &mut dummy_state2);

    // Passkeys header
    let pk_active = field == 4;
    let pk_label = if pk_active {
        "── Passkeys [d=delete] ──"
    } else {
        "── Passkeys ──"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            pk_label,
            Style::default().fg(if pk_active {
                Color::Cyan
            } else {
                Color::DarkGray
            }),
        )),
        chunks[7],
    );

    // Passkeys list
    let pk_items: Vec<ListItem> = if passkeys.is_empty() {
        vec![ListItem::new(Span::styled(
            "  (none registered)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        let visible_start = passkey_selected.saturating_sub(pk_visible.saturating_sub(1) as usize);
        passkeys
            .iter()
            .enumerate()
            .skip(visible_start)
            .take(pk_visible as usize)
            .map(|(i, pk)| {
                let is_sel = pk_active && i == passkey_selected;
                let last = pk
                    .last_used_at
                    .as_deref()
                    .map(|s| &s[..10])
                    .unwrap_or("never");
                let bg = if is_sel {
                    Color::DarkGray
                } else {
                    Color::Reset
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if is_sel { "▶ " } else { "  " },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(pk.name.as_str(), Style::default().fg(Color::White).bg(bg)),
                    Span::styled(
                        format!("  last: {last}"),
                        Style::default().fg(Color::DarkGray).bg(bg),
                    ),
                ]))
            })
            .collect()
    };
    let mut dummy_pk = ListState::default();
    frame.render_stateful_widget(List::new(pk_items), chunks[8], &mut dummy_pk);

    // Hints
    let hints = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" Section  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Save  ", Style::default().fg(Color::DarkGray)),
        Span::styled("d", Style::default().fg(Color::Cyan)),
        Span::styled(" Del passkey  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        chunks[9],
    );
}

/// Role editor: name, description, permissions (toggle).
/// field: 0=name, 1=description, 2=permissions section
pub fn render_edit_role(
    frame: &mut Frame,
    name: &str,
    description: &str,
    all_permissions: &[(String, String, bool)],
    field: usize,
    perm_selected: usize,
) {
    let perms_visible = all_permissions.len().clamp(1, 6) as u16;
    let height = 3 + 3 + 1 + perms_visible + 1 + 1 + 2; // name+desc+header+perms+spacer+hints+borders
    let area = centered_rect(60, height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Edit Role ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width - 4,
        height: area.height - 2,
    };

    let constraints = vec![
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(perms_visible),
        Constraint::Min(1),
        Constraint::Length(1),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Name
    let name_active = field == 0;
    let name_block = Block::default()
        .title("Name")
        .borders(Borders::ALL)
        .border_style(if name_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });
    frame.render_widget(
        Paragraph::new(if name_active {
            format!("{name}█")
        } else {
            name.to_string()
        })
        .block(name_block),
        chunks[0],
    );

    // Description
    let desc_active = field == 1;
    let desc_block = Block::default()
        .title("Description")
        .borders(Borders::ALL)
        .border_style(if desc_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });
    frame.render_widget(
        Paragraph::new(if desc_active {
            format!("{description}█")
        } else {
            description.to_string()
        })
        .block(desc_block),
        chunks[1],
    );

    // Permissions header
    let perms_active = field == 2;
    let perms_label = if perms_active {
        "── Permissions [Space=toggle] ──"
    } else {
        "── Permissions ──"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            perms_label,
            Style::default().fg(if perms_active {
                Color::Cyan
            } else {
                Color::DarkGray
            }),
        )),
        chunks[2],
    );

    // Permissions list
    let visible_start = perm_selected.saturating_sub(perms_visible.saturating_sub(1) as usize);
    let perm_items: Vec<ListItem> = if all_permissions.is_empty() {
        vec![ListItem::new(Span::styled(
            "  (no permissions defined yet)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        all_permissions
            .iter()
            .skip(visible_start)
            .take(perms_visible as usize)
            .enumerate()
            .map(|(i, (_id, name, assigned))| {
                let actual_idx = visible_start + i;
                let is_sel = perms_active && actual_idx == perm_selected;
                let bullet = if *assigned { "●" } else { "○" };
                let color = if *assigned {
                    Color::Yellow
                } else {
                    Color::DarkGray
                };
                let bg = if is_sel {
                    Color::DarkGray
                } else {
                    Color::Reset
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{bullet} "), Style::default().fg(color)),
                    Span::styled(name.as_str(), Style::default().fg(Color::White).bg(bg)),
                ]))
            })
            .collect()
    };
    let mut dummy = ListState::default();
    frame.render_stateful_widget(List::new(perm_items), chunks[3], &mut dummy);

    // Hints
    let hints = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" Section  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Save  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        chunks[5],
    );
}

/// (role_id, role_name, is_assigned)
pub fn render_user_roles(
    frame: &mut Frame,
    email: &str,
    all_roles: &[(String, String, bool)],
    selected: usize,
) {
    let height = (all_roles.len() as u16 + 6).max(8);
    let area = centered_rect(56, height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Roles — {} ", email))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width - 4,
        height: area.height - 2,
    };

    let mut constraints: Vec<ratatui::layout::Constraint> = all_roles
        .iter()
        .map(|_| ratatui::layout::Constraint::Length(1))
        .collect();
    constraints.push(ratatui::layout::Constraint::Min(1));
    constraints.push(ratatui::layout::Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, (_id, name, assigned)) in all_roles.iter().enumerate() {
        let is_sel = i == selected;
        let bullet = if *assigned { "●" } else { "○" };
        let color = if *assigned {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let bg = if is_sel {
            Color::DarkGray
        } else {
            Color::Reset
        };
        let line = Line::from(vec![
            Span::styled(format!("{bullet} "), Style::default().fg(color)),
            Span::styled(name.as_str(), Style::default().fg(Color::White).bg(bg)),
        ]);
        frame.render_widget(Paragraph::new(line), chunks[i]);
    }

    let hint_idx = all_roles.len() + 1;
    let hints = Line::from(vec![
        Span::styled("Space", Style::default().fg(Color::Cyan)),
        Span::styled(" Toggle   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Save   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        chunks[hint_idx],
    );
}

/// Create-client modal with text fields + visual type toggle.
pub fn render_create_client(
    frame: &mut Frame,
    name: &str,
    redirect_uri: &str,
    scopes: &str,
    client_type: u8,
    field: usize,
) {
    let area = centered_rect(62, 17, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" New Client ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width - 4,
        height: area.height - 2,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Length(3), // Redirect URI
            Constraint::Length(3), // Scopes
            Constraint::Length(3), // Type toggle
            Constraint::Min(1),
            Constraint::Length(1), // hints
        ])
        .split(inner);

    let border = |active: bool| {
        if active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    // Name
    let a = field == 0;
    frame.render_widget(
        Paragraph::new(if a {
            format!("{name}█")
        } else {
            name.to_string()
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Name")
                .border_style(border(a)),
        ),
        chunks[0],
    );

    // Redirect URI
    let a = field == 1;
    frame.render_widget(
        Paragraph::new(if a {
            format!("{redirect_uri}█")
        } else {
            redirect_uri.to_string()
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Redirect URI")
                .border_style(border(a)),
        ),
        chunks[1],
    );

    // Scopes
    let a = field == 2;
    frame.render_widget(
        Paragraph::new(if a {
            format!("{scopes}█")
        } else {
            scopes.to_string()
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Scopes")
                .border_style(border(a)),
        ),
        chunks[2],
    );

    // Type toggle — shows all 3 options, selected one in cyan bold
    let type_active = field == 3;
    let title = if type_active { "Type  ←/→" } else { "Type" };
    let labels = ["Confidential", "SPA/Mobile", "Machine (M2M)"];
    let mut spans: Vec<Span> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        if i as u8 == client_type {
            spans.push(Span::styled(
                format!("● {label}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("○ {label}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border(type_active)),
        ),
        chunks[3],
    );

    // Hints
    let hints = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
        Span::styled("←/→", Style::default().fg(Color::Cyan)),
        Span::styled(" Type   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Create   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        chunks[5],
    );
}

/// Render a simple form modal with labelled fields.
/// `fields`: list of `(label, value, placeholder)` triples.
/// `active_field`: index of the currently focused input.
pub fn render_form(
    frame: &mut Frame,
    title: &str,
    fields: &[(&str, &str, &str)],
    active_field: usize,
) {
    let height = (fields.len() as u16) * 3 + 5;
    let area = centered_rect(62, height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width - 4,
        height: area.height - 2,
    };

    let mut constraints = vec![];
    for _ in fields {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, (label, value, placeholder)) in fields.iter().enumerate() {
        let is_active = i == active_field;
        let border_style = if is_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let input_block = Block::default()
            .title(*label)
            .borders(Borders::ALL)
            .border_style(border_style);

        let para = if is_active {
            Paragraph::new(format!("{value}█")).block(input_block)
        } else if value.is_empty() && !placeholder.is_empty() {
            Paragraph::new(Span::styled(
                *placeholder,
                Style::default().fg(Color::DarkGray),
            ))
            .block(input_block)
        } else {
            Paragraph::new(value.to_string()).block(input_block)
        };

        frame.render_widget(para, chunks[i]);
    }

    let hint_area = chunks[fields.len()];
    let hint = Paragraph::new("[Tab] Next field   [Enter] Submit   [Esc] Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, hint_area);
}

/// Edit client modal: text fields + type toggle (field 5).
#[allow(clippy::too_many_arguments)]
pub fn render_edit_client(
    frame: &mut Frame,
    name: &str,
    redirect_uris: &str,
    scopes: &str,
    access_token_ttl: &str,
    refresh_token_ttl: &str,
    client_type: u8,
    field: usize,
) {
    let area = centered_rect(64, 23, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Edit Client ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width - 4,
        height: area.height - 2,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Length(3), // Redirect URIs
            Constraint::Length(3), // Scopes
            Constraint::Length(3), // Access token TTL
            Constraint::Length(3), // Refresh token TTL
            Constraint::Length(3), // Type toggle
            Constraint::Min(1),
            Constraint::Length(1), // hints
        ])
        .split(inner);

    let border = |active: bool| {
        if active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    let text_field = |val: &str, placeholder: &str, active: bool| -> Paragraph<'_> {
        if active {
            Paragraph::new(format!("{val}█"))
        } else if val.is_empty() {
            Paragraph::new(Span::styled(
                placeholder.to_string(),
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            Paragraph::new(val.to_string())
        }
    };

    frame.render_widget(
        text_field(name, "e.g. My App", field == 0).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Name")
                .border_style(border(field == 0)),
        ),
        chunks[0],
    );
    frame.render_widget(
        text_field(
            redirect_uris,
            "https://app.example.com/callback",
            field == 1,
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Redirect URIs (comma-separated)")
                .border_style(border(field == 1)),
        ),
        chunks[1],
    );
    frame.render_widget(
        text_field(scopes, "openid email profile", field == 2).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Scopes")
                .border_style(border(field == 2)),
        ),
        chunks[2],
    );
    frame.render_widget(
        text_field(access_token_ttl, "blank = tenant default", field == 3).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Access Token TTL (minutes, blank = default)")
                .border_style(border(field == 3)),
        ),
        chunks[3],
    );
    frame.render_widget(
        text_field(refresh_token_ttl, "blank = tenant default", field == 4).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Refresh Token TTL (days, blank = default)")
                .border_style(border(field == 4)),
        ),
        chunks[4],
    );

    let type_active = field == 5;
    let title = if type_active {
        "Type  ←/→ or Space"
    } else {
        "Type"
    };
    let labels = ["Confidential", "SPA/Mobile", "Machine (M2M)"];
    let mut spans: Vec<Span> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        if i as u8 == client_type {
            spans.push(Span::styled(
                format!("● {label}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("○ {label}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border(type_active)),
        ),
        chunks[5],
    );

    let hints = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
        Span::styled("←/→", Style::default().fg(Color::Cyan)),
        Span::styled(" Type   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Save   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        chunks[7],
    );
}
