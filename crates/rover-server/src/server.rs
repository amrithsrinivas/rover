use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::service::Interceptor;
use tonic::{Request, Response, Status, Streaming};

use rover_proto::v1::{
    self,
    app_service_server::{AppService, AppServiceServer},
    auth_service_server::{AuthService, AuthServiceServer},
    server_service_server::{ServerService, ServerServiceServer},
};

use crate::auth::AuthManager;
use crate::deploy::Deployer;
use crate::metrics::collect_metrics;
use crate::process::ProcessManager;
use crate::state::{AppRow, StateStore};
use rover_core::AppManifest;

// ----------------------------------------------------------------------
// Shared server state
// ----------------------------------------------------------------------

#[derive(Clone)]
pub struct RoverServer {
    pub store: Arc<StateStore>,
    pub auth: Arc<AuthManager>,
    pub deployer: Arc<Deployer>,
    pub process_manager: ProcessManager,
    pub start_time: std::time::Instant,
    pub data_dir: PathBuf,
}

// ----------------------------------------------------------------------
// Start the gRPC server
// ----------------------------------------------------------------------

pub async fn start(
    port: u16,
    store: Arc<StateStore>,
    auth: AuthManager,
    deployer: Deployer,
    process_manager: ProcessManager,
    data_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let auth = Arc::new(auth);

    let rover = RoverServer {
        store,
        auth: auth.clone(),
        deployer: Arc::new(deployer),
        process_manager,
        start_time: std::time::Instant::now(),
        data_dir: data_dir.to_path_buf(),
    };

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    if let Ok(ips) = local_ips() {
        for ip in &ips {
            tracing::info!("  -> Available at: http://{ip}:{port}");
        }
    }

    tracing::info!("gRPC server listening on {addr}");

    let auth_svc = AuthServiceServer::new(rover.clone());
    let server_svc = ServerServiceServer::new(rover.clone());
    let app_svc = AppServiceServer::new(rover.clone());

    let auth_intercept = tonic::service::interceptor(AuthInterceptor { auth });

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(rover_proto::v1::FILE_DESCRIPTOR_SET)
        .build_v1()
        .map_err(|e| anyhow::anyhow!("failed to build reflection: {e}"))?;

    tonic::transport::Server::builder()
        .layer(auth_intercept)
        .add_service(auth_svc)
        .add_service(server_svc)
        .add_service(app_svc)
        .add_service(reflection)
        .serve(addr)
        .await?;

    Ok(())
}

// ----------------------------------------------------------------------
// Auth interceptor — validates API key on all RPCs except AuthService::Pair
// ----------------------------------------------------------------------

#[derive(Clone)]
struct AuthInterceptor {
    auth: Arc<AuthManager>,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        // If there's no authorization header, let it through.
        // Individual handlers (or future middleware) can reject.
        // The Pair RPC sends no key and is handled by AuthService directly.
        let key = match req.metadata().get("authorization") {
            Some(v) => match v.to_str().ok().and_then(|s| s.strip_prefix("Bearer ")) {
                Some(k) => k.to_string(),
                None => return Err(Status::unauthenticated("invalid authorization format")),
            },
            None => return Ok(req), // No auth header — let it pass (Pair, or health checks)
        };

        match self.auth.verify_api_key(&key) {
            Ok(true) => Ok(req),
            Ok(false) => Err(Status::unauthenticated("invalid api key")),
            Err(_) => Err(Status::internal("auth error")),
        }
    }
}

// Remove unused function — we use tonic::service::interceptor directly in start()
// ----------------------------------------------------------------------
// AuthService impl
// ----------------------------------------------------------------------

#[tonic::async_trait]
impl AuthService for RoverServer {
    async fn pair(
        &self,
        request: Request<v1::PairRequest>,
    ) -> Result<Response<v1::PairResponse>, Status> {
        let req = request.into_inner();
        let token = req.pairing_token;

        match self.auth.pair(&token) {
            Ok(api_key) => Ok(Response::new(v1::PairResponse {
                api_key,
                server_name: hostname(),
                server_version: env!("CARGO_PKG_VERSION").to_string(),
            })),
            Err(e) => Err(Status::unauthenticated(e.to_string())),
        }
    }
}

// ----------------------------------------------------------------------
// ServerService impl
// ----------------------------------------------------------------------

#[tonic::async_trait]
impl ServerService for RoverServer {
    async fn get_info(
        &self,
        _request: Request<v1::GetInfoRequest>,
    ) -> Result<Response<v1::ServerInfo>, Status> {
        Ok(Response::new(v1::ServerInfo {
            name: hostname(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            hostname: hostname(),
            uptime_seconds: self.start_time.elapsed().as_secs() as u32,
        }))
    }

    async fn get_metrics(
        &self,
        _request: Request<v1::GetMetricsRequest>,
    ) -> Result<Response<v1::ServerMetrics>, Status> {
        Ok(Response::new(collect_metrics().await))
    }

    type StreamMetricsStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<v1::ServerMetrics, Status>> + Send>>;

    async fn stream_metrics(
        &self,
        _request: Request<v1::GetMetricsRequest>,
    ) -> Result<Response<Self::StreamMetricsStream>, Status> {
        let (tx, rx) = mpsc::channel(16);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if tx.send(Ok(collect_metrics().await)).await.is_err() {
                    break; // client disconnected
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_apps(
        &self,
        request: Request<v1::AppListRequest>,
    ) -> Result<Response<v1::AppListResponse>, Status> {
        let req = request.into_inner();
        let limit = req.page.as_ref().map_or(50, |p| p.limit as i64);
        let offset = req.page.as_ref().map_or(0, |p| p.offset as i64);

        let apps = self
            .store
            .list_apps(limit, offset)
            .map_err(|e| Status::internal(e.to_string()))?;

        let total = apps.len() as i32; // simple; could do COUNT query

        let summaries: Vec<v1::AppSummary> = apps.into_iter().map(|a| app_to_summary(&a)).collect();

        Ok(Response::new(v1::AppListResponse {
            apps: summaries,
            page: Some(v1::PageResponse {
                total,
                limit: limit as i32,
                offset: offset as i32,
            }),
        }))
    }

    type SystemShellStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<v1::ShellOutput, Status>> + Send>>;

    async fn system_shell(
        &self,
        request: Request<Streaming<v1::ShellInput>>,
    ) -> Result<Response<Self::SystemShellStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(64);

        // Use bash with PS1 showing working directory, with PTY-like env for color support
        let mut child = tokio::process::Command::new("bash")
            .arg("--norc")
            .arg("--noprofile")
            .env("PS1", "\\[\\033[01;32m\\]\\w\\[\\033[00m\\]\\$ ")
            .env("TERM", "xterm-256color")
            .env("CLICOLOR", "1")
            .env("LS_COLORS", "di=34:ln=36:ex=32")
            .current_dir(&self.data_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Status::internal(format!("failed to spawn shell: {e}")))?;

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            while let Some(Ok(input)) = in_stream.next().await {
                if input.data.is_empty() {
                    continue;
                }
                if stdin.write_all(&input.data).await.is_err() {
                    break;
                }
            }
        });

        let tx_out = tx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_out
                            .send(Ok(v1::ShellOutput {
                                data: buf[..n].to_vec(),
                            }))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let tx_err = tx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_err
                            .send(Ok(v1::ShellOutput {
                                data: buf[..n].to_vec(),
                            }))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

// ----------------------------------------------------------------------
// AppService impl
// ----------------------------------------------------------------------

type DeployStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<v1::DeployEvent, Status>> + Send>>;

#[tonic::async_trait]
impl AppService for RoverServer {
    type DeployStream = DeployStream;

    async fn deploy(
        &self,
        request: Request<v1::DeployRequest>,
    ) -> Result<Response<Self::DeployStream>, Status> {
        let req = request.into_inner();

        let manifest = AppManifest::from_toml(&req.manifest_toml)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let event_rx = self
            .deployer
            .deploy(
                &manifest,
                req.source_archive,
                req.github_url,
                req.github_token,
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stream = tokio_stream::wrappers::ReceiverStream::new(event_rx).map(|event| {
            Ok(match event {
                crate::deploy::DeployEvent::Log { line, is_stderr } => v1::DeployEvent {
                    event: Some(v1::deploy_event::Event::Log(v1::DeployLogLine {
                        line,
                        is_stderr,
                    })),
                },
                crate::deploy::DeployEvent::Complete { app_id } => v1::DeployEvent {
                    event: Some(v1::deploy_event::Event::Complete(v1::DeployComplete {
                        app_id,
                    })),
                },
                crate::deploy::DeployEvent::Error(msg) => v1::DeployEvent {
                    event: Some(v1::deploy_event::Event::Error(v1::DeployError {
                        message: msg,
                    })),
                },
            })
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_app(
        &self,
        request: Request<v1::AppRequest>,
    ) -> Result<Response<v1::AppDetailResponse>, Status> {
        let app_id = request.into_inner().app_id;
        let app = self
            .store
            .get_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("app {app_id} not found")))?;

        Ok(Response::new(app_to_detail(&app, &self.store)?))
    }

    async fn start_app(
        &self,
        request: Request<v1::AppRequest>,
    ) -> Result<Response<v1::AppDetailResponse>, Status> {
        let app_id = request.into_inner().app_id;
        let app = self
            .store
            .get_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("app {app_id} not found")))?;

        if app.status == "running" {
            return Ok(Response::new(app_to_detail(&app, &self.store)?));
        }

        let env_vars: std::collections::HashMap<_, _> = self
            .store
            .get_env_vars(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .into_iter()
            .map(|v| (v.key, v.value))
            .collect();

        let (program, args) = crate::process::parse_shell_command(&app.run_command);
        let source_dir = std::path::PathBuf::from(&app.source_dir);

        // start_app handler
        self.process_manager
            .spawn(&app_id, &program, &args, &env_vars, &source_dir)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        self.store
            .update_app_status(&app_id, "running")
            .map_err(|e| Status::internal(e.to_string()))?;

        let updated = self
            .store
            .get_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .unwrap();
        Ok(Response::new(app_to_detail(&updated, &self.store)?))
    }

    async fn stop_app(
        &self,
        request: Request<v1::AppRequest>,
    ) -> Result<Response<v1::AppDetailResponse>, Status> {
        let app_id = request.into_inner().app_id;
        let _app = self
            .store
            .get_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("app {app_id} not found")))?;

        self.process_manager
            .stop(&app_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let updated = self
            .store
            .get_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .unwrap();
        Ok(Response::new(app_to_detail(&updated, &self.store)?))
    }

    async fn restart_app(
        &self,
        request: Request<v1::AppRequest>,
    ) -> Result<Response<v1::AppDetailResponse>, Status> {
        let app_id = request.into_inner().app_id;
        self.process_manager
            .stop(&app_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let app = self
            .store
            .get_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("app {app_id} not found")))?;

        let env_vars: std::collections::HashMap<_, _> = self
            .store
            .get_env_vars(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .into_iter()
            .map(|v| (v.key, v.value))
            .collect();

        let (program, args) = crate::process::parse_shell_command(&app.run_command);
        let source_dir = std::path::PathBuf::from(&app.source_dir);

        // restart_app handler
        self.process_manager
            .spawn(&app_id, &program, &args, &env_vars, &source_dir)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        self.store
            .update_app_status(&app_id, "running")
            .map_err(|e| Status::internal(e.to_string()))?;

        let updated = self
            .store
            .get_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .unwrap();
        Ok(Response::new(app_to_detail(&updated, &self.store)?))
    }

    async fn delete_app(
        &self,
        request: Request<v1::AppRequest>,
    ) -> Result<Response<v1::Empty>, Status> {
        let app_id = request.into_inner().app_id;
        self.process_manager.stop(&app_id).await.ok();
        self.store
            .delete_app(&app_id)
            .map_err(|e| Status::internal(e.to_string()))?;
        // Clean up on-disk source files
        if let Err(e) = self.deployer.cleanup_app_dir(&app_id) {
            tracing::warn!(app_id=%app_id, error=%e, "failed to clean up app directory");
        }
        Ok(Response::new(v1::Empty {}))
    }

    async fn update_app(
        &self,
        request: Request<v1::UpdateAppRequest>,
    ) -> Result<Response<v1::AppDetailResponse>, Status> {
        let req = request.into_inner();
        let app_id = &req.app_id;

        // Verify app exists
        let _ = self
            .store
            .get_app(app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("app {app_id} not found")))?;

        // Update commands in DB
        self.store
            .update_app_commands(
                app_id,
                req.build_command.as_deref(),
                req.run_command.as_deref(),
            )
            .map_err(|e| Status::internal(e.to_string()))?;

        // Return fresh detail
        let updated = self
            .store
            .get_app(app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("app vanished during update"))?;
        Ok(Response::new(app_to_detail(&updated, &self.store)?))
    }

    type StreamLogsStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<v1::LogEntry, Status>> + Send>>;

    async fn stream_logs(
        &self,
        request: Request<v1::LogStreamRequest>,
    ) -> Result<Response<Self::StreamLogsStream>, Status> {
        let req = request.into_inner();
        let app_id = req.app_id;
        let tail = req.tail_lines.max(1).min(1000) as i64;

        // Send recent log lines first
        let recent = self
            .store
            .get_logs(&app_id, tail)
            .map_err(|e| Status::internal(e.to_string()))?;

        let (tx, rx) = mpsc::channel(64);

        for log in recent {
            let _ = tx
                .send(Ok(v1::LogEntry {
                    timestamp: Some(v1::Timestamp {
                        millis: log.timestamp,
                    }),
                    line: log.line,
                    is_stderr: log.is_stderr,
                }))
                .await;
        }

        // Always close after sending — client polls periodically for new lines
        drop(tx);

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn set_env(
        &self,
        request: Request<v1::SetEnvRequest>,
    ) -> Result<Response<v1::AppDetailResponse>, Status> {
        let req = request.into_inner();
        for (key, value) in &req.env_vars {
            self.store
                .set_env_var(&req.app_id, key, value, false)
                .map_err(|e| Status::internal(e.to_string()))?;
        }
        let app = self
            .store
            .get_app(&req.app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("app not found"))?;
        Ok(Response::new(app_to_detail(&app, &self.store)?))
    }

    async fn delete_env(
        &self,
        request: Request<v1::DeleteEnvRequest>,
    ) -> Result<Response<v1::AppDetailResponse>, Status> {
        let req = request.into_inner();
        for key in &req.keys {
            self.store
                .delete_env_var(&req.app_id, key)
                .map_err(|e| Status::internal(e.to_string()))?;
        }
        let app = self
            .store
            .get_app(&req.app_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("app not found"))?;
        Ok(Response::new(app_to_detail(&app, &self.store)?))
    }

    async fn set_secret(
        &self,
        request: Request<v1::SetSecretRequest>,
    ) -> Result<Response<v1::Empty>, Status> {
        let req = request.into_inner();
        self.store
            .set_env_var(&req.app_id, &req.key, &req.value, true)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(v1::Empty {}))
    }

    type ShellStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<v1::ShellOutput, Status>> + Send>>;

    async fn shell(
        &self,
        request: Request<Streaming<v1::ShellInput>>,
    ) -> Result<Response<Self::ShellStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(64);

        // Spawn a shell process
        let mut child = tokio::process::Command::new("sh")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Status::internal(format!("failed to spawn shell: {e}")))?;

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        // Forward stdin from the gRPC stream to the shell
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            while let Some(Ok(input)) = in_stream.next().await {
                if input.data.is_empty() {
                    continue;
                }
                if stdin.write_all(&input.data).await.is_err() {
                    break;
                }
            }
        });

        // Forward stdout to gRPC stream
        let tx_out = tx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_out
                            .send(Ok(v1::ShellOutput {
                                data: buf[..n].to_vec(),
                            }))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Forward stderr to gRPC stream
        let tx_err = tx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_err
                            .send(Ok(v1::ShellOutput {
                                data: buf[..n].to_vec(),
                            }))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Kill child when client disconnects
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

fn app_to_summary(app: &AppRow) -> v1::AppSummary {
    v1::AppSummary {
        app_id: app.app_id.clone(),
        name: app.name.clone(),
        runtime: runtime_to_proto(&app.runtime),
        status: status_to_proto(&app.status),
        created_at: Some(v1::Timestamp {
            millis: app.created_at,
        }),
        updated_at: Some(v1::Timestamp {
            millis: app.updated_at,
        }),
    }
}

fn app_to_detail(app: &AppRow, store: &StateStore) -> Result<v1::AppDetailResponse, Status> {
    let env_vars = store
        .get_env_vars(&app.app_id)
        .map_err(|e| Status::internal(e.to_string()))?
        .into_iter()
        .map(|v| (v.key, v.value))
        .collect();

    Ok(v1::AppDetailResponse {
        app_id: app.app_id.clone(),
        name: app.name.clone(),
        runtime: runtime_to_proto(&app.runtime),
        status: status_to_proto(&app.status),
        build_command: app.build_command.clone(),
        run_command: app.run_command.clone(),
        env_vars,
        created_at: Some(v1::Timestamp {
            millis: app.created_at,
        }),
        updated_at: Some(v1::Timestamp {
            millis: app.updated_at,
        }),
        restart_count: app.restart_count as i32,
        pid: app.pid.map(|p| p as i32),
    })
}

fn runtime_to_proto(s: &str) -> i32 {
    match s {
        "python" => 1,
        "node" => 2,
        "go" => 3,
        "rust" => 4,
        _ => 0,
    }
}

fn status_to_proto(s: &str) -> i32 {
    match s {
        "deploying" => 1,
        "starting" => 2,
        "running" => 3,
        "stopped" => 4,
        "crashed" => 5,
        "failed" => 6,
        _ => 0,
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn local_ips() -> anyhow::Result<Vec<String>> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("1.1.1.1:80")?;
    let addr = socket.local_addr()?;
    Ok(vec![addr.ip().to_string()])
}
