use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::CcmError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub sessions: Vec<Session>,
    pub active_session: Option<String>,
    pub version: u64,
}

// Re-export for convenience
use crate::session::Session;

impl Default for State {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            active_session: None,
            version: 0,
        }
    }
}

/// Return the path to the state file (~/.local/state/ccm/state.json).
pub fn state_path() -> Result<PathBuf, CcmError> {
    let base = dirs::state_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
        .ok_or_else(|| CcmError::State("cannot determine home or state directory".into()))?;
    Ok(base.join("ccm").join("state.json"))
}

/// Read the state from disk. Returns default state if file doesn't exist.
pub fn load() -> Result<State, CcmError> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(State::default());
    }
    let data = fs::read_to_string(&path)
        .map_err(|e| CcmError::State(format!("failed to read {}: {e}", path.display())))?;
    let state: State = serde_json::from_str(&data)?;
    Ok(state)
}

/// Atomically update state: load, apply function, save.
pub fn update<F>(f: F) -> Result<State, CcmError>
where
    F: FnOnce(&mut State) -> Result<(), CcmError>,
{
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CcmError::State(format!(
                "failed to create directory {}: {e}",
                parent.display()
            ))
        })?;
    }

    let lock_path = path.with_extension("lock");
    let lock_file = fs::File::create(&lock_path)
        .map_err(|e| CcmError::State(format!("failed to create lock file: {e}")))?;

    flock_exclusive(&lock_file)?;

    let mut state = if path.exists() {
        let data = fs::read_to_string(&path)
            .map_err(|e| CcmError::State(format!("failed to read {}: {e}", path.display())))?;
        serde_json::from_str(&data)?
    } else {
        State::default()
    };

    f(&mut state)?;
    state.version += 1;

    let json = serde_json::to_string_pretty(&state)?;
    let tmp_path = path.with_extension("tmp");
    let mut tmp_file = fs::File::create(&tmp_path)
        .map_err(|e| CcmError::State(format!("failed to create temp file: {e}")))?;
    tmp_file
        .write_all(json.as_bytes())
        .map_err(|e| CcmError::State(format!("failed to write temp file: {e}")))?;
    tmp_file
        .sync_all()
        .map_err(|e| CcmError::State(format!("failed to sync temp file: {e}")))?;
    fs::rename(&tmp_path, &path)
        .map_err(|e| CcmError::State(format!("failed to rename temp file: {e}")))?;

    Ok(state)
}

#[cfg(unix)]
fn flock_exclusive(file: &fs::File) -> Result<(), CcmError> {
    use std::os::unix::io::AsRawFd;
    let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if ret != 0 {
        return Err(CcmError::State(format!(
            "flock failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}
