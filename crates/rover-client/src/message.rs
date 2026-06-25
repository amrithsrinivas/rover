/// Message enum for all UI events and async responses.
/// Shared across all screens and widgets.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    Navigate(crate::Screen),

    // Periodic refresh
    Tick,

    // Connection
    Connect {
        address: String,
        pairing_token: String,
    },
    ConnectWithApiKey {
        address: String,
        api_key: String,
    },
    Disconnect,
    ConnectionSuccess,
    ConnectionError(String),

    // Profile management
    SaveProfile(String, String),
    DeleteProfile(String),

    // Server data
    ServerInfoRefreshed,
    MetricsRefreshed,
    AppListRefreshed,

    // App actions
    SelectApp(String),
    DeployApp,
    StartApp(String),
    StopApp(String),
    RestartApp(String),
    DeleteApp(String),
    AppActionComplete(Result<(), String>),

    // Streaming
    LogEntryReceived(String),
    DeployEventReceived(String),
    MetricsUpdate,

    // Env vars
    SetEnvVar {
        app_id: String,
        key: String,
        value: String,
    },
    DeleteEnvVar {
        app_id: String,
        key: String,
    },

    // Shell
    OpenShell(String),
    CloseShell,
    ShellInput(Vec<u8>),
    ShellOutput(Vec<u8>),

    // UI
    DismissToast(usize),
}
