/// Bore tunnel integration — exposes the local gRPC server to the internet
/// via a bore public relay, bypassing NAT/firewalls.
///
/// Uses the `bore-cli` library crate to register a tunnel and obtain a
/// publicly reachable address.
use anyhow::Context;
use bore_cli::client::Client;

/// Result of successfully establishing a bore tunnel.
#[derive(Debug, Clone)]
pub struct BoreTunnel {
    /// The remote bore server hostname (e.g. "bore.pub").
    pub remote_host: String,
    /// The remote port assigned by the bore server.
    pub remote_port: u16,
}

impl BoreTunnel {
    /// Returns the full public address string (e.g. "bore.pub:12345").
    pub fn public_address(&self) -> String {
        format!("{}:{}", self.remote_host, self.remote_port)
    }
}

/// Configuration for the bore tunnel client.
#[derive(Debug, Clone)]
pub struct BoreConfig {
    /// Address of the bore server (e.g. "bore.pub").
    pub server_host: String,
    /// Port of the bore server (default 7835).
    pub server_port: u16,
    /// Optional secret for authenticated tunnels.
    pub secret: Option<String>,
    /// The local port to expose (our gRPC server).
    pub local_port: u16,
}

impl Default for BoreConfig {
    fn default() -> Self {
        Self {
            server_host: "bore.pub".into(),
            server_port: 7835,
            secret: None,
            local_port: 9050,
        }
    }
}

/// Start a bore tunnel, returning the public address details.
/// Spawns the tunnel listener as a background tokio task.
pub async fn start_tunnel(config: BoreConfig) -> anyhow::Result<BoreTunnel> {
    tracing::info!(
        "Establishing bore tunnel to {}:{} for local port {}",
        config.server_host,
        config.server_port,
        config.local_port,
    );

    let client = Client::new(
        "127.0.0.1",
        config.local_port,
        &config.server_host,
        config.server_port,
        config.secret.as_deref(),
    )
    .await
    .context("Failed to create bore client — is the bore server reachable?")?;

    let remote_port = client.remote_port();
    let tunnel = BoreTunnel {
        remote_host: config.server_host.clone(),
        remote_port,
    };

    // Spawn the listener in the background — it runs for the lifetime of the process
    tokio::spawn(async move {
        if let Err(e) = client.listen().await {
            tracing::error!("Bore tunnel listener exited with error: {e}");
        } else {
            tracing::info!("Bore tunnel listener shut down cleanly");
        }
    });

    tracing::info!(
        "Bore tunnel established — public address: {}",
        tunnel.public_address()
    );

    Ok(tunnel)
}
