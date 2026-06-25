use serde::{Deserialize, Serialize};

use crate::{RoverError, Runtime};

/// The deployment manifest (`rover.toml`) that defines how to build and run an app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    pub app: AppSection,
    pub build: BuildSection,
    pub run: RunSection,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSection {
    /// Unique name for the app.
    pub name: String,
    /// The runtime language.
    pub runtime: String,
    /// Optional informational version.
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSection {
    /// Shell command to build/install dependencies.
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSection {
    /// Shell command to start the app.
    pub command: String,
}

impl AppManifest {
    /// Parse a TOML string into an AppManifest.
    pub fn from_toml(toml_str: &str) -> Result<Self, RoverError> {
        let manifest: AppManifest = toml::from_str(toml_str)
            .map_err(|e| RoverError::ManifestParse(format!("TOML parse error: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate the manifest fields.
    pub fn validate(&self) -> Result<(), RoverError> {
        if self.app.name.trim().is_empty() {
            return Err(RoverError::ManifestValidation(
                "app.name is required".into(),
            ));
        }

        if self.app.name.contains(char::is_whitespace) {
            return Err(RoverError::ManifestValidation(
                "app.name must not contain whitespace".into(),
            ));
        }

        let _: Runtime = self
            .app
            .runtime
            .parse()
            .map_err(|e| RoverError::ManifestValidation(e))?;

        if self.build.command.trim().is_empty() {
            return Err(RoverError::ManifestValidation(
                "build.command is required".into(),
            ));
        }

        if self.run.command.trim().is_empty() {
            return Err(RoverError::ManifestValidation(
                "run.command is required".into(),
            ));
        }

        Ok(())
    }

    /// The parsed Runtime enum value.
    pub fn runtime(&self) -> Result<Runtime, RoverError> {
        self.app
            .runtime
            .parse()
            .map_err(|e| RoverError::ManifestValidation(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_toml() -> &'static str {
        r#"
[app]
name = "my-app"
runtime = "python"

[build]
command = "pip install -r requirements.txt"

[run]
command = "python main.py"

[env]
DATABASE_URL = "sqlite:///data.db"
"#
    }

    #[test]
    fn parse_valid_manifest() {
        let m = AppManifest::from_toml(valid_toml()).unwrap();
        assert_eq!(m.app.name, "my-app");
        assert_eq!(m.app.runtime, "python");
        assert_eq!(m.runtime().unwrap(), Runtime::Python);
    }

    #[test]
    fn reject_empty_name() {
        let toml = valid_toml().replace("my-app", "");
        assert!(AppManifest::from_toml(&toml).is_err());
    }

    #[test]
    fn reject_unknown_runtime() {
        let toml = valid_toml().replace("python", "lua");
        assert!(AppManifest::from_toml(&toml).is_err());
    }

    #[test]
    fn reject_empty_build_command() {
        let toml = valid_toml().replace("pip install -r requirements.txt", "");
        assert!(AppManifest::from_toml(&toml).is_err());
    }
}
