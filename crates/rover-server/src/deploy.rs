use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::process::ProcessManager;
use crate::runtime::RuntimeRegistry;
use crate::state::StateStore;
use rover_core::AppManifest;

#[derive(Debug, Clone)]
pub enum DeployEvent {
    Log { line: String, is_stderr: bool },
    Complete { app_id: String },
    Error(String),
}

impl DeployEvent {
    pub fn log(line: impl Into<String>) -> Self {
        Self::Log {
            line: line.into(),
            is_stderr: false,
        }
    }
    pub fn complete(app_id: impl Into<String>) -> Self {
        Self::Complete {
            app_id: app_id.into(),
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }
}

pub struct Deployer {
    store: Arc<StateStore>,
    registry: RuntimeRegistry,
    process_manager: ProcessManager,
    data_dir: PathBuf,
}

impl Deployer {
    pub fn new(
        store: Arc<StateStore>,
        registry: RuntimeRegistry,
        process_manager: ProcessManager,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            store,
            registry,
            process_manager,
            data_dir,
        }
    }

    /// Deploy an app from a manifest and source archive.
    /// Returns a receiver for streaming deploy events.
    pub async fn deploy(
        &self,
        manifest: &AppManifest,
        source_tar_gz: Vec<u8>,
    ) -> anyhow::Result<mpsc::Receiver<DeployEvent>> {
        let (tx, rx) = mpsc::channel(128);

        manifest.validate()?;
        let runtime = manifest.runtime()?;
        let app_type_str = manifest.app_type()?;
        let app_id = uuid::Uuid::new_v4().to_string();
        let app_name = manifest.app.name.clone();
        let build_cmd = manifest.build.command.clone();
        let run_cmd = manifest.run.command.clone();
        let runtime_str = manifest.app.runtime.clone();
        let app_type = manifest.app.app_type.clone();
        let env_vars = manifest.env.clone();
        let manifest_toml = toml::to_string(manifest)?;

        let data_dir = self.data_dir.clone();
        let store = self.store.clone();
        let pm = self.process_manager.clone();
        let registry = self.registry.clone();

        // Spawn the full deploy as a background task so the channel doesn't block
        tokio::spawn(async move {
            let _ = run_deploy(
                tx,
                &app_id,
                &app_name,
                &runtime_str,
                &app_type,
                &build_cmd,
                &run_cmd,
                &env_vars,
                &manifest_toml,
                source_tar_gz,
                &data_dir,
                &store,
                &pm,
                &registry,
            )
            .await;
        });

        Ok(rx)
    }
}

async fn run_deploy(
    tx: mpsc::Sender<DeployEvent>,
    app_id: &str,
    app_name: &str,
    runtime_str: &str,
    app_type: &str,
    build_cmd: &str,
    run_cmd: &str,
    env_vars: &std::collections::HashMap<String, String>,
    manifest_toml: &str,
    source_tar_gz: Vec<u8>,
    data_dir: &Path,
    store: &Arc<StateStore>,
    pm: &ProcessManager,
    registry: &RuntimeRegistry,
) -> anyhow::Result<()> {
    use rover_core::Runtime;
    let runtime: Runtime = runtime_str
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;

    let handler = registry
        .get_handler(runtime)
        .ok_or_else(|| anyhow::anyhow!("runtime not available: {runtime_str}"))?;

    // Check runtime installed
    if !handler.check_installed().await? {
        let _ = tx
            .send(DeployEvent::error(format!(
                "Runtime {runtime_str} is not installed"
            )))
            .await;
        let _ = store.update_app_status(app_id, "failed");
        return Err(anyhow::anyhow!("runtime not installed"));
    }

    // Prepare directories
    let app_dir = data_dir.join("apps").join(app_id);
    let source_dir = app_dir.join("source");
    std::fs::create_dir_all(&source_dir)?;

    // Insert app row
    store.insert_app(
        app_id,
        app_name,
        runtime_str,
        app_type,
        "deploying",
        build_cmd,
        run_cmd,
        &source_dir.to_string_lossy(),
        manifest_toml,
    )?;

    // Extract source
    let _ = tx.send(DeployEvent::log("Extracting source...")).await;
    extract_tar_gz(&source_tar_gz, &source_dir)?;

    // Build — capture stdout/stderr live
    let _ = tx
        .send(DeployEvent::log(format!(
            "Building with {}...",
            runtime_str
        )))
        .await;
    let _ = tx.send(DeployEvent::log(format!("> {build_cmd}"))).await;

    let build_tx = tx.clone();
    let build_dir = source_dir.clone();
    let build_cmd_owned = build_cmd.to_string();
    let build_result = tokio::task::spawn_blocking(move || {
        run_build_and_stream(&build_dir, &build_cmd_owned, &build_tx)
    })
    .await??;

    if !build_result.success() {
        let _ = tx
            .send(DeployEvent::error(String::from(
                "Build failed — check output above",
            )))
            .await;
        store.update_app_status(app_id, "failed")?;
        return Ok(());
    }

    // Write manifest
    std::fs::write(app_dir.join("rover.toml"), manifest_toml)?;

    // Set env vars
    for (k, v) in env_vars {
        store.set_env_var(app_id, k, v, false)?;
    }

    // Start the process
    let _ = tx.send(DeployEvent::log("Starting app...")).await;
    store.update_app_status(app_id, "starting")?;

    let (program, args) = crate::process::parse_shell_command(run_cmd);
    pm.spawn(app_id, &program, &args, env_vars, &source_dir, app_type)
        .await?;
    store.update_app_status(app_id, "running")?;

    let _ = tx.send(DeployEvent::complete(app_id)).await;
    Ok(())
}

/// Run a build command synchronously (spawn_blocking) and stream stdout/stderr lines.
fn run_build_and_stream(
    dir: &Path,
    command: &str,
    tx: &mpsc::Sender<DeployEvent>,
) -> anyhow::Result<std::process::ExitStatus> {
    use std::io::{BufRead, BufReader};
    use std::process::Command;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Spawn threads to read stdout/stderr and send via the channel
    let tx_stdout = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx_stdout.blocking_send(DeployEvent::Log {
                line,
                is_stderr: false,
            });
        }
    });

    let tx_stderr = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx_stderr.blocking_send(DeployEvent::Log {
                line,
                is_stderr: true,
            });
        }
    });

    let status = child.wait()?;
    Ok(status)
}

/// Extract a .tar.gz archive to the destination directory.
fn extract_tar_gz(data: &[u8], dest: &Path) -> anyhow::Result<()> {
    let cursor = std::io::Cursor::new(data);
    let decoder = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest)?;
    Ok(())
}
