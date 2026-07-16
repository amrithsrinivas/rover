use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

use rover_core::ConnectionProfile;
use rover_proto::v1::{AppDetailResponse, AppSummary, ServerInfo, ServerMetrics};
use tokio::sync::mpsc::Sender;

use crate::api::client::RoverClient;

/// Shared reference to the gRPC client, used across async tasks.
pub type ClientRef = Arc<Mutex<RoverClient>>;

/// Represents a single connected server.
#[derive(Debug, Clone)]
pub struct ServerState {
    pub profile: ConnectionProfile,
    pub client: Option<ClientRef>,
    pub connected: bool,
    pub connecting: bool,
    pub error: Option<String>,
    pub info: Option<ServerInfo>,
    pub metrics: Option<ServerMetrics>,
    pub apps: Vec<AppSummary>,
}

impl ServerState {
    pub fn from_profile(profile: ConnectionProfile) -> Self {
        Self {
            profile,
            client: None,
            connected: false,
            connecting: false,
            error: None,
            info: None,
            metrics: None,
            apps: Vec::new(),
        }
    }

    /// Number of apps deployed on this server.
    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    /// Connection status label.
    pub fn status_label(&self) -> &'static str {
        if self.connected {
            "connected"
        } else if self.connecting {
            "connecting"
        } else if self.error.is_some() {
            "error"
        } else {
            "offline"
        }
    }
}

/// An app from any server, annotated with its origin so we can route operations.
#[derive(Debug, Clone)]
pub struct AnnotatedApp {
    pub summary: AppSummary,
    pub server_index: usize,
    pub server_name: String,
    pub server_address: String,
}

/// A background deployment tracked by the client UI.
#[derive(Debug, Clone)]
pub struct DeployJob {
    pub id: usize,
    pub name: String,
    pub runtime: String,
    pub source_path: String,
    pub status: String,
    pub logs: Vec<String>,
    pub app_id: Option<String>,
    pub error: Option<String>,
    /// Which server this deploy was sent to.
    pub server_index: usize,
    pub server_name: String,
}

impl DeployJob {
    pub fn is_active(&self) -> bool {
        !matches!(self.status.as_str(), "complete" | "failed")
    }

    pub fn latest_log(&self) -> Option<&str> {
        self.logs.last().map(String::as_str)
    }
}

/// A saved GitHub personal access token.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GithubToken {
    pub id: String,
    pub label: String,
    pub token: String,
}

impl GithubToken {
    pub fn new(label: String, token: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            label,
            token,
        }
    }
}

/// A transient toast notification.
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Error,
}

// ── Screens ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Connect,
    Dashboard,
    /// (app_id, server_index)
    AppDetail(String, usize),
    /// System shell (server_index)
    Terminal(usize),
}

// ── Root application state ───────────────────────────────────────────────────

pub struct RoverApp {
    // ── Servers ───────────────────────────────────────────────────────────
    pub servers: Vec<ServerState>,

    // ── Derived: all apps from all servers (rebuilt on every poll) ────────
    pub all_apps: Vec<AnnotatedApp>,

    // ── Connection / manage forms ─────────────────────────────────────────
    pub show_add_form: bool,
    pub show_manage_servers: bool,
    pub addr_input: String,
    pub token_input: String,
    pub name_input: String,
    pub form_error: Option<String>,

    // ── Current screen ────────────────────────────────────────────────────
    pub screen: Screen,

    // ── App detail (current) ──────────────────────────────────────────────
    pub app_detail: Option<AppDetailResponse>,
    pub app_detail_server: usize,
    pub log_entries: Vec<String>,

    // ── Deploy form ───────────────────────────────────────────────────────
    pub deploy_open: bool,
    pub deploy_target: Option<usize>,
    pub deploy_name: String,
    pub deploy_runtime: String,
    pub deploy_build: String,
    pub deploy_run: String,
    pub deploy_path: String,
    pub deploy_use_github: bool,
    pub deploy_github_url: String,
    pub github_tokens: Vec<GithubToken>,
    pub selected_github_token: Option<String>,
    pub new_token_label: String,
    pub new_token_value: String,
    pub deploy_env_vars: Vec<(String, String)>,
    pub deploy_env_key: String,
    pub deploy_env_value: String,

    // ── Background deploys ────────────────────────────────────────────────
    pub next_deploy_id: usize,
    pub deploy_jobs: Vec<DeployJob>,
    pub expanded_deploy: Option<usize>,

    // ── Modals ────────────────────────────────────────────────────────────
    pub confirm_delete: Option<(String, String, usize)>, // (app_id, name, server_index)
    pub confirm_server_delete: Option<usize>,
    pub editing_server: Option<usize>,
    pub rename_value: String,
    pub edit_address: String,
    pub update_open: bool,
    pub update_build: String,
    pub update_run: String,

    // ── Notifications ─────────────────────────────────────────────────────
    pub toasts: Vec<Toast>,

    // ── Terminal ─────────────────────────────────────────────────────────
    pub terminal_open: bool,
    pub terminal_server: usize,
    pub terminal_output: Vec<String>,
    pub terminal_input: String,
    pub terminal_sender: Option<Sender<rover_proto::v1::ShellInput>>,
    pub terminal_buffer: Arc<StdMutex<Vec<String>>>,
    pub terminal_pending: bool,
    pub terminal_last_cmd: String,
}

impl RoverApp {
    pub fn find_deploy_mut(&mut self, deploy_id: usize) -> Option<&mut DeployJob> {
        self.deploy_jobs.iter_mut().find(|job| job.id == deploy_id)
    }

    pub fn active_deploy_count(&self) -> usize {
        self.deploy_jobs.iter().filter(|j| j.is_active()).count()
    }

    /// Rebuild the unified app list from all connected servers.
    pub fn rebuild_all_apps(&mut self) {
        self.all_apps = self
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.connected)
            .flat_map(|(i, s)| {
                s.apps.iter().map(move |a| AnnotatedApp {
                    summary: a.clone(),
                    server_index: i,
                    server_name: s.profile.name.clone(),
                    server_address: s.profile.address.clone(),
                })
            })
            .collect();
    }

    /// Get the client for a specific server index.
    pub fn client_for(&self, server_index: usize) -> Option<ClientRef> {
        self.servers
            .get(server_index)
            .and_then(|s| s.client.clone())
    }

    /// Get the name for a specific server index.
    pub fn server_name_for(&self, server_index: usize) -> String {
        self.servers
            .get(server_index)
            .map(|s| s.profile.name.clone())
            .unwrap_or_else(|| "unknown".into())
    }

    /// Number of connected servers.
    pub fn connected_count(&self) -> usize {
        self.servers.iter().filter(|s| s.connected).count()
    }

    /// Total number of servers.
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    /// Whether any server is connected.
    pub fn any_connected(&self) -> bool {
        self.servers.iter().any(|s| s.connected)
    }
}
