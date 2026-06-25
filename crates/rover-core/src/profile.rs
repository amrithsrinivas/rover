use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single saved server connection profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Human-readable label (e.g., "Galaxy S25 - Home").
    pub name: String,
    /// Server address (e.g., "192.168.1.42:9050").
    pub address: String,
    /// Persistent API key obtained from pairing.
    pub api_key: Option<String>,
    /// When this profile was last connected to.
    #[serde(default)]
    pub last_used: Option<DateTime<Utc>>,
}

impl ConnectionProfile {
    /// Create a new profile with a generated ID.
    pub fn new(name: String, address: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            address,
            api_key: None,
            last_used: None,
        }
    }
}

/// Persistent store for connection profiles, serialized as JSON on disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionProfileStore {
    /// Saved profiles.
    #[serde(default)]
    pub profiles: Vec<ConnectionProfile>,
    /// ID of the last-used profile.
    #[serde(default)]
    pub active_profile_id: Option<String>,
}

impl ConnectionProfileStore {
    /// Default path for the profiles file.
    pub fn default_path() -> std::path::PathBuf {
        dirs_next().join("profiles.json")
    }

    /// Load profiles from disk. Returns defaults if the file does not exist.
    pub fn load_from_disk() -> Result<Self, crate::RoverError> {
        let path = Self::default_path();
        if path.exists() {
            let data = std::fs::read_to_string(&path).map_err(|e| crate::RoverError::Io(e))?;
            let store: Self = serde_json::from_str(&data)
                .map_err(|e| crate::RoverError::Other(format!("failed to parse profiles: {e}")))?;
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    /// Save profiles to disk.
    pub fn save_to_disk(&self) -> Result<(), crate::RoverError> {
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| crate::RoverError::Other(format!("failed to serialize profiles: {e}")))?;
        std::fs::write(&path, data)?;
        Ok(())
    }

    /// Add or update a profile.
    pub fn upsert(&mut self, profile: ConnectionProfile) {
        if let Some(existing) = self.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            self.profiles.push(profile);
        }
    }

    /// Remove a profile by ID.
    pub fn remove(&mut self, id: &str) {
        self.profiles.retain(|p| p.id != id);
        if self.active_profile_id.as_deref() == Some(id) {
            self.active_profile_id = None;
        }
    }

    /// Get a profile by ID.
    pub fn get(&self, id: &str) -> Option<&ConnectionProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Get the active profile.
    pub fn active(&self) -> Option<&ConnectionProfile> {
        self.active_profile_id
            .as_deref()
            .and_then(|id| self.get(id))
    }
}

/// Helper to get the Rover config directory.
fn dirs_next() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".config").join("rover")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_save_load() {
        let mut store = ConnectionProfileStore::default();
        store.profiles.push(ConnectionProfile::new(
            "Test Phone".into(),
            "192.168.1.42:9050".into(),
        ));

        // Serialize and deserialize without touching disk
        let json = serde_json::to_string(&store).unwrap();
        let loaded: ConnectionProfileStore = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].name, "Test Phone");
    }

    #[test]
    fn remove_clears_active_if_removed() {
        let mut store = ConnectionProfileStore::default();
        let id = "test-id".to_string();
        store.profiles.push(ConnectionProfile {
            id: id.clone(),
            name: "Test".into(),
            address: "addr".into(),
            api_key: None,
            last_used: None,
        });
        store.active_profile_id = Some(id.clone());

        store.remove(&id);
        assert!(store.profiles.is_empty());
        assert!(store.active_profile_id.is_none());
    }
}
