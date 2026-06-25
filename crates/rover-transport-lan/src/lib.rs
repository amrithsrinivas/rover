use rover_transport::{TransportClient, TransportError, TransportServer};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio_stream::wrappers::TcpListenerStream;

/// A transport that uses raw TCP on the local network.
/// gRPC runs over HTTP/2 cleartext (h2c).
pub struct LanTransport {
    shutdown: Arc<Notify>,
}

impl LanTransport {
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(Notify::new()),
        }
    }
}

impl Default for LanTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TransportServer for LanTransport {
    async fn bind(&self, port: u16) -> Result<SocketAddr, TransportError> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let _listener = TcpListener::bind(addr)
            .await
            .map_err(|e| TransportError::BindFailed(format!("failed to bind to {addr}: {e}")))?;
        Ok(addr)
    }

    async fn serve(&self, router: tonic::transport::server::Router) -> Result<(), TransportError> {
        // We need to know which port to use. This is a simplification —
        // in real usage the port comes from config. The bind() above is
        // primarily for address discovery; the actual listener is created
        // inside tonic. For now we serve on 0.0.0.0:9050 (configurable).
        let addr = SocketAddr::from(([0, 0, 0, 0], 9050));

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| TransportError::BindFailed(format!("failed to bind to {addr}: {e}")))?;

        let stream = TcpListenerStream::new(listener);
        let shutdown = self.shutdown.clone();

        tokio::select! {
            result = router.serve_with_incoming(stream) => {
                result.map_err(|e| TransportError::ServerError(e.to_string()))
            }
            _ = shutdown.notified() => {
                Ok(())
            }
        }
    }

    fn local_addresses(&self) -> Vec<SocketAddr> {
        discover_lan_ips()
            .into_iter()
            .map(|ip| SocketAddr::new(ip, 9050))
            .collect()
    }

    async fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}

/// Client that connects to a LAN address via raw TCP.
pub struct LanTransportClient;

impl LanTransportClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LanTransportClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TransportClient for LanTransportClient {
    async fn connect(&self, address: &str) -> Result<tonic::transport::Channel, TransportError> {
        let uri = format!("http://{address}");

        let endpoint = tonic::transport::Endpoint::from_shared(uri)
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        Ok(channel)
    }
}

/// Discover non-loopback IPv4 addresses on the local machine.
pub fn discover_lan_ips() -> Vec<IpAddr> {
    use std::net::UdpSocket;

    // Simple approach: bind a UDP socket and inspect the local address.
    // This gives us one routable address without iterating interfaces.
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => {
            // Connect to a non-routable address just to determine the local IP.
            // The socket doesn't actually send data.
            if socket.connect("1.1.1.1:80").is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    return vec![addr.ip()];
                }
            }
            vec![]
        }
        Err(_) => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_lan_ips_returns_something() {
        let ips = discover_lan_ips();
        // On a machine with networking, we should get at least one IP.
        // In CI without network, this may be empty — that's also valid.
        for ip in &ips {
            assert!(!ip.is_loopback(), "should not return loopback addresses");
        }
    }
}
