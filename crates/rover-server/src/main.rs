mod auth;
mod bore;
mod deploy;
mod health;
mod metrics;
mod process;
mod runtime;
mod server;
mod state;

use clap::Parser;
use std::path::PathBuf;

/// Rover — a tiny PaaS that runs on Android/Termux.
#[derive(Parser, Debug)]
#[command(name = "roverd", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Transport mode: 'lan' or 'relay' (relay is not implemented yet)
    #[arg(long, default_value = "lan")]
    mode: String,

    /// Port to listen on
    #[arg(long, default_value = "9050")]
    port: u16,

    /// Data directory for state, apps, and logs
    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Enable bore tunneling to expose the server to the internet
    #[arg(long)]
    bore: bool,

    /// Bore server address (default: bore.pub:7835)
    #[arg(long, default_value = "bore.pub")]
    bore_server: String,

    /// Bore server port (default: 7835)
    #[arg(long, default_value_t = 7835)]
    bore_port: u16,

    /// Bore authentication secret
    #[arg(long)]
    bore_secret: Option<String>,
}

fn default_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".rover")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "roverd=info,info".into()),
        )
        .init();

    let cli = Cli::parse();
    let data_dir = cli.data_dir.unwrap_or_else(default_data_dir);

    tracing::info!(
        "Rover server starting (mode={}, port={})",
        cli.mode,
        cli.port
    );
    tracing::info!("Data directory: {}", data_dir.display());

    // Ensure data directory exists
    std::fs::create_dir_all(&data_dir)?;

    // Initialize state store
    let store = StateStoreWrapper::open(&data_dir.join("rover.db"))?;
    tracing::info!("State store initialized");

    // Initialize auth — print pairing token if no API keys exist
    let auth_manager = auth::AuthManager::new(store.clone());
    let pairing_token = auth_manager.ensure_pairing_token()?;
    tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    tracing::info!("Pairing token: {}", pairing_token);
    tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Initialize runtime registry
    let registry = runtime::RuntimeRegistry::new();
    registry.register(runtime::python::PythonRuntime::new());
    registry.register(runtime::node::NodeRuntime::new());
    registry.register(runtime::go::GoRuntime::new());
    registry.register(runtime::rust::RustRuntime::new());
    tracing::info!("Runtimes available: python, node, go, rust");

    // Initialize process manager
    let process_manager = process::ProcessManager::new(store.clone());

    // Initialize deployer
    let deployer = deploy::Deployer::new(
        store.clone(),
        registry,
        process_manager.clone(),
        data_dir.clone(),
    );

    // Clean up orphaned app directories (deleted from DB but still on disk)
    if let Err(e) = deployer.cleanup_orphan_dirs() {
        tracing::warn!("failed to clean up orphan app directories: {e}");
    }

    // Start health check loop (background)
    let health_checker = health::HealthChecker::new(store.clone(), process_manager.clone());
    tokio::spawn(async move {
        health_checker.run().await;
    });

    // Start bore tunnel if enabled
    if cli.bore {
        let bore_config = bore::BoreConfig {
            server_host: cli.bore_server,
            server_port: cli.bore_port,
            secret: cli.bore_secret,
            local_port: cli.port,
        };
        match bore::start_tunnel(bore_config).await {
            Ok(tunnel) => {
                tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                tracing::info!("Public address: {}", tunnel.public_address());
                tracing::info!("Use this address to connect from the Rover client");
                tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
            Err(e) => {
                tracing::warn!("Failed to establish bore tunnel: {e}");
                tracing::warn!("Server will continue without public access");
            }
        }
    }

    // Start gRPC server
    server::start(
        cli.port,
        store,
        auth_manager,
        deployer,
        process_manager,
        &data_dir,
    )
    .await?;

    Ok(())
}

// Bridge: StateStore in state.rs is Arc<StateStore>, but main.rs modules expect Arc<StateStore>
// We use a type alias for clarity.
use state::StateStore;
type StateStoreWrapper = StateStore;
