use std::sync::Arc;
use tokio::sync::Mutex;

use rover_proto::v1::{AppDetailResponse, AppSummary, DeployEvent, ServerInfo, ServerMetrics};

use crate::api::client::RoverClient;

/// All UI events and async responses.
#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    /// 2-second subscription timer for auto-refresh.
    Tick,

    // --- Device list ---
    /// Select a device by index.
    Select(usize),
    /// Show the "Add device" connection form.
    ShowAdd,
    /// Hide the connection form.
    HideAdd,
    /// Connection form field: address.
    SetAddr(String),
    /// Connection form field: pairing token.
    SetToken(String),
    /// Connection form field: device name.
    SetName(String),
    /// Submit the connection form (pair with a new device).
    Connect,
    /// New device paired successfully: (name, client, api_key).
    DevAdded(String, ClientRef, String),
    /// Connection form error.
    DevAddErr(String),
    /// Existing device reconnected successfully: (index, client).
    DevConnected(usize, Option<ClientRef>),
    /// Existing device connection error: (index, error).
    DevError(usize, String),
    /// Disconnect from the active device.
    Disconnect,
    /// Show delete confirmation for a device profile.
    DeleteDevice(usize),
    /// Start renaming a device.
    StartRename(usize),
    /// Rename text input.
    SetRenameValue(String),
    /// Confirm rename.
    ConfirmRename(usize),
    /// Cancel rename.
    CancelRename,
    /// Dismiss device delete confirmation.
    CancelDeleteDevice,
    /// Execute device deletion: index.
    ConfirmDeleteDevice(usize),

    // --- Data refresh ---
    /// Refresh result: (info, metrics).
    Data(Box<ServerInfo>, Box<ServerMetrics>),
    /// RefreshApps result.
    Apps(Vec<AppSummary>),

    // --- App detail ---
    /// Navigate to app detail for the given app_id.
    SelectApp(String),
    /// GetApp RPC result.
    Detail(Box<AppDetailResponse>),
    /// StreamLogs result.
    Logs(Vec<String>),
    /// Return to dashboard.
    Back,
    /// Start an app.
    Start(String),
    /// Stop an app.
    Stop(String),
    /// Restart an app.
    Restart(String),
    /// Show delete confirmation for an app_id.
    Delete(String),
    /// Dismiss delete confirmation.
    CancelDelete,
    /// Execute delete: (app_id, name).
    ConfirmDelete(String, String),

    // --- Deploy modal ---
    /// Show deploy modal.
    OpenDeploy,
    /// Hide deploy modal.
    CloseDeploy,
    /// Deploy form: app name.
    SetDName(String),
    /// Deploy form: runtime.
    SetDRuntime(String),
    /// Deploy form: build command.
    SetDBuild(String),
    /// Deploy form: run command.
    SetDRun(String),
    /// Deploy form: source path.
    SetDPath(String),
    /// Toggle between local dir and GitHub deploy.
    ToggleGithub,
    /// Deploy form: GitHub URL.
    SetDGithubUrl(String),
    /// Select a saved GitHub token by index, or None for manual entry.
    SelectGithubToken(Option<String>),
    /// Text input for new token label.
    SetNewTokenLabel(String),
    /// Text input for new token value.
    SetNewTokenValue(String),
    /// Save a new token from the inputs.
    SaveGithubToken,
    /// Open native folder picker.
    PickPath,
    /// Deploy form: env var key.
    SetDEKey(String),
    /// Deploy form: env var value.
    SetDEValue(String),
    /// Add env var to deploy list.
    AddDEVar,
    /// Remove env var from deploy list by index.
    RemoveDEVar(usize),
    /// Set the path to a .env file to import.
    SetDEnvFile(String),
    /// Open file picker for .env file.
    PickEnvFile,
    /// .env file picked via Browse: (path, parsed vars).
    EnvFilePicked(String, Vec<(String, String)>),
    /// Submit deploy.
    SubmitDeploy,
    /// Background deploy status changed: (deploy_id, status).
    DeployStatus(usize, String),
    /// Background deploy emitted a stream event: (deploy_id, event).
    DeployEvent(usize, DeployEvent),
    /// Background deploy task finished without a final server event.
    DeployStreamEnded(usize),
    /// Background deploy failed outside the server event stream: (deploy_id, error).
    DeployErr(usize, String),
    /// Expand/collapse a dashboard deploy log card.
    ToggleDeployLog(usize),
    /// Clear completed and failed deploy cards.
    ClearFinishedDeploys,

    // --- Update app commands modal ---
    /// Open the update commands modal with the app_id.
    /// Prefills build/run from current detail.
    OpenUpdate(String),
    /// Close the update commands modal.
    CloseUpdate,
    /// Build command input in the update modal.
    SetUpdateBuild(String),
    /// Run command input in the update modal.
    SetUpdateRun(String),
    /// Submit the update (sends to server, stop + restart if running).
    ConfirmUpdate(String),

    // --- Toast notifications ---
    /// Show an informational toast.
    Info(String),
    /// Show an error toast.
    Toast(String),
    /// Dismiss a toast by index.
    Dismiss(usize),

    // --- Clipboard ---
    /// Copy the given text to the system clipboard.
    Copy(String),
}

/// Shared reference to the gRPC client, used across async tasks.
pub type ClientRef = Arc<Mutex<RoverClient>>;
