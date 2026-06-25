use crate::state::StateStore;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Manages pairing tokens and API key authentication.
pub struct AuthManager {
    store: Arc<StateStore>,
}

impl AuthManager {
    pub fn new(store: Arc<StateStore>) -> Self {
        Self { store }
    }

    /// Returns an existing pairing token or generates a new one.
    /// The token is displayed on the server console for first-time pairing.
    /// If no API keys exist in the DB, a new pairing token is generated.
    pub fn ensure_pairing_token(&self) -> anyhow::Result<String> {
        let existing = self.store.get_config("pairing_token")?;
        if let Some(token) = existing {
            return Ok(token);
        }

        // Generate a new token if none exists
        let token = generate_random_token("rover-pair");
        self.store.set_config("pairing_token", &token)?;
        Ok(token)
    }

    /// Verify a pairing token and consume it (it can only be used once).
    /// Returns a new persistent API key on success.
    pub fn pair(&self, token: &str) -> anyhow::Result<String> {
        let stored = self.store.get_config("pairing_token")?;
        match stored {
            Some(ref t) if t == token => {
                // Consume the token
                self.store.delete_config("pairing_token")?;

                // Generate a persistent API key
                let api_key = generate_random_token("rover-key");
                let hash = hash_token(&api_key);
                self.store.insert_auth_token(&hash)?;

                Ok(api_key)
            }
            _ => anyhow::bail!("invalid pairing token"),
        }
    }

    /// Verify an API key. Returns true if valid.
    pub fn verify_api_key(&self, key: &str) -> anyhow::Result<bool> {
        let hash = hash_token(key);
        self.store.verify_auth_token(&hash)
    }
}

/// Hash a token with SHA-256 for storage.
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    format!("{digest:x}")
}

/// Generate a random token with the given prefix.
fn generate_random_token(prefix: &str) -> String {
    use rand::Rng;
    let random_part: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    format!("{prefix}-{random_part}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (AuthManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let store = StateStore::open(&path).unwrap();
        let auth = AuthManager::new(store);
        (auth, dir)
    }

    #[test]
    fn test_pairing_flow() {
        let (auth, _dir) = setup();
        let token = auth.ensure_pairing_token().unwrap();
        assert!(token.starts_with("rover-pair-"));

        // Pairing succeeds and returns an API key
        let api_key = auth.pair(&token).unwrap();
        assert!(api_key.starts_with("rover-key-"));

        // The pairing token is consumed
        assert!(auth.pair(&token).is_err());

        // The API key is valid
        assert!(auth.verify_api_key(&api_key).unwrap());

        // A wrong key is not valid
        assert!(!auth.verify_api_key("rover-key-bad").unwrap());
    }

    #[test]
    fn test_ensure_pairing_token_reuses_existing() {
        let (auth, _dir) = setup();
        let t1 = auth.ensure_pairing_token().unwrap();
        let t2 = auth.ensure_pairing_token().unwrap();
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_wrong_pairing_token_fails() {
        let (auth, _dir) = setup();
        let _ = auth.ensure_pairing_token().unwrap();
        assert!(auth.pair("wrong-token").is_err());
    }

    #[test]
    fn test_api_key_validation() {
        let (auth, _dir) = setup();
        let token = auth.ensure_pairing_token().unwrap();
        let api_key = auth.pair(&token).unwrap();

        // Valid
        assert!(auth.verify_api_key(&api_key).unwrap());
        // Invalid
        assert!(
            !auth
                .verify_api_key("rover-key-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")
                .unwrap()
        );
    }
}
