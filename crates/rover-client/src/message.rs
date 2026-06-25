use rover_proto::v1::{AppDetailResponse, AppSummary, ServerInfo, ServerMetrics};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api::client::RoverClient;

/// Message enum for all UI events and async responses.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    Navigate(crate::Screen),
    Tick,
    Noop,

    // Connection form
    SetAddressInput(String),
    SetTokenInput(String),
    SetProfileName(String),
    Connect,
    ConnectWithKey(String, String),
    ConnectionSuccess(Arc<Mutex<RoverClient>>, String),
    ConnectionError(String),
    Disconnect,
    DeleteProfile(String),

    // Server data
    Refresh,
    RefreshApps,
    DataRefreshed(Box<ServerInfo>, Box<ServerMetrics>),
    AppsRefreshed(Vec<AppSummary>),

    // App detail
    SelectApp(String),
    AppDetailLoaded(Box<AppDetailResponse>),
    StartApp(String),
    StopApp(String),
    RestartApp(String),
    DeleteApp(String),

    // Deploy
    Deploy,
    SubmitDeploy,
    DeployComplete,
    DeployError(String),
    SetDeployName(String),
    SetDeployBuildCmd(String),
    SetDeployRunCmd(String),
    SetDeployRuntime(String),
    SetDeployAppType(String),
    SetDeploySourcePath(String),
    PickSourceDirectory,

    // Env vars
    SetEnvKey(String),
    SetEnvValue(String),
    SetEnvSecret(bool),
    AddEnvVar,
    SaveProfile(String, String),

    // UI
    DismissToast(usize),
    ToastError(String),
    ToastInfo(String),
}
