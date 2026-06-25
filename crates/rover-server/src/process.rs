use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing;

use crate::state::StateStore;

/// Tracks and manages running app processes.
pub struct ProcessManager {
    store: Arc<StateStore>,
    processes: Arc<Mutex<HashMap<String, ProcessHandle>>>,
}

struct ProcessHandle {
    pid: u32,
    restart_count: u32,
    /// true if this process should be restarted on crash
    is_service: bool,
    run_program: String,
    run_args: Vec<String>,
    working_dir: std::path::PathBuf,
    env_vars: HashMap<String, String>,
}

impl Clone for ProcessManager {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            processes: self.processes.clone(),
        }
    }
}

impl ProcessManager {
    pub fn new(store: Arc<StateStore>) -> Self {
        Self {
            store,
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn an app process. Returns the PID.
    pub async fn spawn(
        &self,
        app_id: &str,
        program: &str,
        args: &[String],
        env_vars: &HashMap<String, String>,
        working_dir: &Path,
        app_type: &str,
    ) -> anyhow::Result<u32> {
        self.stop(app_id).await.ok();

        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .current_dir(working_dir)
            .envs(env_vars)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn()?;
        let pid = child.id().expect("child should have a PID");
        tracing::info!(app_id = %app_id, pid = pid, "spawned");

        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        tokio::spawn(pipe_to_logs(
            app_id.to_string(),
            stdout,
            false,
            self.store.clone(),
        ));
        tokio::spawn(pipe_to_logs(
            app_id.to_string(),
            stderr,
            true,
            self.store.clone(),
        ));

        let is_service = app_type == "service";

        // Watch for exit in a spawned task
        let app_id_exit = app_id.to_string();
        let store_exit = self.store.clone();
        let processes_exit = self.processes.clone();
        let restart = RestartParams {
            run_program: program.to_string(),
            run_args: args.to_vec(),
            env_vars: env_vars.clone(),
            working_dir: working_dir.to_path_buf(),
        };

        tokio::spawn(async move {
            let status = child.wait().await.ok();
            let exit_ok = status.map_or(false, |s| s.success());

            let handle = processes_exit.lock().unwrap().remove(&app_id_exit);
            if handle.is_none() {
                return;
            }

            if is_service && !exit_ok {
                let rc = handle.unwrap().restart_count + 1;
                if rc > 5 {
                    let _ = store_exit.update_app_status(&app_id_exit, "crashed");
                    return;
                }
                let delay = 2u64.pow(rc).min(60);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;

                restart_loop(app_id_exit, restart, store_exit, processes_exit, rc).await;
            } else if !is_service {
                let status = if exit_ok { "stopped" } else { "failed" };
                let _ = store_exit.update_app_status(&app_id_exit, status);
            }
        });

        self.processes.lock().unwrap().insert(
            app_id.to_string(),
            ProcessHandle {
                pid,
                restart_count: 0,
                is_service,
                run_program: program.to_string(),
                run_args: args.to_vec(),
                working_dir: working_dir.to_path_buf(),
                env_vars: env_vars.clone(),
            },
        );
        self.store.update_app_pid(&app_id, pid)?;

        Ok(pid)
    }

    /// Stop a process.
    pub async fn stop(&self, app_id: &str) -> anyhow::Result<()> {
        let pid = self.processes.lock().unwrap().get(app_id).map(|h| h.pid);
        if let Some(pid) = pid {
            send_signal(pid, libc::SIGTERM);
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
            while check_pid_alive(pid) && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            if check_pid_alive(pid) {
                send_signal(pid, libc::SIGKILL);
            }
            self.processes.lock().unwrap().remove(app_id);
            self.store.update_app_status(app_id, "stopped")?;
        }
        Ok(())
    }

    pub fn is_alive(&self, app_id: &str) -> bool {
        self.processes
            .lock()
            .unwrap()
            .get(app_id)
            .map_or(false, |h| check_pid_alive(h.pid))
    }

    pub fn list_pids(&self) -> HashMap<String, u32> {
        self.processes
            .lock()
            .unwrap()
            .iter()
            .map(|(id, h)| (id.clone(), h.pid))
            .collect()
    }
}

// ----------------------------------------------------------------------
// Restart (free async function, not on ProcessManager)
// ----------------------------------------------------------------------

struct RestartParams {
    run_program: String,
    run_args: Vec<String>,
    env_vars: HashMap<String, String>,
    working_dir: std::path::PathBuf,
}

/// Spawns a new child, watches it, and restarts on crash (loop until success or crash limit).
async fn restart_loop(
    app_id: String,
    params: RestartParams,
    store: Arc<StateStore>,
    processes: Arc<Mutex<HashMap<String, ProcessHandle>>>,
    restart_count: u32,
) {
    let mut cmd = tokio::process::Command::new(&params.run_program);
    cmd.args(&params.run_args)
        .current_dir(&params.working_dir)
        .envs(&params.env_vars)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(app_id = %app_id, error = %e, "restart spawn failed");
            let _ = store.update_app_status(&app_id, "crashed");
            return;
        }
    };

    let pid = child.id().expect("child should have a PID");
    store.update_app_pid(&app_id, pid).ok();

    let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
    tokio::spawn(pipe_to_logs(app_id.clone(), stdout, false, store.clone()));
    tokio::spawn(pipe_to_logs(app_id.clone(), stderr, true, store.clone()));

    processes.lock().unwrap().insert(
        app_id.clone(),
        ProcessHandle {
            pid,
            restart_count,
            is_service: true,
            run_program: params.run_program.clone(),
            run_args: params.run_args.clone(),
            working_dir: params.working_dir.clone(),
            env_vars: params.env_vars.clone(),
        },
    );

    // Wait for exit and decide whether to restart again
    let status = child.wait().await.ok();
    let exit_ok = status.map_or(false, |s| s.success());

    processes.lock().unwrap().remove(&app_id);

    if !exit_ok {
        let rc = restart_count + 1;
        if rc > 5 {
            let _ = store.update_app_status(&app_id, "crashed");
            return;
        }
        let delay = 2u64.pow(rc).min(60);
        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
        Box::pin(restart_loop(app_id, params, store, processes, rc)).await;
    }
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

async fn pipe_to_logs(
    app_id: String,
    reader: Option<impl tokio::io::AsyncRead + Unpin + Send + 'static>,
    is_stderr: bool,
    store: Arc<StateStore>,
) {
    let reader = match reader {
        Some(r) => r,
        None => return,
    };
    use tokio::io::AsyncBufReadExt;
    let buf = tokio::io::BufReader::new(reader);
    let mut lines = buf.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let now = chrono::Utc::now().timestamp_millis();
        tracing::debug!(app_id=%app_id, line=%line, is_stderr=is_stderr, "log line captured");
        if let Err(e) = store.insert_log(&app_id, now, &line, is_stderr) {
            tracing::error!(app_id=%app_id, error=%e, "failed to write log");
        }
    }
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: i32) {
    unsafe {
        libc::kill(pid as i32, signal);
    }
}

#[cfg(not(unix))]
fn send_signal(_pid: u32, _signal: i32) {}

#[cfg(unix)]
fn check_pid_alive(pid: u32) -> bool {
    // kill(pid, 0) checks if process exists — works on macOS and Linux
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn check_pid_alive(_pid: u32) -> bool {
    true
}

pub fn parse_shell_command(command: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), vec![]);
    }
    (
        parts[0].to_string(),
        parts[1..].iter().map(|s| s.to_string()).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_shell_command() {
        let (p, a) = parse_shell_command("python main.py --flag");
        assert_eq!(p, "python");
        assert_eq!(a, vec!["main.py", "--flag"]);
    }

    #[test]
    fn test_parse_empty() {
        let (p, a) = parse_shell_command("");
        assert_eq!(p, "");
        assert!(a.is_empty());
    }

    #[tokio::test]
    async fn test_spawn_and_stop() {
        let dir = TempDir::new().unwrap();
        let store = StateStore::open(&dir.path().join("db")).unwrap();
        store
            .insert_app(
                "t", "t", "python", "service", "stopped", "b", "r", "/t", "m",
            )
            .unwrap();
        let pm = ProcessManager::new(store);
        let pid = pm
            .spawn(
                "t",
                "sleep",
                &["10".into()],
                &HashMap::new(),
                dir.path(),
                "service",
            )
            .await
            .unwrap();
        assert!(pid > 0);
        assert!(pm.is_alive("t"));
        pm.stop("t").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        assert!(!pm.is_alive("t"));
    }

    #[tokio::test]
    async fn test_job_exits() {
        let dir = TempDir::new().unwrap();
        let store = StateStore::open(&dir.path().join("db")).unwrap();
        store
            .insert_app("j", "j", "python", "job", "running", "b", "r", "/j", "m")
            .unwrap();
        let pm = ProcessManager::new(store);
        pm.spawn(
            "j",
            "echo",
            &["hi".into()],
            &HashMap::new(),
            dir.path(),
            "job",
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        assert!(!pm.is_alive("j"));
    }
}
