pub mod python;

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use rover_core::Runtime;

/// Implemented by each language runtime to handle build and run commands.
#[async_trait]
pub trait RuntimeHandler: Send + Sync {
    fn runtime(&self) -> Runtime;
    async fn check_installed(&self) -> Result<bool, rover_core::RoverError>;
    async fn build(&self, app_dir: &Path, command: &str) -> Result<(), rover_core::RoverError>;
    fn run_command(&self, app_dir: &Path, command: &str) -> (String, Vec<String>);
}

/// Registry of available runtime handlers. Clone is cheap (Arc'd data).
#[derive(Clone)]
pub struct RuntimeRegistry {
    handlers: Arc<RwLock<HashMap<Runtime, Arc<dyn RuntimeHandler>>>>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, handler: impl RuntimeHandler + 'static) {
        self.handlers
            .write()
            .unwrap()
            .insert(handler.runtime(), Arc::new(handler));
    }

    pub fn get_handler(&self, runtime: Runtime) -> Option<Arc<dyn RuntimeHandler>> {
        self.handlers.read().unwrap().get(&runtime).cloned()
    }

    pub fn registered(&self) -> Vec<Runtime> {
        self.handlers.read().unwrap().keys().copied().collect()
    }
}
