use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Identifies the language runtime for a deployed app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Python,
    Node,
    Go,
    Rust,
}

impl Runtime {
    /// All known runtimes.
    pub fn all() -> &'static [Runtime] {
        &[Runtime::Python, Runtime::Node, Runtime::Go, Runtime::Rust]
    }

    /// Human-readable name for display.
    pub fn display_name(&self) -> &'static str {
        match self {
            Runtime::Python => "Python",
            Runtime::Node => "Node.js",
            Runtime::Go => "Go",
            Runtime::Rust => "Rust",
        }
    }

    /// Default build command for this runtime.
    pub fn default_build_command(&self) -> &'static str {
        match self {
            Runtime::Python => "pip install -r requirements.txt",
            Runtime::Node => "npm install",
            Runtime::Go => "go build -o app .",
            Runtime::Rust => "cargo build --release",
        }
    }

    /// Default run command for this runtime.
    pub fn default_run_command(&self) -> &'static str {
        match self {
            Runtime::Python => "python3 main.py",
            Runtime::Node => "node index.js",
            Runtime::Go => "./app",
            Runtime::Rust => "./target/release/app",
        }
    }
}

impl fmt::Display for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Runtime::Python => write!(f, "python"),
            Runtime::Node => write!(f, "node"),
            Runtime::Go => write!(f, "go"),
            Runtime::Rust => write!(f, "rust"),
        }
    }
}

impl FromStr for Runtime {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "python" | "python3" => Ok(Runtime::Python),
            "node" | "nodejs" | "node.js" => Ok(Runtime::Node),
            "go" | "golang" => Ok(Runtime::Go),
            "rust" | "cargo" => Ok(Runtime::Rust),
            other => Err(format!("unknown runtime: '{other}'")),
        }
    }
}

/// Represents the current state of a deployed app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppStatus {
    /// Build is in progress.
    Deploying,
    /// Process is starting up.
    Starting,
    /// Process is running and healthy.
    Running,
    /// Explicitly stopped by user.
    Stopped,
    /// Process exited unexpectedly.
    Crashed,
    /// Build or start failed.
    Failed,
}

impl AppStatus {
    /// Whether the app is in a terminal/non-transitional state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AppStatus::Running | AppStatus::Stopped | AppStatus::Crashed | AppStatus::Failed
        )
    }

    /// Display color hint for UIs.
    pub fn color(&self) -> &'static str {
        match self {
            AppStatus::Deploying | AppStatus::Starting => "#f0ad4e", // yellow
            AppStatus::Running => "#5cb85c",                         // green
            AppStatus::Stopped => "#777777",                         // gray
            AppStatus::Crashed | AppStatus::Failed => "#d9534f",     // red
        }
    }
}

impl fmt::Display for AppStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppStatus::Deploying => write!(f, "deploying"),
            AppStatus::Starting => write!(f, "starting"),
            AppStatus::Running => write!(f, "running"),
            AppStatus::Stopped => write!(f, "stopped"),
            AppStatus::Crashed => write!(f, "crashed"),
            AppStatus::Failed => write!(f, "failed"),
        }
    }
}

impl FromStr for AppStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "deploying" => Ok(AppStatus::Deploying),
            "starting" => Ok(AppStatus::Starting),
            "running" => Ok(AppStatus::Running),
            "stopped" => Ok(AppStatus::Stopped),
            "crashed" => Ok(AppStatus::Crashed),
            "failed" => Ok(AppStatus::Failed),
            other => Err(format!("unknown app status: '{other}'")),
        }
    }
}
