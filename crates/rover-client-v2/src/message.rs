use rover_proto::v1::{AppDetailResponse, AppSummary, DeployEvent, ServerInfo, ServerMetrics};

use crate::app::ClientRef;

/// All UI events and async responses.
#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    /// Periodic tick for auto-refresh (2-second interval).
    Tick,

    // ── Server management ───────────────────────────────────────────────
    ManageServers,
    CloseManageServers,
    ShowAddForm,
    HideAddForm,
    SetAddr(String),
    SetToken(String),
    SetServerName(String),
    Connect,
    ServerAdded(String, ClientRef, String),
    ServerAddError(String),
    ServerConnected(usize, Option<ClientRef>),
    ServerError(usize, String),
    Disconnect(usize),
    Reconnect(usize),
    ConfirmServerDelete(usize),
    CancelServerDelete,
    DeleteServer(usize),
    StartRename(usize),
    SetRenameValue(String),
    ConfirmRename(usize),
    CancelRename,

    // ── Data refresh ───────────────────────────────────────────────────
    ServerData(usize, Box<ServerInfo>, Box<ServerMetrics>),
    ServerApps(usize, Vec<AppSummary>),

    // ── App detail ─────────────────────────────────────────────────────
    SelectApp(String, usize),
    AppDetail(Box<AppDetailResponse>),
    LogLines(Vec<String>),
    BackToDashboard,
    StartApp(String, usize),
    StopApp(String, usize),
    RestartApp(String, usize),
    DeleteApp(String, usize),
    CancelDelete,
    ConfirmDelete(String, String, usize),

    // ── Deploy ─────────────────────────────────────────────────────────
    OpenDeploy,
    CloseDeploy,
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

    // ── Terminal ──────────────────────────────────────────────────────
    /// Open the system shell for a server.
    OpenTerminal(usize),
    /// Shell session established — store the input sender.
    ShellStarted(tokio::sync::mpsc::Sender<rover_proto::v1::ShellInput>),
    /// Shell output received from the server.
    ShellOutput(Vec<u8>),
    /// Shell stream ended.
    ShellClosed,
    /// Terminal text input field changed.
    SetTerminalInput(String),
    /// Send the current input as a shell command.
    SubmitShellCommand,
    /// Close the terminal session.
    CloseTerminal,
}
