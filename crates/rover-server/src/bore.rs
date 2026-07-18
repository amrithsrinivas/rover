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
    /// Previously assigned remote port — will be reused on reconnect.
    pub remote_port: Option<u16>,
}

impl Default for BoreConfig {
    fn default() -> Self {
        Self {
            server_host: "bore.pub".into(),
            secret: None,
            local_port: 9050,
            remote_port: None,
        }
    }
}

/// Start a bore tunnel, optionally reusing a previously assigned remote port.
/// If `config.remote_port` is set, the client requests that specific port.
/// If the server rejects it, we fall back to a random port (0).
pub async fn start_tunnel(config: BoreConfig) -> anyhow::Result<BoreTunnel> {
    let preferred = config.remote_port.unwrap_or(0);

    tracing::info!(
        "Establishing bore tunnel to {} for local port {}",
        config.server_host,
        config.local_port,
    );

    // Try with the preferred port first. 0 means random.
    let client = if preferred != 0 {
        tracing::info!("Requesting previously assigned remote port {preferred}...");
        match Client::new(
            "localhost",
            config.local_port,
            &config.server_host,
            preferred,
            config.secret.as_deref(),
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "Could not reuse port {preferred}: {e}. Falling back to random port..."
                );
                Client::new(
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
                ))?
            }
        }
    } else {
        Client::new(
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
        ))?
    };

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

/// Start a persistent bore tunnel that reconnects on failure.
///
/// When the tunnel drops (e.g. connection reset), it reconnects automatically.
/// On reconnect, it first tries to reuse the previously assigned port. If that
/// fails (port was taken by someone else), it falls back to a random port.
///
/// Spawn this as a background task — it runs forever.
pub async fn run_tunnel_loop(mut config: BoreConfig) {
    loop {
        match start_tunnel(config.clone()).await {
            Ok(tunnel) => {
                let addr = tunnel.public_address();
                tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                tracing::info!("Public address: {addr}");
                tracing::info!("Use this address to connect from the Rover client");
                tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                eprintln!();
                eprintln!("╔════════════════════════════════════════════╗");
                eprintln!("║  Bore tunnel established                  ║");
                eprintln!("║  Public address: {addr:<24} ║");
                eprintln!("╚════════════════════════════════════════════╝");
                eprintln!();

                // Save the assigned port so on reconnect we try to reuse it
                let remote_port = addr.split(':').last().and_then(|p| p.parse().ok());
                config.remote_port = remote_port;

                // Wait for the tunnel to drop, then reconnect
                let _ = tokio::join!(tunnel._task);

                tracing::warn!("Bore tunnel lost — reconnecting in 5 seconds...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            Err(e) => {
                tracing::warn!("Failed to establish bore tunnel: {e}");
                tracing::warn!("Retrying in 10 seconds...");
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        }
    }
}
