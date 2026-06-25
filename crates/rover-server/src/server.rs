use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};

use rover_proto::v1::{
    self,
    app_service_server::{AppService, AppServiceServer},
    auth_service_server::{AuthService, AuthServiceServer},
    server_service_server::{ServerService, ServerServiceServer},
};

use crate::auth::AuthManager;
use crate::deploy::Deployer;
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
    let rover = RoverServer {
        store,
        auth: Arc::new(auth),
        deployer: Arc::new(deployer),
        process_manager,
        start_time: std::time::Instant::now(),
    };

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    // Discover LAN IPs for display
    if let Ok(ips) = local_ips() {
        for ip in &ips {
            tracing::info!("  -> Available at: http://{ip}:{port}");
        }
    }

    tracing::info!("gRPC server listening on {addr}");

    // Build tonic server with all services
    let auth_svc = AuthServiceServer::new(rover.clone());
    let server_svc = ServerServiceServer::new(rover.clone());
    let app_svc = AppServiceServer::new(rover.clone());

    tonic::transport::Server::builder()
        .add_service(auth_svc)
        .add_service(server_svc)
        .add_service(app_svc)
        .serve(addr)
        .await?;

    Ok(())
}

// ----------------------------------------------------------------------
// Auth interceptor (extract + verify API key from metadata)
// TODO: implement as tonic interceptor layer
// ----------------------------------------------------------------------

fn extract_api_key<T>(req: &Request<T>) -> Option<String> {
    req.metadata()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

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
        Ok(Response::new(collect_metrics()))
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
                if tx.send(Ok(collect_metrics())).await.is_err() {
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
            .deploy(&manifest, &req.source_archive)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stream = tokio_stream::wrappers::ReceiverStream::new(event_rx).map(|event| {
            Ok(match event {
                crate::deploy::DeployEvent::Log(line) => v1::DeployEvent {
                    event: Some(v1::deploy_event::Event::Log(v1::DeployLogLine {
                        line,
                        is_stderr: false,
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

        self.process_manager
            .spawn(
                &app_id,
                &program,
                &args,
                &env_vars,
                &source_dir,
                &app.app_type,
            )
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
        let app = self
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

        self.process_manager
            .spawn(
                &app_id,
                &program,
                &args,
                &env_vars,
                &source_dir,
                &app.app_type,
            )
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
        Ok(Response::new(v1::Empty {}))
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
        let follow = req.follow;

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

        if follow {
            // Poll for new log entries every second
            let store = self.store.clone();
            let app_id_clone = app_id.clone();
            let mut last_ts = chrono::Utc::now().timestamp_millis();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    match store.get_logs_since(&app_id_clone, last_ts) {
                        Ok(logs) => {
                            for log in logs {
                                last_ts = last_ts.max(log.timestamp);
                                if tx
                                    .send(Ok(v1::LogEntry {
                                        timestamp: Some(v1::Timestamp {
                                            millis: log.timestamp,
                                        }),
                                        line: log.line,
                                        is_stderr: log.is_stderr,
                                    }))
                                    .await
                                    .is_err()
                                {
                                    return; // client disconnected
                                }
                            }
                        }
                        Err(_) => return,
                    }
                }
            });
        }

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
        _request: Request<Streaming<v1::ShellInput>>,
    ) -> Result<Response<Self::ShellStream>, Status> {
        // Shell implementation in Phase 3
        Err(Status::unimplemented("shell not implemented yet"))
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
        app_type: app_type_to_proto(&app.app_type),
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
        app_type: app_type_to_proto(&app.app_type),
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

fn app_type_to_proto(s: &str) -> i32 {
    match s {
        "service" => 1,
        "job" => 2,
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

fn collect_metrics() -> v1::ServerMetrics {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() as f64
        / sys.cpus().len().max(1) as f64;
    let ram_used = sys.used_memory();
    let ram_total = sys.total_memory();
    // Skip disk for simplicity (can add later)

    v1::ServerMetrics {
        cpu_percent: cpu,
        ram_used_bytes: ram_used,
        ram_total_bytes: ram_total,
        disk_used_bytes: 0,
        disk_total_bytes: 0,
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
