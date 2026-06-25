use rover_proto::v1::{
    AppDetailResponse, AppListResponse, AppRequest, AppSummary, DeleteEnvRequest, DeployEvent,
    DeployRequest, LogEntry, LogStreamRequest, PairResponse, ServerInfo, ServerMetrics,
    SetEnvRequest, SetSecretRequest, app_service_client::AppServiceClient,
    auth_service_client::AuthServiceClient, server_service_client::ServerServiceClient,
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::Request;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;

/// Wraps the gRPC client connections for all Rover services.
pub struct RoverClient {
    pub channel: Channel,
    pub auth: AuthServiceClient<Channel>,
    pub server: ServerServiceClient<Channel>,
    pub app: AppServiceClient<Channel>,
    pub api_key: Option<String>,
}

impl std::fmt::Debug for RoverClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoverClient")
            .field("has_api_key", &self.api_key.is_some())
            .finish()
    }
}

impl RoverClient {
    /// Connect to a server at the given address.
    pub async fn connect(address: &str) -> Result<Self, String> {
        let uri = format!("http://{address}");
        let channel = Channel::from_shared(uri)
            .map_err(|e| format!("invalid address: {e}"))?
            .connect()
            .await
            .map_err(|e| format!("connection failed: {e}"))?;

        Ok(Self {
            auth: AuthServiceClient::new(channel.clone()),
            server: ServerServiceClient::new(channel.clone()),
            app: AppServiceClient::new(channel.clone()),
            channel,
            api_key: None,
        })
    }

    /// Exchange a pairing token for a persistent API key.
    pub async fn pair(&mut self, token: &str) -> Result<PairResponse, String> {
        let req = Request::new(rover_proto::v1::PairRequest {
            pairing_token: token.to_string(),
        });
        let resp = self
            .auth
            .pair(req)
            .await
            .map_err(|e| format!("pair failed: {e}"))?
            .into_inner();
        self.api_key = Some(resp.api_key.clone());
        Ok(resp)
    }

    /// Set the API key without going through the pairing flow.
    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = Some(key.to_string());
    }

    /// Add the authorization header to a request.
    fn auth_req<T>(&self, req: &mut Request<T>) -> Result<(), String> {
        let key = self.api_key.as_ref().ok_or("not authenticated")?;
        let value = MetadataValue::try_from(format!("Bearer {key}"))
            .map_err(|e| format!("invalid api key: {e}"))?;
        req.metadata_mut().insert("authorization", value);
        Ok(())
    }

    // --- Server Service ---

    pub async fn get_info(&mut self) -> Result<ServerInfo, String> {
        let mut req = Request::new(rover_proto::v1::GetInfoRequest {});
        self.auth_req(&mut req)?;
        Ok(self
            .server
            .get_info(req)
            .await
            .map_err(|e| format!("get_info: {e}"))?
            .into_inner())
    }

    pub async fn get_metrics(&mut self) -> Result<ServerMetrics, String> {
        let mut req = Request::new(rover_proto::v1::GetMetricsRequest {});
        self.auth_req(&mut req)?;
        Ok(self
            .server
            .get_metrics(req)
            .await
            .map_err(|e| format!("get_metrics: {e}"))?
            .into_inner())
    }

    pub async fn list_apps(&mut self) -> Result<AppListResponse, String> {
        let mut req = Request::new(rover_proto::v1::AppListRequest {
            page: Some(rover_proto::v1::PageRequest {
                limit: 50,
                offset: 0,
            }),
        });
        self.auth_req(&mut req)?;
        Ok(self
            .server
            .list_apps(req)
            .await
            .map_err(|e| format!("list_apps: {e}"))?
            .into_inner())
    }

    // --- App Service ---

    pub async fn get_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String> {
        let mut req = Request::new(AppRequest {
            app_id: app_id.to_string(),
        });
        self.auth_req(&mut req)?;
        Ok(self
            .app
            .get_app(req)
            .await
            .map_err(|e| format!("get_app: {e}"))?
            .into_inner())
    }

    pub async fn start_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String> {
        let mut req = Request::new(AppRequest {
            app_id: app_id.to_string(),
        });
        self.auth_req(&mut req)?;
        Ok(self
            .app
            .start_app(req)
            .await
            .map_err(|e| format!("start_app: {e}"))?
            .into_inner())
    }

    pub async fn stop_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String> {
        let mut req = Request::new(AppRequest {
            app_id: app_id.to_string(),
        });
        self.auth_req(&mut req)?;
        Ok(self
            .app
            .stop_app(req)
            .await
            .map_err(|e| format!("stop_app: {e}"))?
            .into_inner())
    }

    pub async fn restart_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String> {
        let mut req = Request::new(AppRequest {
            app_id: app_id.to_string(),
        });
        self.auth_req(&mut req)?;
        Ok(self
            .app
            .restart_app(req)
            .await
            .map_err(|e| format!("restart_app: {e}"))?
            .into_inner())
    }

    pub async fn delete_app(&mut self, app_id: &str) -> Result<(), String> {
        let mut req = Request::new(AppRequest {
            app_id: app_id.to_string(),
        });
        self.auth_req(&mut req)?;
        self.app
            .delete_app(req)
            .await
            .map_err(|e| format!("delete_app: {e}"))?;
        Ok(())
    }

    pub async fn set_env(
        &mut self,
        app_id: &str,
        vars: HashMap<String, String>,
    ) -> Result<AppDetailResponse, String> {
        let mut req = Request::new(SetEnvRequest {
            app_id: app_id.to_string(),
            env_vars: vars,
        });
        self.auth_req(&mut req)?;
        Ok(self
            .app
            .set_env(req)
            .await
            .map_err(|e| format!("set_env: {e}"))?
            .into_inner())
    }

    pub async fn delete_env(
        &mut self,
        app_id: &str,
        keys: Vec<String>,
    ) -> Result<AppDetailResponse, String> {
        let mut req = Request::new(DeleteEnvRequest {
            app_id: app_id.to_string(),
            keys,
        });
        self.auth_req(&mut req)?;
        Ok(self
            .app
            .delete_env(req)
            .await
            .map_err(|e| format!("delete_env: {e}"))?
            .into_inner())
    }

    pub async fn set_secret(&mut self, app_id: &str, key: &str, value: &str) -> Result<(), String> {
        let mut req = Request::new(SetSecretRequest {
            app_id: app_id.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        });
        self.auth_req(&mut req)?;
        self.app
            .set_secret(req)
            .await
            .map_err(|e| format!("set_secret: {e}"))?;
        Ok(())
    }

    // --- Streaming ---

    /// Deploy an app and stream build events.
    /// Returns a channel receiver of DeployEvents.
    pub async fn deploy_stream(
        &mut self,
        name: String,
        runtime: i32,
        manifest_toml: String,
        source_archive: Vec<u8>,
    ) -> Result<mpsc::Receiver<Result<DeployEvent, String>>, String> {
        let mut req = Request::new(DeployRequest {
            name,
            runtime,
            manifest_toml,
            source_archive,
        });
        self.auth_req(&mut req)?;

        let mut stream = self
            .app
            .deploy(req)
            .await
            .map_err(|e| format!("deploy: {e}"))?
            .into_inner();

        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            while let Some(event) = stream.next().await {
                match event {
                    Ok(e) => {
                        let _ = tx.send(Ok(e)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("stream error: {e}"))).await;
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Stream log entries for an app.
    pub async fn stream_logs(
        &mut self,
        app_id: &str,
        follow: bool,
        tail_lines: i32,
    ) -> Result<mpsc::Receiver<Result<LogEntry, String>>, String> {
        let mut req = Request::new(LogStreamRequest {
            app_id: app_id.to_string(),
            follow,
            tail_lines,
        });
        self.auth_req(&mut req)?;

        let mut stream = self
            .app
            .stream_logs(req)
            .await
            .map_err(|e| format!("stream_logs: {e}"))?
            .into_inner();

        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            while let Some(entry) = stream.next().await {
                match entry {
                    Ok(e) => {
                        let _ = tx.send(Ok(e)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("stream error: {e}"))).await;
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }
}
