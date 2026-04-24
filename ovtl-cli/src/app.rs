use crate::api::{Client, OAuthClient, Permission, Role, Session, Tenant, User};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Login {
        email: String,
        password: String,
        slug: String,       // currently selected slug
        slug_idx: usize,    // selected index in App::tenant_options (usize::MAX = custom text)
        field: usize,       // 0=email, 1=password, 2=tenant picker
        error: Option<String>,
    },
    Admin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Clients,
    Users,
    Roles,
    Permissions,
    Sessions,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuickStartState {
    pub step: u8,
    // Step 1 — tenant
    pub tenant_name: String,
    pub tenant_slug: String,
    // Step 2 — client
    pub client_name: String,
    pub redirect_uri: String,
    pub scopes: String,
    pub client_type: u8,  // 0=Confidential, 1=SPA/Mobile, 2=Machine
    // Step 3 — user
    pub user_email: String,
    pub user_password: String,
    // Results stored after each API call
    pub created_tenant_id: Option<String>,
    pub created_tenant_name: Option<String>,
    pub created_client_id: Option<String>,
    pub created_secret: Option<String>,
    pub show_secret: bool,
    // Active input field index within the current step
    pub field: usize,
    pub error: Option<String>,
}

impl Default for QuickStartState {
    fn default() -> Self {
        Self {
            step: 1,
            tenant_name: String::new(),
            tenant_slug: String::new(),
            client_name: String::new(),
            redirect_uri: String::from("http://localhost:8080/callback"),
            scopes: String::from("openid email profile"),
            client_type: 0,
            user_email: String::new(),
            user_password: String::new(),
            created_tenant_id: None,
            created_tenant_name: None,
            created_client_id: None,
            created_secret: None,
            show_secret: false,
            field: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    None,
    CreateTenant { name: String, slug: String, field: usize },
    CreateClient { name: String, redirect_uri: String, scopes: String, client_type: u8, field: usize },
    CreateUser { email: String, password: String, field: usize },
    ConfirmDelete { id: String, label: String },
    ShowSecret { client_id: String, secret: String },
    Error(String),
    QuickStart(QuickStartState),
    EditClient { id: String, name: String, redirect_uris: String, scopes: String, field: usize },
    EditUser {
        id: String,
        email: String,
        password: String,
        is_active: bool,
        all_roles: Vec<(String, String, bool)>,  // (id, name, assigned)
        permissions: Vec<String>,                 // derived from assigned roles, read-only
        field: usize,   // 0=email, 1=password, 2=is_active, 3=roles section
        role_selected: usize,
    },
    CreateRole { name: String, description: String, field: usize },
    EditRole {
        id: String,
        name: String,
        description: String,
        all_permissions: Vec<(String, String, bool)>,  // (id, name, assigned)
        field: usize,      // 0=name, 1=description, 2=permissions section
        perm_selected: usize,
    },
    CreatePermission { name: String, description: String, field: usize },
    EditPermission { id: String, name: String, description: String, field: usize },
    UserRoles { user_id: String, email: String, all_roles: Vec<(String, String, bool)>, selected: usize },
}

pub struct App {
    pub client: Client,
    pub mode: AppMode,
    pub focus: Focus,
    pub tab: Tab,
    pub modal: Modal,

    pub tenant_options: Vec<(String, String)>,  // (slug, name) fetched before login

    pub tenants: Vec<Tenant>,
    pub tenant_selected: usize,
    pub tenants_loading: bool,

    pub clients: Vec<OAuthClient>,
    pub client_selected: usize,
    pub clients_loading: bool,

    pub users: Vec<User>,
    pub user_selected: usize,
    pub users_loading: bool,

    pub sessions: Vec<Session>,
    pub session_selected: usize,
    pub sessions_loading: bool,

    pub roles: Vec<Role>,
    pub role_selected: usize,
    pub roles_loading: bool,

    pub permissions: Vec<Permission>,
    pub permission_selected: usize,
    pub permissions_loading: bool,

    pub active_tenant_id: Option<String>,

    pub health_status: Option<String>,
    pub health_error: Option<String>,

    pub status_msg: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            mode: AppMode::Login {
                email: String::new(),
                password: String::new(),
                slug: String::from("master"),
                slug_idx: 0,
                field: 0,
                error: None,
            },
            focus: Focus::Sidebar,
            tab: Tab::Clients,
            modal: Modal::None,
            tenant_options: vec![],

            tenants: vec![],
            tenant_selected: 0,
            tenants_loading: false,

            clients: vec![],
            client_selected: 0,
            clients_loading: false,

            users: vec![],
            user_selected: 0,
            users_loading: false,

            sessions: vec![],
            session_selected: 0,
            sessions_loading: false,

            roles: vec![],
            role_selected: 0,
            roles_loading: false,

            permissions: vec![],
            permission_selected: 0,
            permissions_loading: false,

            active_tenant_id: None,

            health_status: None,
            health_error: None,

            status_msg: None,
            should_quit: false,
        }
    }

    pub fn selected_tenant(&self) -> Option<&Tenant> {
        self.tenants.get(self.tenant_selected)
    }

    pub fn selected_client(&self) -> Option<&OAuthClient> {
        self.clients.get(self.client_selected)
    }

    pub fn selected_user(&self) -> Option<&User> {
        self.users.get(self.user_selected)
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.session_selected)
    }

    pub fn selected_role(&self) -> Option<&Role> {
        self.roles.get(self.role_selected)
    }

    pub fn selected_permission(&self) -> Option<&Permission> {
        self.permissions.get(self.permission_selected)
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_msg = None;
    }

    pub fn active_tenant_name(&self) -> Option<&str> {
        self.tenants
            .get(self.tenant_selected)
            .map(|t| t.name.as_str())
    }
}
