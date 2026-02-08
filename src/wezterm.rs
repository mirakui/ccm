use std::process::Command;

use crate::error::CcmError;

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct PaneInfo {
    pub window_id: u64,
    pub tab_id: u64,
    pub pane_id: u64,
    pub title: String,
    pub cwd: String,
    pub is_active: bool,
}

pub enum SplitDirection {
    Left,
    Right,
    Bottom,
}

/// Spawn a new tab and return the pane_id of the initial pane.
pub fn spawn_tab(binary: &str, cwd: &str) -> Result<u64, CcmError> {
    let output = Command::new(binary)
        .args(["cli", "spawn", "--cwd", cwd])
        .output()
        .map_err(|e| CcmError::WezTerm(format!("failed to run wezterm cli spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!("spawn failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<u64>()
        .map_err(|e| CcmError::WezTerm(format!("failed to parse pane_id from spawn: {e}")))
}

/// Split a pane and return the new pane_id.
/// If `program` is provided, it is run in the new pane.
pub fn split_pane(
    binary: &str,
    pane_id: u64,
    direction: SplitDirection,
    percent: u32,
    program: Option<&[&str]>,
) -> Result<u64, CcmError> {
    let mut args = vec![
        "cli".to_string(),
        "split-pane".to_string(),
        "--pane-id".to_string(),
        pane_id.to_string(),
    ];

    match direction {
        SplitDirection::Left => args.push("--left".to_string()),
        SplitDirection::Right => args.push("--right".to_string()),
        SplitDirection::Bottom => args.push("--bottom".to_string()),
    }

    args.push("--percent".to_string());
    args.push(percent.to_string());

    if let Some(prog) = program {
        args.push("--".to_string());
        for p in prog {
            args.push(p.to_string());
        }
    }

    let output = Command::new(binary)
        .args(&args)
        .output()
        .map_err(|e| CcmError::WezTerm(format!("failed to run wezterm cli split-pane: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!("split-pane failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<u64>()
        .map_err(|e| CcmError::WezTerm(format!("failed to parse pane_id from split-pane: {e}")))
}

/// Activate a tab by its tab_id.
pub fn activate_tab(binary: &str, tab_id: u64) -> Result<(), CcmError> {
    let output = Command::new(binary)
        .args(["cli", "activate-tab", "--tab-id", &tab_id.to_string()])
        .output()
        .map_err(|e| CcmError::WezTerm(format!("failed to run wezterm cli activate-tab: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!("activate-tab failed: {stderr}")));
    }
    Ok(())
}

/// Activate a pane by its pane_id.
pub fn activate_pane(binary: &str, pane_id: u64) -> Result<(), CcmError> {
    let output = Command::new(binary)
        .args(["cli", "activate-pane", "--pane-id", &pane_id.to_string()])
        .output()
        .map_err(|e| CcmError::WezTerm(format!("failed to run wezterm cli activate-pane: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!("activate-pane failed: {stderr}")));
    }
    Ok(())
}

/// Set the tab title using a pane_id to identify the tab.
pub fn set_tab_title(binary: &str, pane_id: u64, title: &str) -> Result<(), CcmError> {
    let output = Command::new(binary)
        .args([
            "cli",
            "set-tab-title",
            "--pane-id",
            &pane_id.to_string(),
            title,
        ])
        .output()
        .map_err(|e| {
            CcmError::WezTerm(format!("failed to run wezterm cli set-tab-title: {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!(
            "set-tab-title failed: {stderr}"
        )));
    }
    Ok(())
}

/// Kill a pane by pane_id.
pub fn kill_pane(binary: &str, pane_id: u64) -> Result<(), CcmError> {
    let output = Command::new(binary)
        .args(["cli", "kill-pane", "--pane-id", &pane_id.to_string()])
        .output()
        .map_err(|e| CcmError::WezTerm(format!("failed to run wezterm cli kill-pane: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!("kill-pane failed: {stderr}")));
    }
    Ok(())
}

/// List all panes from WezTerm.
pub fn list_panes(binary: &str) -> Result<Vec<PaneInfo>, CcmError> {
    let output = Command::new(binary)
        .args(["cli", "list", "--format", "json"])
        .output()
        .map_err(|e| CcmError::WezTerm(format!("failed to run wezterm cli list: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!("list failed: {stderr}")));
    }

    let panes: Vec<PaneInfo> = serde_json::from_slice(&output.stdout)
        .map_err(|e| CcmError::WezTerm(format!("failed to parse pane list: {e}")))?;

    Ok(panes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_info_deserialize() {
        let json = r#"{
            "window_id": 0,
            "tab_id": 1,
            "pane_id": 2,
            "title": "zsh",
            "cwd": "/home/user",
            "is_active": true
        }"#;
        let info: PaneInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.pane_id, 2);
        assert_eq!(info.title, "zsh");
        assert!(info.is_active);
    }

    #[test]
    fn pane_info_array_deserialize() {
        let json = r#"[
            {"window_id":0,"tab_id":1,"pane_id":2,"title":"a","cwd":"/","is_active":true},
            {"window_id":0,"tab_id":1,"pane_id":3,"title":"b","cwd":"/tmp","is_active":false}
        ]"#;
        let panes: Vec<PaneInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].pane_id, 2);
        assert_eq!(panes[1].pane_id, 3);
    }

    #[test]
    fn pane_info_extra_fields_ignored() {
        let json = r#"{
            "window_id": 0,
            "tab_id": 1,
            "pane_id": 2,
            "title": "zsh",
            "cwd": "/",
            "is_active": false,
            "unknown_field": "ignored",
            "another": 42
        }"#;
        let info: PaneInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.pane_id, 2);
    }

    #[test]
    fn pane_info_missing_field_errors() {
        let json = r#"{
            "window_id": 0,
            "tab_id": 1,
            "title": "zsh",
            "cwd": "/",
            "is_active": false
        }"#;
        let result = serde_json::from_str::<PaneInfo>(json);
        assert!(result.is_err());
    }
}

/// Send text to a pane (with --no-paste to avoid bracketed paste).
pub fn send_text(binary: &str, pane_id: u64, text: &str) -> Result<(), CcmError> {
    let output = Command::new(binary)
        .args([
            "cli",
            "send-text",
            "--pane-id",
            &pane_id.to_string(),
            "--no-paste",
            text,
        ])
        .output()
        .map_err(|e| CcmError::WezTerm(format!("failed to run wezterm cli send-text: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::WezTerm(format!("send-text failed: {stderr}")));
    }
    Ok(())
}
