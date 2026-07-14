use rover_proto::v1::{AppDetailResponse, AppSummary, DeployEvent, ServerInfo, ServerMetrics};

use crate::app::ClientRef;

/// All UI events and async responses.
#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    /// Periodic tick for auto-refresh (2-second interval).
    Tick,

    // ── Server management ───────────────────────────────────────────────
    /// Show the "Manage Servers" modal.
    ManageServers,
    /// Close the manage servers modal.
    CloseManageServers,
    /// Show the "Add server" connection form.
    ShowAddForm,
    /// Hide the connection form.
    HideAddForm,
    /// Connection form field: address.
    SetAddr(String),
    /// Connection form field: pairing token.
    SetToken(String),
    /// Connection form field: server name.
    SetServerName(String),
    /// Submit the connection form (pair with a new server).
    Connect,
    /// New server paired successfully.
    ServerAdded(String, ClientRef, String),
    /// Connection form error.
    ServerAddError(String),
    /// Existing server reconnected successfully.
    ServerConnected(usize, Option<ClientRef>),
    /// Existing server connection error.
    ServerError(usize, String),
    /// Disconnect a specific server by index.
    Disconnect(usize),
    /// Reconnect a disconnected server by index.
    Reconnect(usize),
    /// Show delete confirmation for a server profile.
    ConfirmServerDelete(usize),
    /// Dismiss server delete confirmation.
    CancelServerDelete,
    /// Execute server deletion.
    DeleteServer(usize),
    /// Start renaming a server.
    StartRename(usize),
    /// Rename text input.
    SetRenameValue(String),
    /// Confirm rename.
    ConfirmRename(usize),
    /// Cancel rename.
    CancelRename,

    // ── Data refresh ───────────────────────────────────────────────────
    /// Refresh result for one server: (server_index, info, metrics).
    ServerData(usize, Box<ServerInfo>, Box<ServerMetrics>),
    /// App list result for one server: (server_index, apps).
    ServerApps(usize, Vec<AppSummary>),

    // ── App detail ─────────────────────────────────────────────────────
    /// Navigate to app detail: (app_id, server_index).
    SelectApp(String, usize),
    /// GetApp RPC result.
    AppDetail(Box<AppDetailResponse>),
    /// StreamLogs result.
    LogLines(Vec<String>),
    /// Return to dashboard.
    BackToDashboard,
    /// Start an app: (app_id, server_index).
    StartApp(String, usize),
    /// Stop an app: (app_id, server_index).
    StopApp(String, usize),
    /// Restart an app: (app_id, server_index).
    RestartApp(String, usize),
    /// Show delete confirmation: (app_id, server_index).
    DeleteApp(String, usize),
    /// Dismiss delete confirmation.
    CancelDelete,
    /// Execute delete: (app_id, name, server_index).
    ConfirmDelete(String, String, usize),

    // ── Deploy ─────────────────────────────────────────────────────────
    OpenDeploy,
    CloseDeploy,
    /// Which server to deploy to.
    SetDeployTarget(Option<usize>),
    SetDeployName(String),
    SetDeployRuntime(String),
    SetDeployBuild(String),
    SetDeployRun(String),
    SetDeployPath(String),
    ToggleGithub,
    SetDeployGithubUrl(String),
    SelectGithubToken(Option<String>),
    SetNewTokenLabel(String),
    SetNewTokenValue(String),
    SaveGithubToken,
    PickPath,
    SetEnvKey(String),
    SetEnvValue(String),
    AddEnvVar,
    RemoveEnvVar(usize),
    PickEnvFile,
    EnvFilePicked(String, Vec<(String, String)>),
    SubmitDeploy,
    DeployStatus(usize, String),
    DeployEvent(usize, DeployEvent),
    DeployStreamEnded(usize),
    DeployError(usize, String),
    ToggleDeployLog(usize),
    ClearFinishedDeploys,

    // ── Update app commands ────────────────────────────────────────────
    OpenUpdate(String),
    CloseUpdate,
    SetUpdateBuild(String),
    SetUpdateRun(String),
    ConfirmUpdate(String),

    // ── Toast notifications ────────────────────────────────────────────
    Info(String),
    Error(String),
    DismissToast(usize),

    // ── Clipboard ──────────────────────────────────────────────────────
    Copy(String),
}
