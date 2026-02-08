use thiserror::Error;

#[derive(Error, Debug)]
pub enum CcmError {
    #[error("Session '{0}' already exists")]
    SessionExists(String),

    #[error("Session '{0}' not found")]
    SessionNotFound(String),

    #[error("WezTerm CLI failed: {0}")]
    WezTerm(String),

    #[error("gj CLI failed: {0}")]
    Gj(String),

    #[error("State file error: {0}")]
    State(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_session_exists() {
        let e = CcmError::SessionExists("foo".to_string());
        assert_eq!(e.to_string(), "Session 'foo' already exists");
    }

    #[test]
    fn display_session_not_found() {
        let e = CcmError::SessionNotFound("bar".to_string());
        assert_eq!(e.to_string(), "Session 'bar' not found");
    }

    #[test]
    fn display_wezterm() {
        let e = CcmError::WezTerm("timeout".to_string());
        assert_eq!(e.to_string(), "WezTerm CLI failed: timeout");
    }

    #[test]
    fn display_gj() {
        let e = CcmError::Gj("err".to_string());
        assert_eq!(e.to_string(), "gj CLI failed: err");
    }

    #[test]
    fn display_state() {
        let e = CcmError::State("corrupt".to_string());
        assert_eq!(e.to_string(), "State file error: corrupt");
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let ccm_err: CcmError = io_err.into();
        assert!(matches!(ccm_err, CcmError::Io(_)));
    }

    #[test]
    fn from_json_error() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let ccm_err: CcmError = json_err.into();
        assert!(matches!(ccm_err, CcmError::Json(_)));
    }
}
