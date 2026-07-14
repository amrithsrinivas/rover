use std::pin::Pin;

use rover_proto::v1::{
    self, app_service_client::AppServiceClient, auth_service_client::AuthServiceClient,
    server_service_client::ServerServiceClient,
};
use tokio_stream::Stream;
use tonic::{
    Request, Status,
    codegen::InterceptedService,
    metadata::{Ascii, MetadataValue},
    service::Interceptor,
    transport::Channel,
};

/// Wraps all gRPC client stubs and manages the API key for auth.
#[derive(Clone)]
pub struct RoverClient {
    pub(crate) auth: AuthServiceClient<InterceptedService<Channel, AuthInterceptor>>,
    pub(crate) server: ServerServiceClient<InterceptedService<Channel, AuthInterceptor>>,
    pub(crate) app: AppServiceClient<InterceptedService<Channel, AuthInterceptor>>,
    interceptor: AuthInterceptor,
}

impl std::fmt::Debug for RoverClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoverClient").finish_non_exhaustive()
    }
}

/// Interceptor that injects the API key into gRPC metadata.
#[derive(Clone)]
pub(crate) struct AuthInterceptor {
    api_key: Option<MetadataValue<Ascii>>,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        if let Some(ref key) = self.api_key {
            request.metadata_mut().insert("authorization", key.clone());
        }
        Ok(request)
    }
}

impl RoverClient {
    /// Connect to a gRPC server at the given address.
    pub async fn connect(address: &str) -> Result<Self, String> {
        let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
            .map_err(|e| format!("invalid address: {e}"))?
            .connect_timeout(std::time::Duration::from_secs(5))
            .connect()
            .await
            .map_err(|e| format!("connection failed: {e}"))?;

        let interceptor = AuthInterceptor { api_key: None };

        Ok(Self {
            auth: AuthServiceClient::with_interceptor(endpoint.clone(), interceptor.clone()),
            server: ServerServiceClient::with_interceptor(endpoint.clone(), interceptor.clone()),
            app: AppServiceClient::with_interceptor(endpoint, interceptor.clone()),
            interceptor,
        })
    }

    /// Set the API key for subsequent authenticated requests.
    pub fn set_api_key(&mut self, key: &str) {
        let bearer = format!("Bearer {key}");
        let value = bearer
            .parse::<MetadataValue<Ascii>>()
            .expect("api key should be valid ASCII");
        self.interceptor.api_key = Some(value);
    }

    /// Pair with the server using a one-time pairing token.
    pub async fn pair(&mut self, token: &str) -> Result<v1::PairResponse, String> {
        let req = Request::new(v1::PairRequest {
            pairing_token: token.to_string(),
        });
        let resp = self
            .auth
            .pair(req)
            .await
            .map_err(|e| format!("pairing failed: {e}"))?
            .into_inner();
        self.set_api_key(&resp.api_key);
        Ok(resp)
    }

    // ── ServerService ────────────────────────────────────────────────────

    pub async fn get_info(&mut self) -> Result<v1::ServerInfo, String> {
        let req = Request::new(v1::GetInfoRequest {});
        let resp = self
            .server
            .get_info(req)
            .await
            .map_err(|e| format!("get_info failed: {e}"))?
            .into_inner();
        Ok(resp)
    }

    pub async fn get_metrics(&mut self) -> Result<v1::ServerMetrics, String> {
        let req = Request::new(v1::GetMetricsRequest {});
        let resp = self
            .server
            .get_metrics(req)
            .await
            .map_err(|e| format!("get_metrics failed: {e}"))?
            .into_inner();
        Ok(resp)
    }

    pub async fn list_apps(
        &mut self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<v1::AppSummary>, String> {
        let req = Request::new(v1::AppListRequest {
            page: Some(v1::PageRequest { limit, offset }),
        });
        let resp = self
            .server
            .list_apps(req)
            .await
            .map_err(|e| format!("list_apps failed: {e}"))?
            .into_inner();
        Ok(resp.apps)
    }

    // ── AppService ───────────────────────────────────────────────────────

    pub async fn get_app(&mut self, app_id: &str) -> Result<v1::AppDetailResponse, String> {
        let req = Request::new(v1::AppRequest {
            app_id: app_id.to_string(),
        });
        let resp = self
            .app
            .get_app(req)
            .await
            .map_err(|e| format!("get_app failed: {e}"))?
            .into_inner();
        Ok(resp)
    }

    pub async fn start_app(&mut self, app_id: &str) -> Result<v1::AppDetailResponse, String> {
        let req = Request::new(v1::AppRequest {
            app_id: app_id.to_string(),
        });
        let resp = self
            .app
            .start_app(req)
            .await
            .map_err(|e| format!("start_app failed: {e}"))?
            .into_inner();
        Ok(resp)
    }

    pub async fn stop_app(&mut self, app_id: &str) -> Result<v1::AppDetailResponse, String> {
        let req = Request::new(v1::AppRequest {
            app_id: app_id.to_string(),
        });
        let resp = self
            .app
            .stop_app(req)
            .await
            .map_err(|e| format!("stop_app failed: {e}"))?
            .into_inner();
        Ok(resp)
    }

    pub async fn restart_app(&mut self, app_id: &str) -> Result<v1::AppDetailResponse, String> {
        let req = Request::new(v1::AppRequest {
            app_id: app_id.to_string(),
        });
        let resp = self
            .app
            .restart_app(req)
            .await
            .map_err(|e| format!("restart_app failed: {e}"))?
            .into_inner();
        Ok(resp)
    }

    pub async fn delete_app(&mut self, app_id: &str) -> Result<(), String> {
        let req = Request::new(v1::AppRequest {
            app_id: app_id.to_string(),
        });
        self.app
            .delete_app(req)
            .await
            .map_err(|e| format!("delete_app failed: {e}"))?;
        Ok(())
    }

    pub async fn stream_logs(
        &mut self,
        app_id: &str,
        tail_lines: i32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<v1::LogEntry, Status>> + Send>>, String> {
        let req = Request::new(v1::LogStreamRequest {
            app_id: app_id.to_string(),
            tail_lines,
        });
        let resp = self
            .app
            .stream_logs(req)
            .await
            .map_err(|e| format!("stream_logs failed: {e}"))?
            .into_inner();
        Ok(Box::pin(resp))
    }

    pub async fn deploy_stream(
        &mut self,
        req: v1::DeployRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<v1::DeployEvent, Status>> + Send>>, String> {
        let request = Request::new(req);
        let resp = self
            .app
            .deploy(request)
            .await
            .map_err(|e| format!("deploy failed: {e}"))?
            .into_inner();
        Ok(Box::pin(resp))
    }

    pub async fn set_env(
        &mut self,
        app_id: &str,
        env_vars: std::collections::HashMap<String, String>,
    ) -> Result<v1::AppDetailResponse, String> {
        let req = Request::new(v1::SetEnvRequest {
            app_id: app_id.to_string(),
            env_vars,
        });
        let resp = self
            .app
            .set_env(req)
            .await
            .map_err(|e| format!("set_env failed: {e}"))?
            .into_inner();
        Ok(resp)
    }

    pub async fn update_app(
        &mut self,
        app_id: &str,
        build_command: Option<String>,
        run_command: Option<String>,
    ) -> Result<v1::AppDetailResponse, String> {
        let req = Request::new(v1::UpdateAppRequest {
            app_id: app_id.to_string(),
            build_command,
            run_command,
        });
        let resp = self
            .app
            .update_app(req)
            .await
            .map_err(|e| format!("update_app failed: {e}"))?
            .into_inner();
        Ok(resp)
    }
}
