use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::process::ProcessManager;
use crate::runtime::RuntimeRegistry;
use crate::state::StateStore;
use rover_core::{AppManifest, Runtime};

/// Orchestrates the deployment lifecycle: validate, extract, build, start.
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
        source_tar_gz: &[u8],
    ) -> anyhow::Result<mpsc::Receiver<DeployEvent>> {
        let (tx, rx) = mpsc::channel(64);

        manifest.validate()?;
        let runtime = manifest.runtime()?;
        let app_type = manifest.app_type()?;
        let app_id = uuid::Uuid::new_v4().to_string();

        let handler = self
            .registry
            .get_handler(runtime)
            .ok_or_else(|| anyhow::anyhow!("runtime not available: {runtime}"))?;

        // Check runtime is installed
        if !handler.check_installed().await? {
            return Err(anyhow::anyhow!("runtime {runtime} is not installed"));
        }

        // Create app directory
        let app_dir = self.data_dir.join("apps").join(&app_id);
        let source_dir = app_dir.join("source");
        std::fs::create_dir_all(&source_dir)?;

        // Insert app with status "deploying"
        self.store.insert_app(
            &app_id,
            &manifest.app.name,
            &manifest.app.runtime,
            &manifest.app.app_type,
            "deploying",
            &manifest.build.command,
            &manifest.run.command,
            &source_dir.to_string_lossy(),
            &toml::to_string(manifest)?,
        )?;

        let tx_log = tx.clone();
        // Extract source archive
        {
            let _ = tx_log.send(DeployEvent::log("Extracting source...")).await;
            extract_tar_gz(source_tar_gz, &source_dir)?;
        }

        // Run build
        let tx_build = tx.clone();
        {
            let _ = tx_build
                .send(DeployEvent::log(format!(
                    "Building with {}...",
                    runtime.display_name()
                )))
                .await;
            let _ = tx_build
                .send(DeployEvent::log(format!("> {}", manifest.build.command)))
                .await;

            handler
                .build(&source_dir, &manifest.build.command)
                .await
                .map_err(|e| {
                    let _ = tx_build.try_send(DeployEvent::error(format!("Build failed: {e}")));
                    let _ = self.store.update_app_status(&app_id, "failed");
                    e
                })?;
        }

        // Write rover.toml to app dir
        std::fs::write(app_dir.join("rover.toml"), toml::to_string(manifest)?)?;

        // Set env vars
        for (key, value) in &manifest.env {
            self.store.set_env_var(&app_id, key, value, false)?;
        }

        // Start the process
        {
            let _ = tx.send(DeployEvent::log("Starting app...")).await;
            self.store.update_app_status(&app_id, "starting")?;

            let (program, args) = crate::process::parse_shell_command(&manifest.run.command);
            let env_vars: std::collections::HashMap<String, String> = manifest.env.clone();

            self.process_manager
                .spawn(
                    &app_id,
                    &program,
                    &args,
                    &env_vars,
                    &source_dir,
                    &manifest.app.app_type,
                )
                .await?;

            self.store.update_app_status(&app_id, "running")?;
        }

        let _ = tx.send(DeployEvent::complete(app_id.clone())).await;

        Ok(rx)
    }
}

// ----------------------------------------------------------------------
// Deploy events (streamed to client during deployment)
// ----------------------------------------------------------------------

/// An event emitted during a deployment (build output, progress, completion).
#[derive(Debug, Clone)]
pub enum DeployEvent {
    Log(String),
    Complete { app_id: String },
    Error(String),
}

impl DeployEvent {
    pub fn log(line: impl Into<String>) -> Self {
        Self::Log(line.into())
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

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

/// Extract a .tar.gz archive to the destination directory.
fn extract_tar_gz(data: &[u8], dest: &Path) -> anyhow::Result<()> {
    let cursor = std::io::Cursor::new(data);
    let decoder = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest)?;
    Ok(())
}
