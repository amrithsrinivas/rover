use rover_proto::v1::AppDetailResponse;

use crate::state::DeviceState;

/// A single background deployment tracked by the client UI.
#[derive(Debug, Clone)]
pub struct DeployState {
    pub id: usize,
    pub name: String,
    pub runtime: String,
    pub source_path: String,
    pub status: String,
    pub logs: Vec<String>,
    pub app_id: Option<String>,
    pub error: Option<String>,
}

impl DeployState {
    /// Whether this deploy is still in progress.
    pub fn is_active(&self) -> bool {
        !matches!(self.status.as_str(), "complete" | "failed")
    }

    /// Last log line emitted for display in compact cards.
    pub fn latest_log(&self) -> Option<&str> {
        self.logs.last().map(String::as_str)
    }
}

/// Visual severity for toast notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Error,
}

/// A transient notification shown over the UI.
#[derive(Debug, Clone)]
pub struct ToastState {
    pub message: String,
    pub kind: ToastKind,
}

/// Root state for the Rover client application.
pub struct RoverApp {
    pub devices: Vec<DeviceState>,
    pub active: usize,
    pub show_add: bool,
    pub addr: String,
    pub token: String,
    pub name: String,
    pub error: Option<String>,
    pub selected_app: Option<String>,
    pub app_detail: Option<AppDetailResponse>,
    pub log_entries: Vec<String>,
    pub deploy_open: bool,
    pub deploy_name: String,
    pub deploy_runtime: String,
    pub deploy_build: String,
    pub deploy_run: String,
    pub deploy_path: String,
    pub deploy_env_vars: Vec<(String, String)>,
    pub deploy_env_key: String,
    pub deploy_env_value: String,
    pub next_deploy_id: usize,
    pub active_deploys: Vec<DeployState>,
    pub expanded_deploy: Option<usize>,
    pub confirm_delete: Option<(String, String)>,
    pub confirm_device_delete: Option<usize>,
    pub env_key: String,
    pub env_value: String,
    pub build_cmd_input: String,
    pub run_cmd_input: String,
    pub toasts: Vec<ToastState>,
}

impl RoverApp {
    /// Find a mutable deploy by client-side id.
    pub fn find_deploy_mut(&mut self, deploy_id: usize) -> Option<&mut DeployState> {
        self.active_deploys
            .iter_mut()
            .find(|deploy| deploy.id == deploy_id)
    }

    /// Count deploys that are not yet terminal.
    pub fn active_deploy_count(&self) -> usize {
        self.active_deploys
            .iter()
            .filter(|deploy| deploy.is_active())
            .count()
    }
}
