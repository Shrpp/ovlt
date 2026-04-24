use arboard;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use crate::{
    api::ApiError,
    app::{App, AppMode, Focus, Modal, QuickStartState, Tab},
    components::{modal, statusbar, table::StatefulTable},
    events::{poll, AppEvent},
    ui,
};

pub async fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Pre-fetch tenant list so login can show a dropdown.
    if let Ok(opts) = app.client.list_tenant_slugs().await {
        if !opts.is_empty() {
            // Sync the default slug to the first option.
            if let AppMode::Login { slug, slug_idx, .. } = &mut app.mode {
                *slug = opts[0].0.clone();
                *slug_idx = 0;
            }
            app.tenant_options = opts;
        }
    }

    let mut client_table = StatefulTable::new();
    let mut user_table = StatefulTable::new();
    let mut session_list_state = ratatui::widgets::ListState::default();
    let mut role_list_state = ratatui::widgets::ListState::default();
    let mut permission_list_state = ratatui::widgets::ListState::default();

    loop {
        terminal.draw(|frame| {
            if matches!(&app.mode, AppMode::Login { .. }) {
                ui::login::render(frame, &app);
                return;
            }

            let (sidebar, content, header, statusbar_area) = ui::layout::split_areas(frame);
            let (tabs_area, content_body) = ui::layout::split_content(content);

            ui::layout::render_header(frame, &app, header);
            ui::layout::render_tenant_sidebar(frame, &app, sidebar);
            ui::layout::render_tabs(frame, &app, tabs_area);

            match app.tab {
                Tab::Clients => ui::clients::render(frame, &app, content_body, &mut client_table),
                Tab::Users => ui::users::render(frame, &app, content_body, &mut user_table),
                Tab::Roles => ui::roles::render(frame, &app, content_body, &mut role_list_state),
                Tab::Permissions => ui::permissions::render(frame, &app, content_body, &mut permission_list_state),
                Tab::Sessions => ui::sessions::render(frame, &app, content_body, &mut session_list_state),
            }

            let hints: Vec<(&str, &str)> = match app.focus {
                Focus::Sidebar => vec![
                    ("↑↓", "Tenant"),
                    ("→/Enter", "Open"),
                    ("n", "New tenant"),
                    ("r", "Refresh"),
                    ("q", "Quit"),
                ],
                Focus::Content => match app.tab {
                    Tab::Clients => vec![
                        ("Esc", "Back"),
                        ("←→", "Switch tab"),
                        ("↑↓", "Navigate"),
                        ("n", "New"),
                        ("e", "Edit"),
                        ("d", "Delete"),
                        ("q", "Quit"),
                    ],
                    Tab::Users => vec![
                        ("Esc", "Back"),
                        ("←→", "Switch tab"),
                        ("↑↓", "Navigate"),
                        ("n", "New"),
                        ("e", "Edit"),
                        ("d", "Deactivate"),
                        ("q", "Quit"),
                    ],
                    Tab::Roles => vec![
                        ("Esc", "Back"),
                        ("←→", "Switch tab"),
                        ("↑↓", "Navigate"),
                        ("n", "New"),
                        ("e", "Edit"),
                        ("d", "Delete"),
                        ("q", "Quit"),
                    ],
                    Tab::Permissions => vec![
                        ("Esc", "Back"),
                        ("←→", "Switch tab"),
                        ("↑↓", "Navigate"),
                        ("n", "New"),
                        ("e", "Edit"),
                        ("d", "Delete"),
                        ("q", "Quit"),
                    ],
                    Tab::Sessions => vec![
                        ("Esc", "Back"),
                        ("←→", "Switch tab"),
                        ("↑↓", "Navigate"),
                        ("d", "Revoke"),
                        ("q", "Quit"),
                    ],
                },
            };

            statusbar::render(frame, statusbar_area, &hints, app.status_msg.as_deref());

            match &app.modal.clone() {
                Modal::None => {}
                Modal::ConfirmDelete { id: _, label } => {
                    modal::render_confirm(frame, label);
                }
                Modal::ShowSecret { client_id, secret } => {
                    modal::render_secret(frame, client_id, secret);
                }
                Modal::Error(msg) => {
                    modal::render_error(frame, msg);
                }
                Modal::CreateTenant { name, slug, field } => {
                    modal::render_form(frame, "New Tenant", &[("Name", name), ("Slug", slug)], *field);
                }
                Modal::CreateClient { name, redirect_uri, scopes, client_type, field } => {
                    modal::render_create_client(frame, name, redirect_uri, scopes, *client_type, *field);
                }
                Modal::CreateUser { email, password, field } => {
                    modal::render_form(
                        frame,
                        "New User",
                        &[("Email", email), ("Password", password)],
                        *field,
                    );
                }
                Modal::QuickStart(_) => {
                    ui::quickstart::render(frame, &app);
                }
                Modal::EditClient { name, redirect_uris, scopes, field, .. } => {
                    modal::render_form(
                        frame,
                        "Edit Client",
                        &[("Name", name), ("Redirect URIs", redirect_uris), ("Scopes", scopes)],
                        *field,
                    );
                }
                Modal::EditUser { email, password, is_active, all_roles, permissions, field, role_selected, .. } => {
                    modal::render_edit_user(frame, email, password, *is_active, all_roles, permissions, *field, *role_selected);
                }
                Modal::CreateRole { name, description, field } => {
                    modal::render_form(
                        frame,
                        "New Role",
                        &[("Name", name), ("Description", description)],
                        *field,
                    );
                }
                Modal::EditRole { name, description, all_permissions, field, perm_selected, .. } => {
                    modal::render_edit_role(frame, name, description, all_permissions, *field, *perm_selected);
                }
                Modal::CreatePermission { name, description, field } => {
                    modal::render_form(
                        frame,
                        "New Permission",
                        &[("Name", name), ("Description", description)],
                        *field,
                    );
                }
                Modal::EditPermission { name, description, field, .. } => {
                    modal::render_form(
                        frame,
                        "Edit Permission",
                        &[("Name", name), ("Description", description)],
                        *field,
                    );
                }
                Modal::UserRoles { email, all_roles, selected, .. } => {
                    modal::render_user_roles(frame, email, all_roles, *selected);
                }
            }
        })?;

        match poll()? {
            Some(AppEvent::Key(key)) => {
                if matches!(&app.mode, AppMode::Login { .. }) {
                    handle_login_key(&mut app, key.code).await;
                } else {
                    handle_key(&mut app, key.code, key.modifiers).await;
                }
                if app.should_quit {
                    break;
                }
            }
            Some(AppEvent::Tick) => {}
            None => break,
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

async fn handle_login_key(app: &mut App, code: KeyCode) {
    let AppMode::Login {
        email,
        password,
        slug,
        slug_idx,
        field,
        ..
    } = app.mode.clone()
    else {
        return;
    };

    let opts = app.tenant_options.clone();
    let has_opts = !opts.is_empty();

    let mk_mode = |email: String, password: String, slug: String, slug_idx: usize, field: usize, error: Option<String>| {
        AppMode::Login { email, password, slug, slug_idx, field, error }
    };

    match code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Tab => {
            app.mode = mk_mode(email, password, slug, slug_idx, (field + 1) % 3, None);
        }
        // Tenant picker: Up/Down cycle through options
        KeyCode::Up if field == 2 && has_opts => {
            let idx = if slug_idx == 0 { opts.len() - 1 } else { slug_idx - 1 };
            app.mode = mk_mode(email, password, opts[idx].0.clone(), idx, field, None);
        }
        KeyCode::Down if field == 2 && has_opts => {
            let idx = (slug_idx + 1) % opts.len();
            app.mode = mk_mode(email, password, opts[idx].0.clone(), idx, field, None);
        }
        // Text input for email/password (and slug when no options available)
        KeyCode::Backspace => {
            let (mut e, mut p, mut s) = (email, password, slug);
            match field {
                0 => { e.pop(); }
                1 => { p.pop(); }
                _ if !has_opts => { s.pop(); }
                _ => {}
            }
            app.mode = mk_mode(e, p, s, slug_idx, field, None);
        }
        KeyCode::Char(c) => {
            let (mut e, mut p, mut s) = (email, password, slug);
            match field {
                0 => e.push(c),
                1 => p.push(c),
                _ if !has_opts => s.push(c),
                _ => {}
            }
            app.mode = mk_mode(e, p, s, slug_idx, field, None);
        }
        KeyCode::Enter => {
            if email.is_empty() || password.is_empty() {
                return;
            }
            let client = app.client.clone();
            match client.login(&email, &password, &slug).await {
                Ok(token) => {
                    app.client.set_token(token);
                    app.mode = AppMode::Admin;
                    load_tenants(app).await;
                    check_health(app).await;
                    // Auto-open wizard if only the master tenant exists
                    let only_master = app.tenants.len() <= 1
                        && app.tenants.iter().all(|t| t.slug == "master");
                    if only_master && slug == "master" {
                        app.modal = Modal::QuickStart(QuickStartState::default());
                    }
                }
                Err(ApiError::Api { status: 401, .. }) => {
                    app.mode = mk_mode(email, password, slug, slug_idx, field,
                        Some("Invalid credentials".to_string()));
                }
                Err(e) => {
                    app.mode = mk_mode(email, password, slug, slug_idx, field,
                        Some(format!("Error: {e}")));
                }
            }
        }
        _ => {}
    }
}

async fn handle_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match app.modal.clone() {
        Modal::ConfirmDelete { id, label: _ } => {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => perform_delete(app, id).await,
                _ => app.modal = Modal::None,
            }
            return;
        }
        Modal::ShowSecret { .. } | Modal::Error(_) => {
            app.modal = Modal::None;
            return;
        }
        Modal::CreateTenant { mut name, mut slug, mut field } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 2;
                    app.modal = Modal::CreateTenant { name, slug, field };
                }
                KeyCode::Enter => {
                    if !name.is_empty() && !slug.is_empty() {
                        let n = name.clone();
                        let s = slug.clone();
                        app.modal = Modal::None;
                        perform_create_tenant(app, n, s).await;
                    }
                }
                KeyCode::Backspace => {
                    if field == 0 {
                        name.pop();
                    } else {
                        slug.pop();
                    }
                    app.modal = Modal::CreateTenant { name, slug, field };
                }
                KeyCode::Char(c) => {
                    if field == 0 {
                        name.push(c);
                    } else {
                        slug.push(c);
                    }
                    app.modal = Modal::CreateTenant { name, slug, field };
                }
                _ => {}
            }
            return;
        }
        Modal::CreateClient { mut name, mut redirect_uri, mut scopes, mut client_type, mut field } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 4;
                    app.modal = Modal::CreateClient { name, redirect_uri, scopes, client_type, field };
                }
                // On the type field (3), Space/Left/Right cycles the client_type.
                KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right if field == 3 => {
                    client_type = (client_type + 1) % 3;
                    app.modal = Modal::CreateClient { name, redirect_uri, scopes, client_type, field };
                }
                KeyCode::Enter => {
                    // Machine clients (type 2) don't require redirect_uri.
                    let requires_uri = client_type != 2;
                    if !name.is_empty() && (!requires_uri || !redirect_uri.is_empty()) {
                        let n = name.clone();
                        let u = redirect_uri.clone();
                        let sc = scopes.clone();
                        let ct = client_type;
                        app.modal = Modal::None;
                        perform_create_client(app, n, u, sc, ct).await;
                    }
                }
                KeyCode::Backspace => {
                    match field {
                        0 => { name.pop(); }
                        1 => { redirect_uri.pop(); }
                        2 => { scopes.pop(); }
                        _ => {}  // field 3 is the type selector, not editable
                    }
                    app.modal = Modal::CreateClient { name, redirect_uri, scopes, client_type, field };
                }
                KeyCode::Char(c) => {
                    match field {
                        0 => name.push(c),
                        1 => redirect_uri.push(c),
                        2 => scopes.push(c),
                        _ => {}  // field 3 is the type selector, not editable
                    }
                    app.modal = Modal::CreateClient { name, redirect_uri, scopes, client_type, field };
                }
                _ => {}
            }
            return;
        }
        Modal::CreateUser { mut email, mut password, mut field } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 2;
                    app.modal = Modal::CreateUser { email, password, field };
                }
                KeyCode::Enter => {
                    if !email.is_empty() && !password.is_empty() {
                        let e = email.clone();
                        let p = password.clone();
                        app.modal = Modal::None;
                        perform_create_user(app, e, p).await;
                    }
                }
                KeyCode::Backspace => {
                    if field == 0 {
                        email.pop();
                    } else {
                        password.pop();
                    }
                    app.modal = Modal::CreateUser { email, password, field };
                }
                KeyCode::Char(c) => {
                    if field == 0 {
                        email.push(c);
                    } else {
                        password.push(c);
                    }
                    app.modal = Modal::CreateUser { email, password, field };
                }
                _ => {}
            }
            return;
        }
        Modal::CreateRole { mut name, mut description, mut field } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 2;
                    app.modal = Modal::CreateRole { name, description, field };
                }
                KeyCode::Enter => {
                    if !name.is_empty() {
                        let n = name.clone();
                        let d = description.clone();
                        app.modal = Modal::None;
                        perform_create_role(app, n, d).await;
                    }
                }
                KeyCode::Backspace => {
                    if field == 0 { name.pop(); } else { description.pop(); }
                    app.modal = Modal::CreateRole { name, description, field };
                }
                KeyCode::Char(c) => {
                    if field == 0 { name.push(c); } else { description.push(c); }
                    app.modal = Modal::CreateRole { name, description, field };
                }
                _ => {}
            }
            return;
        }
        Modal::UserRoles { user_id, email, mut all_roles, mut selected } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Up => {
                    if selected > 0 { selected -= 1; }
                    app.modal = Modal::UserRoles { user_id, email, all_roles, selected };
                }
                KeyCode::Down => {
                    if selected + 1 < all_roles.len() { selected += 1; }
                    app.modal = Modal::UserRoles { user_id, email, all_roles, selected };
                }
                KeyCode::Char(' ') => {
                    if let Some(entry) = all_roles.get_mut(selected) {
                        entry.2 = !entry.2;
                    }
                    app.modal = Modal::UserRoles { user_id, email, all_roles, selected };
                }
                KeyCode::Enter => {
                    let uid = user_id.clone();
                    let entries = all_roles.clone();
                    app.modal = Modal::None;
                    perform_save_user_roles(app, uid, entries).await;
                }
                _ => {}
            }
            return;
        }
        Modal::EditClient { mut name, mut redirect_uris, mut scopes, mut field, id } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 3;
                    app.modal = Modal::EditClient { id, name, redirect_uris, scopes, field };
                }
                KeyCode::Enter => {
                    if !name.is_empty() {
                        let id2 = id.clone();
                        let n = name.clone();
                        let ru = redirect_uris.clone();
                        let sc = scopes.clone();
                        app.modal = Modal::None;
                        perform_edit_client(app, id2, n, ru, sc).await;
                    }
                }
                KeyCode::Backspace => {
                    match field {
                        0 => { name.pop(); }
                        1 => { redirect_uris.pop(); }
                        _ => { scopes.pop(); }
                    }
                    app.modal = Modal::EditClient { id, name, redirect_uris, scopes, field };
                }
                KeyCode::Char(c) => {
                    match field {
                        0 => name.push(c),
                        1 => redirect_uris.push(c),
                        _ => scopes.push(c),
                    }
                    app.modal = Modal::EditClient { id, name, redirect_uris, scopes, field };
                }
                _ => {}
            }
            return;
        }
        Modal::EditUser { id, mut email, mut password, mut is_active, mut all_roles, permissions, mut field, mut role_selected } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    // Cycle: email(0) → password(1) → is_active(2) → roles(3) → email
                    field = (field + 1) % 4;
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Up if field == 3 => {
                    if role_selected > 0 { role_selected -= 1; }
                    // Recalculate permissions from currently checked roles
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Down if field == 3 => {
                    if role_selected + 1 < all_roles.len() { role_selected += 1; }
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Char(' ') if field == 2 => {
                    is_active = !is_active;
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Char(' ') if field == 3 => {
                    if let Some(entry) = all_roles.get_mut(role_selected) {
                        entry.2 = !entry.2;
                    }
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Enter => {
                    let id2 = id.clone();
                    let e = email.clone();
                    let pw = if password.is_empty() { None } else { Some(password.clone()) };
                    let roles = all_roles.clone();
                    app.modal = Modal::None;
                    perform_edit_user(app, id2, e, pw, is_active, roles).await;
                }
                KeyCode::Backspace if field == 0 => {
                    email.pop();
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Backspace if field == 1 => {
                    password.pop();
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Char(c) if field == 0 => {
                    email.push(c);
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                KeyCode::Char(c) if field == 1 => {
                    password.push(c);
                    app.modal = Modal::EditUser { id, email, password, is_active, all_roles, permissions, field, role_selected };
                }
                _ => {}
            }
            return;
        }
        Modal::EditRole { id, mut name, mut description, mut all_permissions, mut field, mut perm_selected } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 3;
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                KeyCode::Up if field == 2 => {
                    if perm_selected > 0 { perm_selected -= 1; }
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                KeyCode::Down if field == 2 => {
                    if perm_selected + 1 < all_permissions.len() { perm_selected += 1; }
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                KeyCode::Char(' ') if field == 2 => {
                    if let Some(entry) = all_permissions.get_mut(perm_selected) {
                        entry.2 = !entry.2;
                    }
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                KeyCode::Enter if field != 2 => {
                    if !name.is_empty() {
                        let id2 = id.clone();
                        let n = name.clone();
                        let d = description.clone();
                        let perms = all_permissions.clone();
                        app.modal = Modal::None;
                        perform_edit_role(app, id2, n, d, perms).await;
                    }
                }
                KeyCode::Enter if field == 2 => {
                    let id2 = id.clone();
                    let n = name.clone();
                    let d = description.clone();
                    let perms = all_permissions.clone();
                    app.modal = Modal::None;
                    perform_edit_role(app, id2, n, d, perms).await;
                }
                KeyCode::Backspace if field == 0 => {
                    name.pop();
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                KeyCode::Backspace if field == 1 => {
                    description.pop();
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                KeyCode::Char(c) if field == 0 => {
                    name.push(c);
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                KeyCode::Char(c) if field == 1 => {
                    description.push(c);
                    app.modal = Modal::EditRole { id, name, description, all_permissions, field, perm_selected };
                }
                _ => {}
            }
            return;
        }
        Modal::CreatePermission { mut name, mut description, mut field } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 2;
                    app.modal = Modal::CreatePermission { name, description, field };
                }
                KeyCode::Enter => {
                    if !name.is_empty() {
                        let n = name.clone();
                        let d = description.clone();
                        app.modal = Modal::None;
                        perform_create_permission(app, n, d).await;
                    }
                }
                KeyCode::Backspace => {
                    if field == 0 { name.pop(); } else { description.pop(); }
                    app.modal = Modal::CreatePermission { name, description, field };
                }
                KeyCode::Char(c) => {
                    if field == 0 { name.push(c); } else { description.push(c); }
                    app.modal = Modal::CreatePermission { name, description, field };
                }
                _ => {}
            }
            return;
        }
        Modal::EditPermission { id, mut name, mut description, mut field } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Tab => {
                    field = (field + 1) % 2;
                    app.modal = Modal::EditPermission { id, name, description, field };
                }
                KeyCode::Enter => {
                    if !name.is_empty() {
                        let id2 = id.clone();
                        let n = name.clone();
                        let d = description.clone();
                        app.modal = Modal::None;
                        perform_edit_permission(app, id2, n, d).await;
                    }
                }
                KeyCode::Backspace => {
                    if field == 0 { name.pop(); } else { description.pop(); }
                    app.modal = Modal::EditPermission { id, name, description, field };
                }
                KeyCode::Char(c) => {
                    if field == 0 { name.push(c); } else { description.push(c); }
                    app.modal = Modal::EditPermission { id, name, description, field };
                }
                _ => {}
            }
            return;
        }
        Modal::QuickStart(_) => {
            handle_quickstart_key(app, code).await;
            return;
        }
        Modal::None => {}
    }

    if code == KeyCode::Char('q') {
        app.should_quit = true;
        return;
    }

    if code == KeyCode::Char('?') {
        app.modal = Modal::QuickStart(QuickStartState::default());
        return;
    }

    match app.focus {
        Focus::Sidebar => handle_sidebar_key(app, code).await,
        Focus::Content => handle_content_key(app, code).await,
    }
}

async fn handle_sidebar_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up => {
            if app.tenant_selected > 0 {
                app.tenant_selected -= 1;
            }
        }
        KeyCode::Down => {
            if app.tenant_selected + 1 < app.tenants.len() {
                app.tenant_selected += 1;
            }
        }
        KeyCode::Enter | KeyCode::Right => {
            if let Some(t) = app.selected_tenant() {
                let tid = t.id.clone();
                let switching_tenant = app.active_tenant_id.as_deref() != Some(&tid);
                if switching_tenant {
                    app.clients = vec![];
                    app.users = vec![];
                    app.roles = vec![];
                    app.permissions = vec![];
                    app.sessions = vec![];
                    app.client_selected = 0;
                    app.user_selected = 0;
                    app.role_selected = 0;
                    app.permission_selected = 0;
                    app.session_selected = 0;
                }
                app.active_tenant_id = Some(tid.clone());
                app.focus = Focus::Content;
                load_all(app).await;
            }
        }
        KeyCode::Tab => {
            if app.active_tenant_id.is_some() {
                app.focus = Focus::Content;
            }
        }
        KeyCode::Char('n') => {
            app.modal = Modal::CreateTenant {
                name: String::new(),
                slug: String::new(),
                field: 0,
            };
        }
        KeyCode::Char('r') => load_tenants(app).await,
        _ => {}
    }
}

async fn handle_content_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.focus = Focus::Sidebar;
        }
        KeyCode::Left => {
            app.tab = match app.tab {
                Tab::Clients => Tab::Sessions,
                Tab::Users => Tab::Clients,
                Tab::Roles => Tab::Users,
                Tab::Permissions => Tab::Roles,
                Tab::Sessions => Tab::Permissions,
            };
            load_current_tab(app).await;
        }
        KeyCode::Right | KeyCode::Tab => {
            app.tab = match app.tab {
                Tab::Clients => Tab::Users,
                Tab::Users => Tab::Roles,
                Tab::Roles => Tab::Permissions,
                Tab::Permissions => Tab::Sessions,
                Tab::Sessions => Tab::Clients,
            };
            load_current_tab(app).await;
        }
        KeyCode::Up => match app.tab {
            Tab::Clients => { if app.client_selected > 0 { app.client_selected -= 1; } }
            Tab::Users => { if app.user_selected > 0 { app.user_selected -= 1; } }
            Tab::Roles => { if app.role_selected > 0 { app.role_selected -= 1; } }
            Tab::Permissions => { if app.permission_selected > 0 { app.permission_selected -= 1; } }
            Tab::Sessions => { if app.session_selected > 0 { app.session_selected -= 1; } }
        },
        KeyCode::Down => match app.tab {
            Tab::Clients => { if app.client_selected + 1 < app.clients.len() { app.client_selected += 1; } }
            Tab::Users => { if app.user_selected + 1 < app.users.len() { app.user_selected += 1; } }
            Tab::Roles => { if app.role_selected + 1 < app.roles.len() { app.role_selected += 1; } }
            Tab::Permissions => { if app.permission_selected + 1 < app.permissions.len() { app.permission_selected += 1; } }
            Tab::Sessions => { if app.session_selected + 1 < app.sessions.len() { app.session_selected += 1; } }
        },
        KeyCode::Char('n') => match app.tab {
            Tab::Clients => {
                if app.active_tenant_id.is_some() {
                    app.modal = Modal::CreateClient {
                        name: String::new(),
                        redirect_uri: String::new(),
                        scopes: String::from("openid email profile"),
                        client_type: 0,
                        field: 0,
                    };
                }
            }
            Tab::Users => {
                if app.active_tenant_id.is_some() {
                    app.modal = Modal::CreateUser {
                        email: String::new(),
                        password: String::new(),
                        field: 0,
                    };
                }
            }
            Tab::Roles => {
                if app.active_tenant_id.is_some() {
                    app.modal = Modal::CreateRole {
                        name: String::new(),
                        description: String::new(),
                        field: 0,
                    };
                }
            }
            Tab::Permissions => {
                if app.active_tenant_id.is_some() {
                    app.modal = Modal::CreatePermission {
                        name: String::new(),
                        description: String::new(),
                        field: 0,
                    };
                }
            }
            Tab::Sessions => {}
        },
        KeyCode::Char('e') => match app.tab {
            Tab::Clients => {
                if let Some(c) = app.selected_client() {
                    app.modal = Modal::EditClient {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        redirect_uris: c.redirect_uris.join(", "),
                        scopes: c.scopes.join(" "),
                        field: 0,
                    };
                }
            }
            Tab::Users => {
                if let (Some(u), Some(tid)) = (app.selected_user().cloned(), app.active_tenant_id.clone()) {
                    open_edit_user(app, u.id, u.email, u.is_active, tid).await;
                }
            }
            Tab::Roles => {
                if let (Some(r), Some(tid)) = (app.selected_role().cloned(), app.active_tenant_id.clone()) {
                    open_edit_role(app, r.id, r.name, r.description, tid).await;
                }
            }
            Tab::Permissions => {
                if let Some(p) = app.selected_permission() {
                    app.modal = Modal::EditPermission {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        description: p.description.clone(),
                        field: 0,
                    };
                }
            }
            Tab::Sessions => {}
        },
        KeyCode::Char('d') => match app.tab {
            Tab::Clients => {
                if let Some(c) = app.selected_client() {
                    app.modal = Modal::ConfirmDelete { id: c.id.clone(), label: c.name.clone() };
                }
            }
            Tab::Users => {
                if let Some(u) = app.selected_user() {
                    app.modal = Modal::ConfirmDelete { id: u.id.clone(), label: u.email.clone() };
                }
            }
            Tab::Roles => {
                if let Some(r) = app.selected_role() {
                    app.modal = Modal::ConfirmDelete { id: r.id.clone(), label: r.name.clone() };
                }
            }
            Tab::Permissions => {
                if let Some(p) = app.selected_permission() {
                    app.modal = Modal::ConfirmDelete { id: p.id.clone(), label: p.name.clone() };
                }
            }
            Tab::Sessions => {
                if let Some(s) = app.selected_session() {
                    app.modal = Modal::ConfirmDelete { id: s.id.clone(), label: s.email.clone() };
                }
            }
        },
        _ => {}
    }
}

async fn load_all(app: &mut App) {
    let Some(tid) = app.active_tenant_id.clone() else { return };
    let client = app.client.clone();

    let (clients_r, users_r, roles_r, perms_r, sessions_r) = tokio::join!(
        client.list_clients(&tid),
        client.list_users(&tid),
        client.list_roles(&tid),
        client.list_permissions(&tid),
        client.list_sessions(&tid),
    );

    match clients_r {
        Ok(list) => {
            app.client_selected = app.client_selected.min(list.len().saturating_sub(1));
            app.clients = list;
        }
        Err(e) => app.set_status(format!("Clients error: {e}")),
    }
    match users_r {
        Ok(list) => {
            app.user_selected = app.user_selected.min(list.len().saturating_sub(1));
            app.users = list;
        }
        Err(e) => app.set_status(format!("Users error: {e}")),
    }
    match roles_r {
        Ok(list) => {
            app.role_selected = app.role_selected.min(list.len().saturating_sub(1));
            app.roles = list;
        }
        Err(e) => app.set_status(format!("Roles error: {e}")),
    }
    match perms_r {
        Ok(list) => {
            app.permission_selected = app.permission_selected.min(list.len().saturating_sub(1));
            app.permissions = list;
        }
        Err(e) => app.set_status(format!("Permissions error: {e}")),
    }
    match sessions_r {
        Ok(list) => {
            app.session_selected = app.session_selected.min(list.len().saturating_sub(1));
            app.sessions = list;
        }
        Err(e) => app.set_status(format!("Sessions error: {e}")),
    }
}

async fn load_current_tab(app: &mut App) {
    let Some(tid) = app.active_tenant_id.clone() else { return };
    match app.tab {
        Tab::Clients => load_clients(app, tid).await,
        Tab::Users => load_users(app, tid).await,
        Tab::Roles => load_roles(app, tid).await,
        Tab::Permissions => load_permissions(app, tid).await,
        Tab::Sessions => load_sessions(app, tid).await,
    }
}

async fn load_tenants(app: &mut App) {
    app.tenants_loading = true;
    match app.client.list_tenants().await {
        Ok(list) => {
            app.tenants = list;
            app.tenant_selected = app.tenant_selected.min(app.tenants.len().saturating_sub(1));
            app.clear_status();
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.tenants_loading = false;
}

async fn load_clients(app: &mut App, tenant_id: String) {
    app.clients = vec![];
    app.client_selected = 0;
    app.clients_loading = true;
    match app.client.list_clients(&tenant_id).await {
        Ok(list) => {
            app.clients = list;
            app.clear_status();
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.clients_loading = false;
}

async fn load_users(app: &mut App, tenant_id: String) {
    app.users = vec![];
    app.user_selected = 0;
    app.users_loading = true;
    match app.client.list_users(&tenant_id).await {
        Ok(list) => {
            app.users = list;
            app.clear_status();
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.users_loading = false;
}

async fn load_roles(app: &mut App, tenant_id: String) {
    app.roles = vec![];
    app.role_selected = 0;
    app.roles_loading = true;
    match app.client.list_roles(&tenant_id).await {
        Ok(list) => {
            app.roles = list;
            app.clear_status();
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.roles_loading = false;
}

async fn load_sessions(app: &mut App, tenant_id: String) {
    app.sessions = vec![];
    app.session_selected = 0;
    app.sessions_loading = true;
    match app.client.list_sessions(&tenant_id).await {
        Ok(list) => {
            app.sessions = list;
            app.clear_status();
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.sessions_loading = false;
}

async fn check_health(app: &mut App) {
    match app.client.health().await {
        Ok(v) => {
            app.health_status = Some(v["status"].as_str().unwrap_or("ok").to_owned());
            app.health_error = None;
        }
        Err(e) => {
            app.health_status = None;
            app.health_error = Some(e.to_string());
        }
    }
}

async fn perform_create_tenant(app: &mut App, name: String, slug: String) {
    match app.client.create_tenant(&name, &slug).await {
        Ok(_) => {
            app.set_status(format!("Tenant '{name}' created"));
            load_tenants(app).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}

async fn perform_create_client(
    app: &mut App,
    name: String,
    redirect_uri: String,
    scopes_str: String,
    client_type: u8,
) {
    let Some(tid) = app.active_tenant_id.clone() else {
        return;
    };
    let scopes: Vec<String> = scopes_str
        .split_whitespace()
        .map(|s| s.to_owned())
        .collect();

    // Derive is_confidential and grant_types from client_type:
    //   0 = Confidential/Web  → is_confidential=true,  grant_types=["authorization_code"]
    //   1 = Public/SPA        → is_confidential=false, grant_types=["authorization_code"]
    //   2 = Machine/M2M       → is_confidential=true,  grant_types=["client_credentials"]
    let (is_confidential, grant_types) = match client_type {
        1 => (false, vec!["authorization_code".to_owned()]),
        2 => (true, vec!["client_credentials".to_owned()]),
        _ => (true, vec!["authorization_code".to_owned()]),
    };

    // Machine clients have no redirect URI.
    let redirect_uris = if client_type == 2 {
        vec![]
    } else {
        vec![redirect_uri]
    };

    match app
        .client
        .create_client(&tid, &name, redirect_uris, scopes, is_confidential, grant_types)
        .await
    {
        Ok(c) => {
            if let Some(secret) = c.client_secret {
                app.modal = Modal::ShowSecret {
                    client_id: c.client_id,
                    secret,
                };
            } else {
                app.set_status(format!("Client '{name}' created"));
            }
            load_all(app).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}

async fn perform_create_user(app: &mut App, email: String, password: String) {
    let Some(tid) = app.active_tenant_id.clone() else {
        return;
    };
    match app.client.create_user(&tid, &email, &password).await {
        Ok(_) => {
            app.set_status(format!("User '{email}' created"));
            load_all(app).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}

fn copy_to_clipboard(app: &mut App, text: &str, success_msg: &str) {
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
        Ok(_) => app.set_status(success_msg.to_string()),
        Err(_) => app.set_status("clipboard unavailable".to_string()),
    }
}

async fn handle_quickstart_key(app: &mut App, code: KeyCode) {
    let Modal::QuickStart(mut qs) = app.modal.clone() else {
        return;
    };

    if qs.step == 4 {
        match code {
            KeyCode::Char('c') => {
                qs.show_secret = !qs.show_secret;
                app.modal = Modal::QuickStart(qs);
            }
            KeyCode::Char('i') => {
                if let Some(cid) = &qs.created_client_id.clone() {
                    copy_to_clipboard(app, cid, "client_id copied");
                }
                app.modal = Modal::QuickStart(qs);
            }
            KeyCode::Char('s') => {
                if let Some(secret) = &qs.created_secret.clone() {
                    copy_to_clipboard(app, secret, "secret copied");
                }
                app.modal = Modal::QuickStart(qs);
            }
            KeyCode::Enter | KeyCode::Esc => {
                app.modal = Modal::None;
                load_tenants(app).await;
            }
            _ => {}
        }
        return;
    }

    let max_fields: usize = match qs.step {
        2 => 4, // name, redirect_uri, scopes, type-toggle
        _ => 2,
    };

    match code {
        KeyCode::Esc => {
            app.modal = Modal::None;
        }
        KeyCode::Tab => {
            qs.field = (qs.field + 1) % max_fields;
            qs.error = None;
            app.modal = Modal::QuickStart(qs);
        }
        // Left/Right on the type toggle field (step 2, field 3) cycle the client type.
        KeyCode::Left if qs.step == 2 && qs.field == 3 => {
            qs.client_type = (qs.client_type + 2) % 3;
            app.modal = Modal::QuickStart(qs);
        }
        KeyCode::Right if qs.step == 2 && qs.field == 3 => {
            qs.client_type = (qs.client_type + 1) % 3;
            app.modal = Modal::QuickStart(qs);
        }
        KeyCode::Backspace => {
            pop_quickstart_field(&mut qs);
            app.modal = Modal::QuickStart(qs);
        }
        KeyCode::Char(c) => {
            push_quickstart_field(&mut qs, c);
            app.modal = Modal::QuickStart(qs);
        }
        KeyCode::Enter => {
            qs.error = None;
            let client = app.client.clone();
            match qs.step {
                1 => {
                    if qs.tenant_name.is_empty() || qs.tenant_slug.is_empty() {
                        qs.error = Some("Name and Slug are required".to_string());
                        app.modal = Modal::QuickStart(qs);
                        return;
                    }
                    match client.create_tenant(&qs.tenant_name.clone(), &qs.tenant_slug.clone()).await {
                        Ok(t) => {
                            qs.created_tenant_id = Some(t.id);
                            qs.created_tenant_name = Some(qs.tenant_name.clone());
                            qs.step = 2;
                            qs.field = 0;
                            app.modal = Modal::QuickStart(qs);
                        }
                        Err(e) => {
                            qs.error = Some(format!("{e}"));
                            app.modal = Modal::QuickStart(qs);
                        }
                    }
                }
                2 => {
                    let needs_uri = qs.client_type != 2;
                    if qs.client_name.is_empty() || (needs_uri && qs.redirect_uri.is_empty()) {
                        qs.error = Some(if needs_uri {
                            "Name and Redirect URI are required".to_string()
                        } else {
                            "Name is required".to_string()
                        });
                        app.modal = Modal::QuickStart(qs);
                        return;
                    }
                    let Some(tid) = qs.created_tenant_id.clone() else { return };
                    let scopes: Vec<String> = qs.scopes.split_whitespace().map(|s| s.to_owned()).collect();
                    let (is_confidential, grant_types) = match qs.client_type {
                        1 => (false, vec!["authorization_code".to_owned()]),
                        2 => (true,  vec!["client_credentials".to_owned()]),
                        _ => (true,  vec!["authorization_code".to_owned()]),
                    };
                    let redirect_uris = if qs.client_type == 2 {
                        vec![]
                    } else {
                        vec![qs.redirect_uri.clone()]
                    };
                    match client.create_client(
                        &tid,
                        &qs.client_name.clone(),
                        redirect_uris,
                        scopes,
                        is_confidential,
                        grant_types,
                    ).await {
                        Ok(c) => {
                            qs.created_client_id = Some(c.client_id);
                            qs.created_secret = c.client_secret;
                            qs.step = 3;
                            qs.field = 0;
                            app.modal = Modal::QuickStart(qs);
                        }
                        Err(e) => {
                            qs.error = Some(format!("{e}"));
                            app.modal = Modal::QuickStart(qs);
                        }
                    }
                }
                3 => {
                    if qs.user_email.is_empty() || qs.user_password.is_empty() {
                        qs.error = Some("Email and Password are required".to_string());
                        app.modal = Modal::QuickStart(qs);
                        return;
                    }
                    let Some(tid) = qs.created_tenant_id.clone() else { return };
                    match client.create_user(&tid, &qs.user_email.clone(), &qs.user_password.clone()).await {
                        Ok(_) => {
                            qs.step = 4;
                            app.modal = Modal::QuickStart(qs);
                        }
                        Err(e) => {
                            qs.error = Some(format!("{e}"));
                            app.modal = Modal::QuickStart(qs);
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn pop_quickstart_field(qs: &mut QuickStartState) {
    match (qs.step, qs.field) {
        (1, 0) => { qs.tenant_name.pop(); }
        (1, 1) => { qs.tenant_slug.pop(); }
        (2, 0) => { qs.client_name.pop(); }
        (2, 1) => { qs.redirect_uri.pop(); }
        (2, 2) => { qs.scopes.pop(); }
        (3, 0) => { qs.user_email.pop(); }
        (3, 1) => { qs.user_password.pop(); }
        _ => {}
    }
}

fn push_quickstart_field(qs: &mut QuickStartState, c: char) {
    match (qs.step, qs.field) {
        (1, 0) => qs.tenant_name.push(c),
        (1, 1) => qs.tenant_slug.push(c),
        (2, 0) => qs.client_name.push(c),
        (2, 1) => qs.redirect_uri.push(c),
        (2, 2) => qs.scopes.push(c),
        (3, 0) => qs.user_email.push(c),
        (3, 1) => qs.user_password.push(c),
        _ => {}
    }
}

async fn perform_create_role(app: &mut App, name: String, description: String) {
    let Some(tid) = app.active_tenant_id.clone() else { return };
    match app.client.create_role(&tid, &name, &description).await {
        Ok(role) => {
            app.set_status(format!("Role '{}' created — assign permissions below", role.name));
            load_all(app).await;
            open_edit_role(app, role.id, role.name, role.description, tid).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}

async fn perform_save_user_roles(
    app: &mut App,
    user_id: String,
    entries: Vec<(String, String, bool)>,
) {
    let Some(tid) = app.active_tenant_id.clone() else { return };
    let client = app.client.clone();
    for (role_id, _, assigned) in &entries {
        if *assigned {
            let _ = client.assign_user_role(&tid, &user_id, role_id).await;
        } else {
            let _ = client.revoke_user_role(&tid, &user_id, role_id).await;
        }
    }
    app.set_status("Roles saved");
}

async fn perform_edit_client(
    app: &mut App,
    id: String,
    name: String,
    redirect_uris_str: String,
    scopes_str: String,
) {
    let Some(tid) = app.active_tenant_id.clone() else { return };
    let redirect_uris: Vec<String> = redirect_uris_str
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let scopes: Vec<String> = scopes_str
        .split_whitespace()
        .map(|s| s.to_owned())
        .collect();
    match app.client.update_client(&tid, &id, &name, redirect_uris, scopes).await {
        Ok(_) => {
            app.set_status(format!("Client '{name}' updated"));
            load_all(app).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}

async fn load_permissions(app: &mut App, tenant_id: String) {
    app.permissions = vec![];
    app.permission_selected = 0;
    app.permissions_loading = true;
    match app.client.list_permissions(&tenant_id).await {
        Ok(list) => {
            app.permissions = list;
            app.clear_status();
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.permissions_loading = false;
}

/// Eager-load everything needed to open EditUser modal.
async fn open_edit_user(app: &mut App, user_id: String, email: String, is_active: bool, tid: String) {
    let client = app.client.clone();
    // Always fetch fresh from the API so we get current-tenant data regardless of tab cache.
    let (roles_result, assigned_result) = tokio::join!(
        client.list_roles(&tid),
        client.list_user_roles(&tid, &user_id),
    );
    let all_roles = match roles_result {
        Ok(r) => {
            app.roles = r.clone();
            r
        }
        Err(e) => {
            app.modal = Modal::Error(format!("Failed to load roles: {e}"));
            return;
        }
    };
    let assigned_ids: std::collections::HashSet<String> = match assigned_result {
        Ok(r) => r.iter().map(|r| r.id.clone()).collect(),
        Err(e) => {
            app.modal = Modal::Error(format!("Failed to load user roles: {e}"));
            return;
        }
    };

    let role_entries: Vec<(String, String, bool)> = all_roles
        .iter()
        .map(|r| (r.id.clone(), r.name.clone(), assigned_ids.contains(&r.id)))
        .collect();

    // Derive permissions from currently assigned roles (eager).
    let permissions = derive_user_permissions(&role_entries, &tid, &client).await;

    app.modal = Modal::EditUser {
        id: user_id,
        email,
        password: String::new(),
        is_active,
        all_roles: role_entries,
        permissions,
        field: 0,
        role_selected: 0,
    };
}

/// Derive permission names from currently-assigned roles via fresh API calls.
async fn derive_user_permissions(
    role_entries: &[(String, String, bool)],
    tid: &str,
    client: &crate::api::Client,
) -> Vec<String> {
    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (role_id, _, assigned) in role_entries {
        if *assigned {
            if let Ok(perms) = client.list_role_permissions(tid, role_id).await {
                for p in perms { names.insert(p.name); }
            }
        }
    }
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
}

/// Eager-load everything needed to open EditRole modal (fresh from API, not cached state).
async fn open_edit_role(app: &mut App, role_id: String, name: String, description: String, tid: String) {
    let client = app.client.clone();
    let (all_perms_result, assigned_result) = tokio::join!(
        client.list_permissions(&tid),
        client.list_role_permissions(&tid, &role_id),
    );
    let all_perms = match all_perms_result {
        Ok(p) => {
            app.permissions = p.clone();
            p
        }
        Err(e) => {
            app.modal = Modal::Error(format!("Failed to load permissions: {e}"));
            return;
        }
    };
    let assigned_ids: std::collections::HashSet<String> = match assigned_result {
        Ok(p) => p.iter().map(|p| p.id.clone()).collect(),
        Err(e) => {
            app.modal = Modal::Error(format!("Failed to load role permissions: {e}"));
            return;
        }
    };
    let perm_entries: Vec<(String, String, bool)> = all_perms
        .iter()
        .map(|p| (p.id.clone(), p.name.clone(), assigned_ids.contains(&p.id)))
        .collect();
    app.modal = Modal::EditRole {
        id: role_id,
        name,
        description,
        all_permissions: perm_entries,
        field: 0,
        perm_selected: 0,
    };
}

async fn perform_edit_user(
    app: &mut App,
    id: String,
    email: String,
    password: Option<String>,
    is_active: bool,
    role_entries: Vec<(String, String, bool)>,
) {
    let Some(tid) = app.active_tenant_id.clone() else { return };
    let pw = password.as_deref();
    match app.client.update_user_email(&tid, &id, &email, pw, is_active).await {
        Ok(_) => {}
        Err(e) => { app.modal = Modal::Error(format!("{e}")); return; }
    }
    // Save role assignments
    let client = app.client.clone();
    for (role_id, _, assigned) in &role_entries {
        if *assigned {
            let _ = client.assign_user_role(&tid, &id, role_id).await;
        } else {
            let _ = client.revoke_user_role(&tid, &id, role_id).await;
        }
    }
    app.set_status("User updated");
    load_all(app).await;
}

async fn perform_edit_role(
    app: &mut App,
    id: String,
    name: String,
    description: String,
    perm_entries: Vec<(String, String, bool)>,
) {
    let Some(tid) = app.active_tenant_id.clone() else { return };
    match app.client.update_role(&tid, &id, &name, &description).await {
        Ok(_) => {}
        Err(e) => { app.modal = Modal::Error(format!("{e}")); return; }
    }
    let client = app.client.clone();
    for (perm_id, _, assigned) in &perm_entries {
        if *assigned {
            let _ = client.assign_role_permission(&tid, &id, perm_id).await;
        } else {
            let _ = client.revoke_role_permission(&tid, &id, perm_id).await;
        }
    }
    app.set_status("Role updated");
    load_all(app).await;
}

async fn perform_create_permission(app: &mut App, name: String, description: String) {
    let Some(_tid) = app.active_tenant_id.clone() else { return };
    match app.client.create_permission(&_tid, &name, &description).await {
        Ok(_) => {
            app.set_status(format!("Permission '{name}' created"));
            load_all(app).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}

async fn perform_edit_permission(app: &mut App, id: String, name: String, description: String) {
    let Some(_tid) = app.active_tenant_id.clone() else { return };
    match app.client.update_permission(&_tid, &id, &name, &description).await {
        Ok(_) => {
            app.set_status("Permission updated");
            load_all(app).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}

async fn perform_delete(app: &mut App, id: String) {
    app.modal = Modal::None;
    let Some(tid) = app.active_tenant_id.clone() else {
        return;
    };
    let result = match app.tab {
        Tab::Clients => app.client.deactivate_client(&tid, &id).await
            .map(|_| "Client deactivated"),
        Tab::Users => app.client.deactivate_user(&tid, &id).await
            .map(|_| "User deactivated"),
        Tab::Roles => app.client.delete_role(&tid, &id).await
            .map(|_| "Role deleted"),
        Tab::Permissions => app.client.delete_permission(&tid, &id).await
            .map(|_| "Permission deleted"),
        Tab::Sessions => app.client.delete_session(&tid, &id).await
            .map(|_| "Session revoked"),
    };
    match result {
        Ok(msg) => {
            app.set_status(msg);
            load_all(app).await;
        }
        Err(e) => app.modal = Modal::Error(format!("{e}")),
    }
}
