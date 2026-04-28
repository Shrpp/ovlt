use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, SettingsState};

const SECTIONS: &[&str] = &["Password Policy", "Lockout", "Tokens", "Registration"];

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let s = &app.settings;

    let border_color = if s.entered {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let outer = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(outer, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    render_section_tabs(frame, s, layout[0]);

    if s.entered {
        match s.section {
            0 => render_policy(frame, s, layout[2]),
            1 => render_lockout(frame, s, layout[2]),
            2 => render_tokens(frame, s, layout[2]),
            3 => render_registration(frame, s, layout[2]),
            _ => {}
        }
    } else {
        // Not yet entered — show a hint to press Enter
        let hint = Paragraph::new(Span::styled(
            "Press Enter to edit settings",
            Style::default().fg(Color::DarkGray),
        ))
        .alignment(Alignment::Center);
        frame.render_widget(hint, layout[2]);
    }
}

fn render_section_tabs(frame: &mut Frame, s: &SettingsState, area: Rect) {
    let mut spans = vec![Span::raw("  ")];
    for (i, name) in SECTIONS.iter().enumerate() {
        let idx = i as u8;
        let style = if idx == s.section && s.entered {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if idx == s.section {
            // Hovered but not entered
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!(" {name} "), style));
        spans.push(Span::raw("  "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn field_block(title: &str, active: bool) -> Block<'static> {
    Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .border_style(if active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn text_field<'a>(label: &'static str, val: &str, active: bool) -> Paragraph<'a> {
    let display = if active {
        format!("{val}█")
    } else {
        val.to_string()
    };
    Paragraph::new(display).block(field_block(label, active))
}

fn toggle_line(label: &str, val: bool, focused: bool) -> Paragraph<'static> {
    let (bullet, color) = if val {
        ("●", Color::Cyan)
    } else {
        ("○", Color::DarkGray)
    };
    let bg = if focused {
        Color::DarkGray
    } else {
        Color::Reset
    };
    Paragraph::new(Line::from(vec![
        Span::styled(format!("{bullet} "), Style::default().fg(color)),
        Span::styled(label.to_string(), Style::default().fg(Color::White).bg(bg)),
        Span::styled("  [Space]", Style::default().fg(Color::DarkGray)),
    ]))
}

fn hints<'a>() -> Paragraph<'a> {
    Paragraph::new(Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" Next   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Space", Style::default().fg(Color::Cyan)),
        Span::styled(" Toggle   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" Save   ", Style::default().fg(Color::DarkGray)),
        Span::styled("◄►", Style::default().fg(Color::Cyan)),
        Span::styled(" Section   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Backspace", Style::default().fg(Color::Cyan)),
        Span::styled(" Back", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center)
}

fn render_policy(frame: &mut Frame, s: &SettingsState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        text_field("Min Length", &s.policy_min_length, s.field == 0),
        chunks[0],
    );
    frame.render_widget(
        toggle_line(
            "Require Uppercase",
            s.policy_require_uppercase,
            s.field == 1,
        ),
        chunks[2],
    );
    frame.render_widget(
        toggle_line("Require Digit", s.policy_require_digit, s.field == 2),
        chunks[3],
    );
    frame.render_widget(
        toggle_line(
            "Require Special (!@#...)",
            s.policy_require_special,
            s.field == 3,
        ),
        chunks[4],
    );
    frame.render_widget(hints(), chunks[6]);
}

fn render_lockout(frame: &mut Frame, s: &SettingsState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        text_field("Max Attempts", &s.lockout_max_attempts, s.field == 0),
        chunks[0],
    );
    frame.render_widget(
        text_field("Window (minutes)", &s.lockout_window_minutes, s.field == 1),
        chunks[1],
    );
    frame.render_widget(
        text_field(
            "Lockout Duration (minutes)",
            &s.lockout_duration_minutes,
            s.field == 2,
        ),
        chunks[2],
    );
    frame.render_widget(hints(), chunks[4]);
}

fn render_tokens(frame: &mut Frame, s: &SettingsState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        text_field(
            "Access Token TTL (minutes)",
            &s.access_token_ttl_minutes,
            s.field == 0,
        ),
        chunks[0],
    );
    frame.render_widget(
        text_field(
            "Refresh Token TTL (days)",
            &s.refresh_token_ttl_days,
            s.field == 1,
        ),
        chunks[1],
    );
    frame.render_widget(hints(), chunks[3]);
}

fn render_registration(frame: &mut Frame, s: &SettingsState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        toggle_line(
            "Allow Public Registration",
            s.allow_public_registration,
            s.field == 0,
        ),
        chunks[0],
    );
    frame.render_widget(
        toggle_line(
            "Require Email Verified to Login",
            s.require_email_verified,
            s.field == 1,
        ),
        chunks[1],
    );
    frame.render_widget(hints(), chunks[3]);
}
