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
        let _runtime = manifest.runtime()?;
        let app_id = uuid::Uuid::new_v4().to_string();
        let app_name = manifest.app.name.clone();
        let build_cmd = manifest.build.command.clone();
        let run_cmd = manifest.run.command.clone();
        let runtime_str = manifest.app.runtime.clone();
        let env_vars = manifest.env.clone();
        let manifest_toml = toml::to_string(manifest)?;

        let data_dir = self.data_dir.clone();
        let store = self.store.clone();
        let pm = self.process_manager.clone();
        let registry = self.registry.clone();

        tokio::spawn(async move {
            if let Err(e) = run_deploy(
                tx.clone(),
                &app_id,
                &app_name,
                &runtime_str,
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
            .await
            {
                tracing::error!(app_id=%app_id, error=%e, "deploy failed");
                let _ = tx
                    .send(DeployEvent::error(format!("deploy failed: {e}")))
                    .await;
            }
        });

        Ok(rx)
    }

    /// Remove the on-disk app directory for a deleted app.
    ///
    /// This removes `{data_dir}/apps/{app_id}/` and all its contents.
    /// Does nothing if the directory doesn't exist.
    pub fn cleanup_app_dir(&self, app_id: &str) -> anyhow::Result<()> {
        let app_dir = self.data_dir.join("apps").join(app_id);
        if app_dir.exists() {
            std::fs::remove_dir_all(&app_dir)?;
            tracing::info!(app_id=%app_id, path=%app_dir.display(), "cleaned up app directory");
        } else {
            tracing::debug!(app_id=%app_id, "app directory not found, nothing to clean up");
        }
        Ok(())
    }

    /// On startup, scan `{data_dir}/apps/` for directories that have no
    /// corresponding row in the database, and remove them.
    pub fn cleanup_orphan_dirs(&self) -> anyhow::Result<()> {
        let apps_dir = self.data_dir.join("apps");
        if !apps_dir.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&apps_dir)?;
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("failed to read entry in apps dir: {e}");
                    continue;
                }
            };

            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }

            let app_id = entry.file_name().to_string_lossy().to_string();
            // Check if this app_id exists in the database
            if self.store.get_app(&app_id).unwrap_or(None).is_none() {
                tracing::info!(
                    app_id=%app_id,
                    "removing orphaned app directory with no DB entry"
                );
                if let Err(e) = std::fs::remove_dir_all(entry.path()) {
                    tracing::warn!(app_id=%app_id, error=%e, "failed to remove orphaned app dir");
                }
            }
        }

        Ok(())
    }
}

async fn run_deploy(
    tx: mpsc::Sender<DeployEvent>,
    app_id: &str,
    app_name: &str,
    runtime_str: &str,
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
    tracing::info!(app_id=%app_id, app_name=%app_name, runtime=%runtime_str, "deploy starting");
    tracing::info!(app_id=%app_id, "source archive size: {} bytes", source_tar_gz.len());
    use rover_core::Runtime;
    let runtime: Runtime = match runtime_str.parse() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(app_id=%app_id, "invalid runtime '{runtime_str}': {e}");
            let _ = tx
                .send(DeployEvent::error(format!(
                    "invalid runtime: {runtime_str}"
                )))
                .await;
            store.update_app_status(app_id, "failed")?;
            return Ok(());
        }
    };

    let handler = match registry.get_handler(runtime) {
        Some(h) => h,
        None => {
            tracing::error!(app_id=%app_id, "runtime {runtime_str} not registered");
            let _ = tx
                .send(DeployEvent::error(format!(
                    "runtime not available: {runtime_str}"
                )))
                .await;
            store.update_app_status(app_id, "failed")?;
            return Ok(());
        }
    };

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
        "service",
        "deploying",
        build_cmd,
        run_cmd,
        &source_dir.to_string_lossy(),
        manifest_toml,
    )?;

    // Extract source
    let _ = tx.send(DeployEvent::log("Extracting source...")).await;
    extract_tar_gz(&source_tar_gz, &source_dir)?;
    tracing::info!(app_id=%app_id, "source extracted to {}", source_dir.display());

    // Build
    let _ = tx
        .send(DeployEvent::log(format!("Building with {runtime_str}...")))
        .await;
    let _ = tx.send(DeployEvent::log(format!("> {build_cmd}"))).await;

    let build_tx = tx.clone();
    let build_dir = source_dir.clone();
    let build_cmd_owned = build_cmd.to_string();
    let build_env = env_vars.clone();
    let build_result = tokio::task::spawn_blocking(move || {
        run_build_and_stream(&build_dir, &build_cmd_owned, &build_tx, &build_env)
    })
    .await??;
    tracing::info!(app_id=%app_id, ?build_result, "build finished");

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
    pm.spawn(app_id, &program, &args, env_vars, &source_dir)
        .await?;
    store.update_app_status(app_id, "running")?;
    tracing::info!(app_id=%app_id, "app started successfully");

    let _ = tx.send(DeployEvent::complete(app_id)).await;
    Ok(())
}

/// Run a build command synchronously (spawn_blocking) and stream stdout/stderr lines.
fn run_build_and_stream(
    dir: &Path,
    command: &str,
    tx: &mpsc::Sender<DeployEvent>,
    env_vars: &std::collections::HashMap<String, String>,
) -> anyhow::Result<std::process::ExitStatus> {
    use std::io::{BufRead, BufReader};
    use std::process::Command;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .envs(env_vars)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimeRegistry;

    fn make_deployer(tmp: &tempfile::TempDir) -> Deployer {
        let store = StateStore::open(&tmp.path().join("rover.db")).unwrap();
        let registry = RuntimeRegistry::new();
        let pm = ProcessManager::new(store.clone());
        Deployer::new(store, registry, pm, tmp.path().to_path_buf())
    }

    #[test]
    fn test_cleanup_orphan_dirs_removes_orphans() {
        let tmp = tempfile::tempdir().unwrap();
        let deployer = make_deployer(&tmp);

        // Create an orphan app directory (dir exists but no DB entry)
        let orphan_dir = tmp.path().join("apps").join("orphan-app-id");
        std::fs::create_dir_all(orphan_dir.join("source")).unwrap();
        // Write a dummy file
        std::fs::write(orphan_dir.join("source").join("main.py"), "print('hi')").unwrap();

        // Also create an app that IS in the DB (should not be removed)
        let kept_dir = tmp.path().join("apps").join("kept-app-id");
        std::fs::create_dir_all(kept_dir.join("source")).unwrap();
        std::fs::write(kept_dir.join("source").join("main.py"), "print('hi')").unwrap();
        deployer
            .store
            .insert_app(
                "kept-app-id",
                "kept",
                "python",
                "service",
                "stopped",
                "pip install",
                "python main.py",
                "/tmp/kept",
                "",
            )
            .unwrap();

        // Run cleanup
        deployer.cleanup_orphan_dirs().unwrap();

        // Orphan should be gone
        assert!(!orphan_dir.exists(), "orphan directory should be removed");
        // Kept app should still exist
        assert!(kept_dir.exists(), "kept app directory should remain");
    }

    #[test]
    fn test_cleanup_app_dir_removes_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let deployer = make_deployer(&tmp);

        // Create an app directory
        let app_dir = tmp.path().join("apps").join("test-app");
        std::fs::create_dir_all(app_dir.join("source")).unwrap();
        std::fs::write(app_dir.join("source").join("main.py"), "print('hi')").unwrap();

        assert!(app_dir.exists());

        deployer.cleanup_app_dir("test-app").unwrap();

        assert!(!app_dir.exists(), "app directory should be removed");
    }

    #[test]
    fn test_cleanup_app_dir_nonexistent_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let deployer = make_deployer(&tmp);

        // Should not panic or error for a nonexistent app
        deployer.cleanup_app_dir("nonexistent-app").unwrap();
    }
}
