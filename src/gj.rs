use std::process::Command;

use serde::Deserialize;

use crate::error::CcmError;

#[derive(Debug, Deserialize)]
pub struct NewOutput {
    pub worktree_path: String,
    pub branch: String,
}

/// Run `gj new [branch-suffix] --output=json` in the given directory.
/// If branch_suffix is None, uses `--random-suffix` instead.
/// Returns the parsed worktree information.
pub fn new_worktree(cwd: &str, branch_suffix: Option<&str>) -> Result<NewOutput, CcmError> {
    let mut cmd = Command::new("gj");
    cmd.arg("new");

    if let Some(suffix) = branch_suffix {
        cmd.arg(suffix);
    } else {
        cmd.arg("--random-suffix");
    }

    cmd.arg("--output=json");
    cmd.current_dir(cwd);

    let output = cmd
        .output()
        .map_err(|e| CcmError::Gj(format!("failed to run gj new: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::Gj(format!("gj new failed: {stderr}")));
    }

    parse_new_output(&output.stdout)
}

fn parse_new_output(bytes: &[u8]) -> Result<NewOutput, CcmError> {
    serde_json::from_slice(bytes)
        .map_err(|e| CcmError::Gj(format!("failed to parse gj new output: {e}")))
}

/// Run `gj exit` in the given worktree directory.
/// If `merge` is true, uses `--merge`; otherwise uses `--force`.
pub fn exit_worktree(worktree_path: &str, merge: bool) -> Result<(), CcmError> {
    let flag = if merge { "--merge" } else { "--force" };
    let output = Command::new("gj")
        .args(["exit", flag])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| CcmError::Gj(format!("failed to run gj exit: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcmError::Gj(format!("gj exit {flag} failed: {stderr}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gj_new_output() {
        let json = r#"{"worktree_path":"/tmp/repo-abc123","branch":"gj/main-abc123"}"#;
        let result = parse_new_output(json.as_bytes()).unwrap();
        assert_eq!(result.worktree_path, "/tmp/repo-abc123");
        assert_eq!(result.branch, "gj/main-abc123");
    }

    #[test]
    fn parse_new_output_missing_field() {
        let json = r#"{"worktree_path":"/tmp/repo"}"#;
        let result = parse_new_output(json.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn parse_new_output_invalid_json() {
        let result = parse_new_output(b"not json");
        assert!(result.is_err());
    }
}
