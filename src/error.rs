use thiserror::Error;

#[derive(Error, Debug)]
pub enum CcmError {
    #[error("Session '{0}' already exists")]
    SessionExists(String),

    #[error("Session '{0}' not found")]
    SessionNotFound(String),

    #[error("WezTerm CLI failed: {0}")]
    WezTerm(String),

    #[error("State file error: {0}")]
    State(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
