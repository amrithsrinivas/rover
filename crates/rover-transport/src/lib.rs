use std::net::SocketAddr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Bind failed: {0}")]
    BindFailed(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("{0}")]
    Other(String),
}

/// Server-side transport: binds to an address and serves gRPC.
#[async_trait::async_trait]
pub trait TransportServer: Send + Sync {
    /// Bind to the given port and start listening.
    async fn bind(&self, port: u16) -> Result<SocketAddr, TransportError>;

    /// Serve a tonic Router on this transport. Blocks until shutdown.
    async fn serve(&self, router: tonic::transport::server::Router) -> Result<(), TransportError>;

    /// Discover local addresses suitable for clients to connect to.
    fn local_addresses(&self) -> Vec<SocketAddr>;

    /// Initiate graceful shutdown.
    async fn shutdown(&self);
}

/// Client-side transport: connects to a server address.
#[async_trait::async_trait]
pub trait TransportClient: Send + Sync {
    /// Connect to the server at the given address.
    /// Returns a tonic Channel ready for gRPC calls.
    async fn connect(&self, address: &str) -> Result<tonic::transport::Channel, TransportError>;
}
