use std::net::SocketAddr;
use rover_transport::{TransportClient, TransportError, TransportServer};

/// Stub transport for future relay-based connectivity.
/// All methods return `TransportError::NotImplemented`.
pub struct RelayTransport;

impl RelayTransport {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RelayTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TransportServer for RelayTransport {
    async fn bind(&self, _port: u16) -> Result<SocketAddr, TransportError> {
        Err(TransportError::NotImplemented(
            "relay transport not implemented yet".into(),
        ))
    }

    async fn serve(
        &self,
        _router: tonic::transport::server::Router,
    ) -> Result<(), TransportError> {
        Err(TransportError::NotImplemented(
            "relay transport not implemented yet".into(),
        ))
    }

    fn local_addresses(&self) -> Vec<SocketAddr> {
        vec![]
    }

    async fn shutdown(&self) {}
}

pub struct RelayTransportClient;

impl RelayTransportClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RelayTransportClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TransportClient for RelayTransportClient {
    async fn connect(
        &self,
        _address: &str,
    ) -> Result<tonic::transport::Channel, TransportError> {
        Err(TransportError::NotImplemented(
            "relay transport not implemented yet".into(),
        ))
    }
}
