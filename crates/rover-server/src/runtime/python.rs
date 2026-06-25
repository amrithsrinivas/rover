use async_trait::async_trait;
use std::path::Path;

use rover_core::{RoverError, Runtime};

use super::RuntimeHandler;

pub struct PythonRuntime;

impl PythonRuntime {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RuntimeHandler for PythonRuntime {
    fn runtime(&self) -> Runtime {
        Runtime::Python
    }

    /// Check if Python is installed via `which python`.
    async fn check_installed(&self) -> Result<bool, RoverError> {
        let output = tokio::process::Command::new("which")
            .arg("python")
            .output()
            .await
            .map_err(|e| RoverError::RuntimeNotAvailable(format!("failed to check python: {e}")))?;

        Ok(output.status.success())
    }

    async fn build(&self, app_dir: &Path, command: &str) -> Result<(), RoverError> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(app_dir)
            .output()
            .await
            .map_err(|e| RoverError::BuildFailed(format!("build command failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RoverError::BuildFailed(format!(
                "build exited with {}: {stderr}",
                output.status
            )));
        }

        Ok(())
    }

    fn run_command(&self, _app_dir: &Path, command: &str) -> (String, Vec<String>) {
        // Use `sh -c` so shell features (pipes, redirects, etc.) work
        (
            "sh".to_string(),
            vec!["-c".to_string(), command.to_string()],
        )
    }
}
