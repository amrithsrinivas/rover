/// gRPC client for communicating with the Rover server.
pub struct RoverClient {
    // TODO: tonic channel + service clients
}

impl RoverClient {
    /// Create a new client and connect to the server address.
    pub async fn connect(address: &str) -> Result<Self, String> {
        let _ = address;
        Err("not implemented".to_string())
    }

    /// Exchange a pairing token for a persistent API key.
    pub async fn pair(&mut self, token: &str) -> Result<String, String> {
        let _ = token;
        Err("not implemented".to_string())
    }
}
