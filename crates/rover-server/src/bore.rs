/// Bore tunnel integration — exposes the local gRPC server to the internet
/// via a bore public relay, bypassing NAT/firewalls.
use anyhow::Context;
use bore_cli::client::Client;
use tokio::task::JoinHandle;

/// An active bore tunnel forwarding traffic to our local gRPC server.
pub struct BoreTunnel {
    /// Full public address string (e.g. "bore.pub:51923").
    pub address: String,
    /// Background task running the tunnel listener — aborted on drop.
    _task: JoinHandle<()>,
}

impl BoreTunnel {
    pub fn public_address(&self) -> String {
        self.address.clone()
    }
}

/// Configuration for the bore tunnel client.
#[derive(Debug, Clone)]
pub struct BoreConfig {
    /// Address of the bore server (e.g. "bore.pub").
    pub server_host: String,
    /// Optional secret for authenticated tunnels.
    pub secret: Option<String>,
    /// The local port to expose (our gRPC server).
    pub local_port: u16,
}

impl Default for BoreConfig {
    fn default() -> Self {
        Self {
            server_host: "bore.pub".into(),
            secret: None,
            local_port: 9050,
        }
    }
}

/// Start a bore tunnel using the bore server's default port (port 0 signals
/// the client to use the well-known bore port). Returns the tunnel with the
/// randomly assigned remote port.
pub async fn start_tunnel(config: BoreConfig) -> anyhow::Result<BoreTunnel> {
    tracing::info!(
        "Establishing bore tunnel to {} for local port {}",
        config.server_host,
        config.local_port,
    );

    // port = 0 asks the Bore client to use the server's default port (7835).
    let client = Client::new(
        "localhost",
        config.local_port,
        &config.server_host,
        0,
        config.secret.as_deref(),
    )
    .await
    .context(format!(
        "Failed to connect to bore server '{}'. \
         Make sure the server is running and reachable from this network.",
        config.server_host,
    ))?;

    let remote_port = client.remote_port();
    let address = format!("{}:{remote_port}", config.server_host);

    // Spawn the listener as a background task — it runs for the lifetime of the tunnel
    let task = tokio::spawn(async move {
        if let Err(err) = client.listen().await {
            tracing::error!("Bore tunnel listener stopped: {err:#}");
        }
    });

    let tunnel = BoreTunnel {
        address,
        _task: task,
    };

    tracing::info!(
        "Bore tunnel established — public address: {}",
        tunnel.address
    );

    Ok(tunnel)
}
