mod cli;
mod config;
mod error;
mod gj;
mod pty_wrap;
mod session;
mod state;
mod tui;
mod wezterm;

use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;

use cli::{Cli, Command};
use config::Config;
use error::CcmError;
use session::Session;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Command::Init = &cli.command {
        let path = Config::init().context("failed to initialize config")?;
        println!("Created config file: {}", path.display());
        return Ok(());
    }

    let config = Config::load().context("failed to load config")?;

    if !Config::exists() {
        eprintln!("hint: no config file found. Run 'ccm init' to create ~/.config/ccm/config.toml");
    }

    match cli.command {
        Command::New { name, cwd } => {
            let claude_cmd = config.wezterm.claude_command.clone();
            cmd_new(&config, name, cwd, Some(claude_cmd))?;
        }
        Command::List => cmd_list(&config)?,
        Command::Switch { name } => cmd_switch(&config, &name)?,
        Command::Close { name, merge } => cmd_close(&config, name, merge)?,
        Command::Plan { cwd } => cmd_plan(&config, cwd)?,
        Command::TabWatcher { session } => tui::run(&session, &config)?,
        Command::Wrap { session, prompt_file, command } => {
            let exit_code = pty_wrap::run_wrap(&session, &command, prompt_file.as_deref())?;
            std::process::exit(exit_code);
        }
        Command::Init => unreachable!(),
    }

    Ok(())
}

struct NewSessionInfo {
    worktree_path: String,
    #[allow(dead_code)]
    session_name: String,
    claude_pane_id: u64,
}

/// Creates a new session. If `claude_command` is `Some`, sends it to the claude pane.
/// If `None`, the caller is responsible for sending the command later.
fn cmd_new(
    config: &Config,
    name: Option<String>,
    cwd: Option<String>,
    claude_command: Option<String>,
) -> Result<NewSessionInfo> {
    let cwd = match cwd {
        Some(p) => p,
        None => env::current_dir()
            .context("failed to get current directory")?
            .to_string_lossy()
            .to_string(),
    };

    // 1. Create git worktree via gj
    let gj_output = gj::new_worktree(&cwd, name.as_deref())
        .context("failed to create git worktree")?;
    let worktree_path = gj_output.worktree_path;
    let branch = gj_output.branch;

    // Validate branch name is not empty
    if branch.is_empty() {
        let _ = gj::exit_worktree(&worktree_path, false);
        return Err(anyhow::anyhow!("gj returned empty branch name"));
    }

    // session-name is always the same as branch name
    let session_name = branch.clone();

    let binary = &config.wezterm.binary;

    // Helper: kill panes + clean up worktree on failure
    let cleanup_panes = |binary: &str, panes: &[u64]| {
        for &pane_id in panes {
            let _ = wezterm::kill_pane(binary, pane_id);
        }
    };

    // 2. Spawn new tab in the worktree directory (this becomes the claude pane)
    let claude_pane_id = match wezterm::spawn_tab(binary, &worktree_path) {
        Ok(id) => id,
        Err(e) => {
            let _ = gj::exit_worktree(&worktree_path, false);
            return Err(e).context("failed to spawn new tab for session");
        }
    };

    // 3. Split left for tab-watcher
    let ccm_path = env::current_exe().context("failed to get ccm executable path")?;
    let ccm_str = ccm_path.to_string_lossy().to_string();
    let watcher_pane_id = match wezterm::split_pane(
        binary,
        claude_pane_id,
        wezterm::SplitDirection::Left,
        config.layout.watcher_width,
        Some(&[&ccm_str, "tab-watcher", "--session", &session_name]),
    ) {
        Ok(id) => id,
        Err(e) => {
            cleanup_panes(binary, &[claude_pane_id]);
            let _ = gj::exit_worktree(&worktree_path, false);
            return Err(e).context("failed to create tab-watcher pane");
        }
    };

    // 4. Split bottom for shell
    let shell_pane_id = match wezterm::split_pane(
        binary,
        claude_pane_id,
        wezterm::SplitDirection::Bottom,
        config.layout.shell_height,
        None,
    ) {
        Ok(id) => id,
        Err(e) => {
            cleanup_panes(binary, &[claude_pane_id, watcher_pane_id]);
            let _ = gj::exit_worktree(&worktree_path, false);
            return Err(e).context("failed to create shell pane");
        }
    };

    // 5. Send claude command to the claude pane (via PTY wrapper for OSC 0 detection)
    if let Some(cmd) = &claude_command {
        let quoted_session = session_name.replace('\'', "'\\''");
        let wrapped_cmd = format!(
            "{} wrap --session '{}' -- {}\n",
            ccm_str,
            quoted_session,
            cmd.trim_end_matches('\n')
        );
        wezterm::send_text(binary, claude_pane_id, &wrapped_cmd)
            .context("failed to send claude command to pane")?;
    }

    // 6. Set tab title
    wezterm::set_tab_title(binary, watcher_pane_id, &session_name)
        .context("failed to set tab title")?;

    // 7. Find the tab_id from wezterm list
    let panes = wezterm::list_panes(binary).context("failed to list panes")?;
    let tab_id = panes
        .iter()
        .find(|p| p.pane_id == claude_pane_id)
        .map(|p| p.tab_id)
        .ok_or_else(|| anyhow::anyhow!("could not find tab_id for pane {claude_pane_id}"))?;

    // 8. Save to state (duplicate check inside lock to avoid TOCTOU race)
    let session = Session {
        name: session_name.clone(),
        tab_id,
        watcher_pane_id,
        claude_pane_id,
        shell_pane_id,
        cwd: worktree_path,
        created_at: Utc::now(),
        claude_status: None,
    };

    let result = state::update(|state| {
        if state.sessions.iter().any(|s| s.name == session_name) {
            return Err(CcmError::SessionExists(session_name.clone()));
        }
        state.active_session = Some(session_name.clone());
        state.sessions.push(session.clone());
        Ok(())
    });

    if let Err(e) = result {
        cleanup_panes(binary, &[watcher_pane_id, shell_pane_id, claude_pane_id]);
        let _ = gj::exit_worktree(&session.cwd, false);
        return Err(e.into());
    }

    // Activate claude pane so user can immediately interact with Claude Code
    // This is best-effort; if it fails, the session is still functional
    if let Err(e) = wezterm::activate_pane(binary, claude_pane_id) {
        eprintln!("Warning: failed to activate claude pane: {e}");
    }

    let created_cwd = session.cwd.clone();
    println!("Created session '{session_name}' (tab {tab_id}, branch {branch})");
    Ok(NewSessionInfo {
        worktree_path: created_cwd,
        session_name,
        claude_pane_id,
    })
}

fn cmd_list(config: &Config) -> Result<()> {
    let state = state::load()?;
    let live_panes = wezterm::list_panes(&config.wezterm.binary).unwrap_or_default();
    let live_pane_ids: std::collections::HashSet<u64> =
        live_panes.iter().map(|p| p.pane_id).collect();

    if state.sessions.is_empty() {
        println!("No sessions.");
        return Ok(());
    }

    for session in &state.sessions {
        let is_active = state.active_session.as_deref() == Some(&session.name);
        let active_mark = if is_active { " *" } else { "" };

        let alive = live_pane_ids.contains(&session.claude_pane_id)
            || live_pane_ids.contains(&session.shell_pane_id);
        let status = if alive { "" } else { " [dead]" };

        let claude_status = session
            .claude_status
            .as_deref()
            .unwrap_or("");
        let claude_info = if claude_status.is_empty() {
            String::new()
        } else {
            format!(" [{claude_status}]")
        };

        println!(
            "  {}{active_mark}{status}{claude_info}  (tab:{}, cwd:{})",
            session.name, session.tab_id, session.cwd
        );
    }

    Ok(())
}

fn cmd_switch(config: &Config, name: &str) -> Result<()> {
    // Read state under lock, validate session exists, update active, then activate tab
    let tab_id = state::update(|state| {
        if !state.sessions.iter().any(|s| s.name == name) {
            return Err(CcmError::SessionNotFound(name.to_string()));
        }
        state.active_session = Some(name.to_string());
        Ok(())
    })?
    .sessions
    .iter()
    .find(|s| s.name == name)
    .map(|s| s.tab_id)
    .expect("session was just validated to exist");

    wezterm::activate_tab(&config.wezterm.binary, tab_id).context("failed to activate tab")?;

    println!("Switched to '{name}'");
    Ok(())
}

fn get_editor() -> String {
    env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vim".to_string())
}

fn open_editor_for_plan() -> Result<String> {
    use std::io::Read;

    let tmp_dir = env::temp_dir();
    let tmp_path = tmp_dir.join(format!(
        "ccm-plan-{}-{}.md",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));

    // Use create_new (O_EXCL) to prevent symlink attacks on predictable paths
    fs::File::create_new(&tmp_path).context("failed to create temporary plan file")?;

    let editor = get_editor();
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vim");
    let editor_args: Vec<&str> = parts.collect();

    let status = std::process::Command::new(program)
        .args(editor_args)
        .arg(&tmp_path)
        .status()
        .context(format!("failed to launch editor '{}'", editor))?;

    if !status.success() {
        fs::remove_file(&tmp_path).ok();
        anyhow::bail!("editor exited with non-zero status");
    }

    let mut content = String::new();
    fs::File::open(&tmp_path)
        .context("failed to open plan file after editing")?
        .read_to_string(&mut content)
        .context("failed to read plan file")?;

    fs::remove_file(&tmp_path).ok();

    Ok(content)
}

fn sanitize_session_name(s: &str) -> String {
    // Single-pass: collapse non-alphanumeric runs into a single hyphen.
    // Result is always pure ASCII, so byte-level truncation is safe.
    let mut result = String::with_capacity(s.len());
    let mut prev_hyphen = false;

    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen {
            result.push('-');
            prev_hyphen = true;
        }
    }

    let trimmed = result.trim_matches('-');
    if trimmed.len() > 50 {
        trimmed[..50].trim_end_matches('-').to_string()
    } else {
        trimmed.to_string()
    }
}

fn extract_name_from_first_line(content: &str) -> Option<String> {
    let first_line = content
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())?;

    // Strip markdown heading prefix (e.g. "# Title" -> "Title")
    let stripped = first_line.trim_start_matches('#').trim_start();

    let name = sanitize_session_name(stripped);
    if name.is_empty() {
        return None;
    }
    Some(name)
}

fn generate_session_name_from_plan(content: &str) -> String {
    if let Some(name) = extract_name_from_first_line(content) {
        if name.len() >= 3 {
            return name;
        }
    }

    let now = Utc::now();
    format!("plan-{}", now.format("%Y%m%d-%H%M%S"))
}

fn save_plan_to_worktree(worktree_path: &str, content: &str) -> Result<()> {
    let cctmp_dir = PathBuf::from(worktree_path).join(".cctmp");

    fs::create_dir_all(&cctmp_dir).context("failed to create .cctmp directory")?;

    // Ensure .cctmp contents are gitignored
    let gitignore = cctmp_dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, "*\n").ok();
    }

    let plan_file = cctmp_dir.join("plan.md");
    fs::write(&plan_file, content).context("failed to write plan.md")?;

    Ok(())
}

fn cmd_plan(config: &Config, cwd: Option<String>) -> Result<()> {
    let plan_content = open_editor_for_plan().context("failed to capture plan content")?;

    let trimmed = plan_content.trim();
    if trimmed.is_empty() {
        anyhow::bail!("plan content is empty. Session not created.");
    }

    let branch_suffix = generate_session_name_from_plan(&plan_content);

    println!("Creating session with branch suffix '{}'...", branch_suffix);

    let info = cmd_new(config, Some(branch_suffix), cwd, None)?;

    save_plan_to_worktree(&info.worktree_path, &plan_content)
        .context("failed to save plan to worktree")?;

    let ccm_path = env::current_exe().context("failed to get ccm executable path")?;
    let ccm_str = ccm_path.to_string_lossy().to_string();
    let quoted_session = info.session_name.replace('\'', "'\\''");
    let plan_path = PathBuf::from(&info.worktree_path)
        .join(".cctmp/plan.md");
    let plan_path_str = plan_path.to_string_lossy();
    let quoted_plan_path = plan_path_str.replace('\'', "'\\''");
    let claude_cmd = format!(
        "{} wrap --session '{}' --prompt-file '{}' -- {} --permission-mode=plan\n",
        ccm_str,
        quoted_session,
        quoted_plan_path,
        config.wezterm.claude_command.trim_end_matches('\n')
    );
    wezterm::send_text(&config.wezterm.binary, info.claude_pane_id, &claude_cmd)
        .context("failed to send claude plan command to pane")?;

    println!("Plan saved to .cctmp/plan.md");

    Ok(())
}

fn cmd_close(config: &Config, name: Option<String>, merge: bool) -> Result<()> {
    let name = match name {
        Some(n) => n,
        None => resolve_session_from_cwd()?,
    };
    let binary = &config.wezterm.binary;

    // If merging, attempt merge BEFORE destroying session state.
    // This way, on merge failure the session remains intact for the user to investigate.
    if merge {
        let state = state::load()?;
        let session = state
            .sessions
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| CcmError::SessionNotFound(name.to_string()))?;
        gj::exit_worktree(&session.cwd, true)
            .context("failed to merge and clean up worktree")?;
    }

    // Remove session from state atomically under lock
    let mut removed_session = None;
    state::update(|state| {
        let idx = state
            .sessions
            .iter()
            .position(|s| s.name == name)
            .ok_or_else(|| CcmError::SessionNotFound(name.to_string()))?;
        removed_session = Some(state.sessions.remove(idx));
        if state.active_session.as_deref() == Some(&*name) {
            state.active_session = None;
        }
        Ok(())
    })?;

    let session = removed_session.expect("session was just removed in update closure");

    // Kill all three panes (ignore errors for already-dead panes)
    let _ = wezterm::kill_pane(binary, session.watcher_pane_id);
    let _ = wezterm::kill_pane(binary, session.shell_pane_id);
    let _ = wezterm::kill_pane(binary, session.claude_pane_id);

    // Clean up git worktree (best-effort for non-merge path)
    if !merge {
        let _ = gj::exit_worktree(&session.cwd, false);
    }

    println!("Closed session '{name}'");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_basic() {
        assert_eq!(
            sanitize_session_name("Implement User Auth"),
            "implement-user-auth"
        );
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(sanitize_session_name("Fix bug #123!"), "fix-bug-123");
    }

    #[test]
    fn test_sanitize_unicode() {
        assert_eq!(
            sanitize_session_name("API redesign 🚀"),
            "api-redesign"
        );
    }

    #[test]
    fn test_sanitize_consecutive_hyphens() {
        assert_eq!(sanitize_session_name("a---b"), "a-b");
    }

    #[test]
    fn test_sanitize_max_length() {
        let long = "a".repeat(100);
        let result = sanitize_session_name(&long);
        assert!(result.len() <= 50);
    }

    #[test]
    fn test_generate_name_from_first_line() {
        let plan = "Implement authentication\n\nDetails here";
        assert_eq!(
            generate_session_name_from_plan(plan),
            "implement-authentication"
        );
    }

    #[test]
    fn test_generate_name_empty_content() {
        let name = generate_session_name_from_plan("");
        assert!(name.starts_with("plan-"));
    }

    #[test]
    fn test_generate_name_only_whitespace() {
        let name = generate_session_name_from_plan("   \n\n   ");
        assert!(name.starts_with("plan-"));
    }

    #[test]
    fn test_extract_name_skips_empty_lines() {
        let content = "\n\n  \nImplement feature\n";
        let result = extract_name_from_first_line(content);
        assert_eq!(result, Some("implement-feature".to_string()));
    }

    #[test]
    fn test_extract_name_too_short() {
        let content = "ab";
        let result = extract_name_from_first_line(content);
        assert_eq!(result, Some("ab".to_string()));
    }

    #[test]
    fn test_sanitize_all_special_chars() {
        assert_eq!(sanitize_session_name("---"), "");
        assert_eq!(sanitize_session_name("🚀🐛"), "");
    }

    #[test]
    fn test_extract_name_all_special_returns_none() {
        let content = "🚀🐛\nsome text";
        let result = extract_name_from_first_line(content);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_name_markdown_heading() {
        let content = "# Implement auth\n\nDetails";
        assert_eq!(
            extract_name_from_first_line(content),
            Some("implement-auth".to_string())
        );
    }

    #[test]
    fn test_generate_name_all_special_first_line_falls_back() {
        let name = generate_session_name_from_plan("🚀🐛\n");
        assert!(name.starts_with("plan-"));
    }
}

/// Resolve session name from the current working directory by matching against known sessions.
/// Uses canonicalized paths and picks the longest (most specific) match.
fn resolve_session_from_cwd() -> Result<String> {
    let cwd = env::current_dir()
        .context("failed to get current directory")?
        .canonicalize()
        .context("failed to canonicalize current directory")?;

    let state = state::load()?;
    let best = state
        .sessions
        .iter()
        .filter(|s| {
            std::path::Path::new(&s.cwd)
                .canonicalize()
                .map(|p| cwd.starts_with(&p))
                .unwrap_or(false)
        })
        .max_by_key(|s| s.cwd.len());

    match best {
        Some(session) => Ok(session.name.clone()),
        None => Err(anyhow::anyhow!(
            "no session found for current directory '{}'. Specify a session name explicitly.",
            cwd.display()
        )),
    }
}
