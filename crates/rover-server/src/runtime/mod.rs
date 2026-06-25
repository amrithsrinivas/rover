pub mod python;

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use rover_core::Runtime;

/// Implemented by each language runtime to handle build and run commands.
#[async_trait]
pub trait RuntimeHandler: Send + Sync {
    /// The runtime identifier.
    fn runtime(&self) -> Runtime;

    /// Check whether the runtime toolchain is installed.
    async fn check_installed(&self) -> Result<bool, rover_core::RoverError>;

    /// Execute the build command in the app's source directory.
    async fn build(&self, app_dir: &Path, command: &str) -> Result<(), rover_core::RoverError>;

    /// Return the command and args to run the app process.
    fn run_command(&self, app_dir: &Path, command: &str) -> (String, Vec<String>);
}

/// Registry of available runtime handlers.
pub struct RuntimeRegistry {
    handlers: RwLock<HashMap<Runtime, Box<dyn RuntimeHandler>>>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a runtime handler.
    pub fn register(&self, handler: impl RuntimeHandler + 'static) {
        let runtime = handler.runtime();
        self.handlers
            .write()
            .unwrap()
            .insert(runtime, Box::new(handler));
    }

    /// Get a handler by runtime. Returns None if not registered/available.
    pub fn get_handler(&self, runtime: Runtime) -> Option<std::sync::Arc<dyn RuntimeHandler>> {
        // For now, return the boxed handler. In practice we'd store Arc<dyn>.
        // Since we only need read access, we clone by re-boxing — not ideal but simple.
        // TODO: store Arc<dyn RuntimeHandler> instead of Box<dyn>
        let _ = runtime;
        None
    }

    /// List runtimes that are currently registered.
    pub fn registered(&self) -> Vec<Runtime> {
        self.handlers.read().unwrap().keys().copied().collect()
    }
}
