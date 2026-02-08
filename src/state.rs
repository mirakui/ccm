use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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

/// Read state from the given path. Returns default state if file doesn't exist.
fn load_from(path: &Path) -> Result<State, CcmError> {
    if !path.exists() {
        return Ok(State::default());
    }
    let data = fs::read_to_string(path)
        .map_err(|e| CcmError::State(format!("failed to read {}: {e}", path.display())))?;
    let state: State = serde_json::from_str(&data)?;
    Ok(state)
}

/// Read the state from disk. Returns default state if file doesn't exist.
pub fn load() -> Result<State, CcmError> {
    load_from(&state_path()?)
}

/// Atomically update state at the given path: load, apply function, save.
fn update_at<F>(path: &Path, f: F) -> Result<State, CcmError>
where
    F: FnOnce(&mut State) -> Result<(), CcmError>,
{
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

    let mut state = load_from(path)?;

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

/// Atomically update state: load, apply function, save.
pub fn update<F>(f: F) -> Result<State, CcmError>
where
    F: FnOnce(&mut State) -> Result<(), CcmError>,
{
    update_at(&state_path()?, f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use chrono::Utc;

    fn sample_session(name: &str) -> Session {
        Session {
            name: name.to_string(),
            tab_id: 1,
            watcher_pane_id: 2,
            claude_pane_id: 3,
            shell_pane_id: 4,
            cwd: "/tmp".to_string(),
            created_at: Utc::now(),
            claude_status: None,
        }
    }

    #[test]
    fn state_default() {
        let s = State::default();
        assert!(s.sessions.is_empty());
        assert_eq!(s.active_session, None);
        assert_eq!(s.version, 0);
    }

    #[test]
    fn state_serialize_roundtrip() {
        let mut state = State::default();
        state.sessions.push(sample_session("a"));
        state.active_session = Some("a".to_string());
        state.version = 5;
        let json = serde_json::to_string(&state).unwrap();
        let restored: State = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.sessions.len(), 1);
        assert_eq!(restored.sessions[0].name, "a");
        assert_eq!(restored.active_session, Some("a".to_string()));
        assert_eq!(restored.version, 5);
    }

    #[test]
    fn state_serialize_empty() {
        let state = State::default();
        let json = serde_json::to_string(&state).unwrap();
        let restored: State = serde_json::from_str(&json).unwrap();
        assert!(restored.sessions.is_empty());
    }

    #[test]
    fn state_deserialize_with_sessions() {
        let json = r#"{
            "sessions": [
                {"name":"s1","tab_id":1,"watcher_pane_id":2,"claude_pane_id":3,"shell_pane_id":4,"cwd":"/","created_at":"2024-01-01T00:00:00Z"},
                {"name":"s2","tab_id":5,"watcher_pane_id":6,"claude_pane_id":7,"shell_pane_id":8,"cwd":"/tmp","created_at":"2024-01-02T00:00:00Z"}
            ],
            "active_session": "s1",
            "version": 3
        }"#;
        let state: State = serde_json::from_str(json).unwrap();
        assert_eq!(state.sessions.len(), 2);
        assert_eq!(state.sessions[0].name, "s1");
        assert_eq!(state.sessions[1].name, "s2");
        assert_eq!(state.active_session, Some("s1".to_string()));
        assert_eq!(state.version, 3);
    }

    #[test]
    fn state_path_ends_with_expected() {
        let path = state_path().unwrap();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with("ccm/state.json"),
            "path was: {path_str}"
        );
    }

    // ---------------------------------------------------------------
    // File I/O tests (using tempfile)
    // ---------------------------------------------------------------

    fn temp_state_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("ccm").join("state.json")
    }

    #[test]
    fn load_from_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        let state = load_from(&path).unwrap();
        assert!(state.sessions.is_empty());
        assert_eq!(state.version, 0);
    }

    #[test]
    fn load_from_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let json = r#"{"sessions":[],"active_session":null,"version":42}"#;
        fs::write(&path, json).unwrap();
        let state = load_from(&path).unwrap();
        assert_eq!(state.version, 42);
    }

    #[test]
    fn load_from_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not json").unwrap();
        assert!(load_from(&path).is_err());
    }

    #[test]
    fn update_at_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        let state = update_at(&path, |_| Ok(())).unwrap();
        assert_eq!(state.version, 1);
        assert!(path.exists());
    }

    #[test]
    fn update_at_increments_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        update_at(&path, |_| Ok(())).unwrap();
        let state = update_at(&path, |_| Ok(())).unwrap();
        assert_eq!(state.version, 2);
    }

    #[test]
    fn update_at_applies_closure() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        let state = update_at(&path, |s| {
            s.sessions.push(sample_session("test"));
            Ok(())
        })
        .unwrap();
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.sessions[0].name, "test");

        // Verify persisted
        let reloaded = load_from(&path).unwrap();
        assert_eq!(reloaded.sessions.len(), 1);
    }

    #[test]
    fn update_at_closure_error_no_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        // Create initial state
        update_at(&path, |s| {
            s.active_session = Some("original".to_string());
            Ok(())
        })
        .unwrap();

        // Attempt failing update
        let result = update_at(&path, |_| {
            Err(CcmError::State("intentional error".to_string()))
        });
        assert!(result.is_err());

        // State should be unchanged
        let state = load_from(&path).unwrap();
        assert_eq!(state.active_session, Some("original".to_string()));
        assert_eq!(state.version, 1);
    }

    #[test]
    fn update_at_no_tmp_file_remains() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_state_path(&dir);
        update_at(&path, |_| Ok(())).unwrap();
        let tmp_path = path.with_extension("tmp");
        assert!(!tmp_path.exists(), ".tmp file should not remain");
    }
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
