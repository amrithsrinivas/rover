use serde::{Deserialize, Serialize};

/// A saved GitHub personal access token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubToken {
    /// Unique identifier.
    pub id: String,
    /// Display label (e.g., "Personal", "Work").
    pub label: String,
    /// The access token value.
    pub token: String,
}

impl GithubToken {
    pub fn new(label: String, token: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            label,
            token,
        }
    }
}

/// Persistent store for GitHub tokens.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GithubTokenStore {
    #[serde(default)]
    pub tokens: Vec<GithubToken>,
}

impl GithubTokenStore {
    fn path() -> std::path::PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        std::path::PathBuf::from(home)
            .join(".config")
            .join("rover")
            .join("github-tokens.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }
}
