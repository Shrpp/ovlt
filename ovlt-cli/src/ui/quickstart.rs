use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

fn render_type_toggle(frame: &mut Frame, area: Rect, client_type: u8, active: bool) {
    let border_style = if active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = if active { "Type  ←/→" } else { "Type" };
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
                .border_style(border_style),
        ),
        area,
    );
}

use crate::app::{App, Modal, QuickStartState};

pub fn render(frame: &mut Frame, app: &App) {
    let Modal::QuickStart(qs) = &app.modal else {
        return;
    };

    let size = frame.area();
    let box_w: u16 = 68;
    let box_h: u16 = if qs.step == 4 { 16 } else { 20 };

    let area = Rect {
        x: size.x + size.width.saturating_sub(box_w) / 2,
        y: size.y + size.height.saturating_sub(box_h) / 2,
        width: box_w.min(size.width),
        height: box_h.min(size.height),
    };

    frame.render_widget(Clear, area);

    let title = if qs.step == 4 {
        " Quick Start — Done! "
    } else {
        " Quick Start "
    };
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    match qs.step {
        1 => render_form_step(
            frame,
            inner,
            qs,
            "Step 1 of 4 — Create Tenant",
            1,
            &["Name", "Slug"],
            &[&qs.tenant_name, &qs.tenant_slug],
            &[false, false],
        ),
        2 => render_client_step(frame, inner, qs),
        3 => render_form_step(
            frame,
            inner,
            qs,
            "Step 3 of 4 — Create Admin User",
            3,
            &["Email", "Password"],
            &[&qs.user_email, &qs.user_password],
            &[false, true],
        ),
        4 => render_summary(frame, inner, qs, &app.client.base_url),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn render_form_step(
    frame: &mut Frame,
    area: Rect,
    qs: &QuickStartState,
    step_label: &str,
    step_num: u8,
    labels: &[&str],
    values: &[&str],
    masked: &[bool],
) {
    let n = labels.len() as u16;
    let mut constraints = vec![
        Constraint::Length(1), // step header
        Constraint::Length(1), // spacer
    ];
    for _ in 0..n {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(1)); // spacer
    constraints.push(Constraint::Length(1)); // error
    constraints.push(Constraint::Length(1)); // hints

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Header: step label + progress dots
    let dots = progress_dots(step_num);
    let header = Line::from(vec![
        Span::styled(
            step_label,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(dots, Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    // Fields
    for (i, (label, (value, is_masked))) in labels
        .iter()
        .zip(values.iter().zip(masked.iter()))
        .enumerate()
    {
        let chunk_idx = 2 + i;
        let active = qs.field == i;
        let border_style = if active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let display = if *is_masked {
            let m = "*".repeat(value.len());
            if active {
                format!("{m}█")
            } else {
                m
            }
        } else if active {
            format!("{value}█")
        } else {
            value.to_string()
        };
        let widget = Paragraph::new(display).block(
            Block::default()
                .borders(Borders::ALL)
                .title(*label)
                .border_style(border_style),
        );
        frame.render_widget(widget, chunks[chunk_idx]);
    }

    let err_idx = 2 + labels.len() + 1;
    let hint_idx = err_idx + 1;

    // Error
    if let Some(err) = &qs.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                err.as_str(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            chunks[err_idx],
        );
    }

    // Hints
    let hints = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Continue   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled(" Close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        chunks[hint_idx],
    );
}

fn render_client_step(frame: &mut Frame, area: Rect, qs: &QuickStartState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // spacer
            Constraint::Length(3), // Name
            Constraint::Length(3), // Redirect URI
            Constraint::Length(3), // Scopes
            Constraint::Length(3), // Type toggle
            Constraint::Min(1),
            Constraint::Length(1), // error
            Constraint::Length(1), // hints
        ])
        .split(area);

    let dots = progress_dots(2);
    let header = Line::from(vec![
        Span::styled(
            "Step 2 of 4 — Create OAuth Client",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(dots, Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    let fields = [
        ("Name", &qs.client_name, false, 0usize),
        ("Redirect URI", &qs.redirect_uri, false, 1),
        ("Scopes", &qs.scopes, false, 2),
    ];
    for (label, value, _masked, idx) in &fields {
        let active = qs.field == *idx;
        let border_style = if active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let display = if active {
            format!("{value}█")
        } else {
            value.to_string()
        };
        frame.render_widget(
            Paragraph::new(display).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(*label)
                    .border_style(border_style),
            ),
            chunks[2 + idx],
        );
    }

    render_type_toggle(frame, chunks[5], qs.client_type, qs.field == 3);

    if let Some(err) = &qs.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                err.as_str(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            chunks[7],
        );
    }

    let hints = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
        Span::styled("←/→", Style::default().fg(Color::Cyan)),
        Span::styled(" Type   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Continue   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled(" Close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        chunks[8],
    );
}

fn render_summary(frame: &mut Frame, area: Rect, qs: &QuickStartState, base_url: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // spacer
            Constraint::Length(1), // tenant
            Constraint::Length(1), // client name
            Constraint::Length(1), // client_id
            Constraint::Length(1), // secret
            Constraint::Length(1), // spacer
            Constraint::Length(1), // discovery
            Constraint::Min(1),    // spacer
            Constraint::Length(1), // hint
        ])
        .split(area);

    let tenant_line = Line::from(vec![
        Span::styled("✓ Tenant    ", Style::default().fg(Color::Green)),
        Span::styled(
            qs.created_tenant_name.as_deref().unwrap_or("—"),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(tenant_line), chunks[1]);

    let client_line = Line::from(vec![
        Span::styled("✓ Client    ", Style::default().fg(Color::Green)),
        Span::styled(
            &qs.client_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(client_line), chunks[2]);

    let cid = qs.created_client_id.as_deref().unwrap_or("—");
    let cid_line = Line::from(vec![
        Span::styled("  client_id ", Style::default().fg(Color::DarkGray)),
        Span::styled(cid, Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(cid_line), chunks[3]);

    let secret_display = match (&qs.created_secret, qs.show_secret) {
        (Some(s), true) => s.clone(),
        (Some(s), false) => "•".repeat(s.len().min(24)),
        (None, _) => "—".to_string(),
    };
    let secret_line = Line::from(vec![
        Span::styled("  secret    ", Style::default().fg(Color::DarkGray)),
        Span::styled(&secret_display, Style::default().fg(Color::Yellow)),
        Span::styled(
            if qs.show_secret {
                "  [ c → hide ]"
            } else {
                "  [ c → reveal ]"
            },
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(secret_line), chunks[4]);

    let discovery_url = format!("{base_url}/.well-known/openid-configuration");
    let disc_line = Line::from(vec![
        Span::styled("Discovery   ", Style::default().fg(Color::DarkGray)),
        Span::styled(discovery_url, Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(disc_line), chunks[6]);

    let hint = Line::from(vec![
        Span::styled("i", Style::default().fg(Color::Cyan)),
        Span::styled(" Copy ID   ", Style::default().fg(Color::DarkGray)),
        Span::styled("s", Style::default().fg(Color::Cyan)),
        Span::styled(" Copy secret   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Dashboard", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hint).alignment(Alignment::Center), chunks[8]);
}

fn progress_dots(current: u8) -> String {
    (1u8..=4)
        .map(|i| if i <= current { '●' } else { '○' })
        .collect()
}
