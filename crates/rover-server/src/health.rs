use crate::process::ProcessManager;
use crate::state::StateStore;
use std::sync::Arc;

/// Periodically checks that running apps are still alive.
pub struct HealthChecker {
    store: Arc<StateStore>,
    process_manager: ProcessManager,
}

impl HealthChecker {
    pub fn new(store: Arc<StateStore>, process_manager: ProcessManager) -> Self {
        Self {
            store,
            process_manager,
        }
    }

    /// Run the health check loop. Checks all running apps every 30 seconds.
    pub async fn run(&self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            self.check_all().await;
        }
    }

    /// Check all apps with status "running" — if the process is dead, mark as crashed.
    async fn check_all(&self) {
        let Ok(apps) = self.store.list_apps(1000, 0) else {
            tracing::error!("health check: failed to list apps");
            return;
        };

        for app in &apps {
            if app.status == "running" {
                if !self.process_manager.is_alive(&app.app_id) {
                    tracing::warn!(
                        app_id = %app.app_id,
                        app_name = %app.name,
                        "health check: app process is dead"
                    );
                    let _ = self.store.update_app_status(&app.app_id, "crashed");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_health_check_detects_dead_process() {
        let dir = TempDir::new().unwrap();
        let store = StateStore::open(&dir.path().join("db")).unwrap();
        store
            .insert_app(
                "h",
                "health-test",
                "python",
                "service",
                "running",
                "b",
                "r",
                "/h",
                "m",
            )
            .unwrap();

        let pm = ProcessManager::new(store.clone());
        // Simulate a process that died without the exit watcher updating status
        // (e.g., killed externally). We set status to "running" then the health
        // checker should detect it's dead and mark as "crashed".
        store.update_app_status("h", "running").unwrap();

        let checker = HealthChecker::new(store.clone(), pm);
        checker.check_all().await;

        let app = store.get_app("h").unwrap().unwrap();
        assert_eq!(app.status, "crashed");
    }

    #[tokio::test]
    async fn test_health_check_ignores_stopped() {
        let dir = TempDir::new().unwrap();
        let store = StateStore::open(&dir.path().join("db")).unwrap();
        store
            .insert_app(
                "s",
                "stopped-test",
                "python",
                "service",
                "stopped",
                "b",
                "r",
                "/s",
                "m",
            )
            .unwrap();

        let pm = ProcessManager::new(store.clone());
        let checker = HealthChecker::new(store.clone(), pm);
        // Should not panic and should not change status
        checker.check_all().await;

        let app = store.get_app("s").unwrap().unwrap();
        assert_eq!(app.status, "stopped");
    }
}
