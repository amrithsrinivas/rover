use async_trait::async_trait;
use std::path::Path;

use rover_core::{RoverError, Runtime};

use super::RuntimeHandler;

pub struct GoRuntime;

impl GoRuntime {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RuntimeHandler for GoRuntime {
    fn runtime(&self) -> Runtime {
        Runtime::Go
    }

    async fn check_installed(&self) -> Result<bool, RoverError> {
        let output = tokio::process::Command::new("go")
            .arg("version")
            .output()
            .await;
        match output {
            Ok(o) if o.status.success() => Ok(true),
            _ => Ok(false),
        }
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
        (
            "sh".to_string(),
            vec!["-c".to_string(), command.to_string()],
        )
    }
}
