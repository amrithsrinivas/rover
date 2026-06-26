use core::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;

use rover_core::ConnectionProfile;
use rover_proto::v1::{AppSummary, ServerInfo, ServerMetrics};

use crate::api::client::RoverClient;

/// Represents a single device (phone) being managed.
pub struct DeviceState {
    /// The saved connection profile.
    pub profile: ConnectionProfile,
    /// The gRPC client, wrapped for shared access.
    pub client: Option<Arc<Mutex<RoverClient>>>,
    /// Whether the device is currently connected.
    pub connected: bool,
    /// Last fetched server info.
    pub info: Option<ServerInfo>,
    /// Last fetched metrics.
    pub metrics: Option<ServerMetrics>,
    /// Last fetched app list.
    pub apps: Vec<AppSummary>,
    /// Whether a connection attempt is in progress.
    pub connecting: bool,
    /// Last connection error, if any.
    pub err: Option<String>,
}

impl DeviceState {
    /// Create a DeviceState from a saved profile (disconnected).
    pub fn from_profile(profile: ConnectionProfile) -> Self {
        Self {
            profile,
            client: None,
            connected: false,
            info: None,
            metrics: None,
            apps: Vec::new(),
            connecting: false,
            err: None,
        }
    }
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.profile.name)
    }
}
