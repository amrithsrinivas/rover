use thiserror::Error;

#[derive(Debug, Error)]
pub enum RoverError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Manifest parse error: {0}")]
    ManifestParse(String),

    #[error("Manifest validation error: {0}")]
    ManifestValidation(String),

    #[error("App not found: {0}")]
    AppNotFound(String),

    #[error("Runtime not available: {0}")]
    RuntimeNotAvailable(String),

    #[error("Build failed: {0}")]
    BuildFailed(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("State store error: {0}")]
    StateStore(String),

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("{0}")]
    Other(String),
}
