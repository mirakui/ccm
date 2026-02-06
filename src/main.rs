mod cli;
mod error;
mod session;
mod state;
mod tui;
mod wezterm;

use std::env;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;

use cli::{Cli, Command};
use error::CcmError;
use session::Session;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::New { name, cwd } => cmd_new(&name, cwd)?,
        Command::List => cmd_list()?,
        Command::Switch { name } => cmd_switch(&name)?,
        Command::Close { name } => cmd_close(&name)?,
        Command::TabWatcher { session } => tui::run(&session)?,
    }

    Ok(())
}

fn cmd_new(name: &str, cwd: Option<String>) -> Result<()> {
    let cwd = match cwd {
        Some(p) => p,
        None => env::current_dir()
            .context("failed to get current directory")?
            .to_string_lossy()
            .to_string(),
    };

    // 1. Spawn new tab (this becomes the claude pane)
    let claude_pane_id =
        wezterm::spawn_tab(&cwd).context("failed to spawn new tab for session")?;

    // Track created panes for cleanup on failure
    let cleanup = |panes: &[u64]| {
        for &pane_id in panes {
            let _ = wezterm::kill_pane(pane_id);
        }
    };

    // 2. Split left for tab-watcher (20%)
    let ccm_path = env::current_exe().context("failed to get ccm executable path")?;
    let ccm_str = ccm_path.to_string_lossy().to_string();
    let watcher_pane_id = match wezterm::split_pane(
        claude_pane_id,
        wezterm::SplitDirection::Left,
        20,
        Some(&[&ccm_str, "tab-watcher", "--session", name]),
    ) {
        Ok(id) => id,
        Err(e) => {
            cleanup(&[claude_pane_id]);
            return Err(e).context("failed to create tab-watcher pane");
        }
    };

    // 3. Split bottom for shell (30%)
    let shell_pane_id = match wezterm::split_pane(
        claude_pane_id,
        wezterm::SplitDirection::Bottom,
        30,
        None,
    ) {
        Ok(id) => id,
        Err(e) => {
            cleanup(&[claude_pane_id, watcher_pane_id]);
            return Err(e).context("failed to create shell pane");
        }
    };

    // 4. Send "claude\n" to the claude pane
    wezterm::send_text(claude_pane_id, "claude\n")
        .context("failed to send claude command to pane")?;

    // 5. Set tab title
    wezterm::set_tab_title(watcher_pane_id, name)
        .context("failed to set tab title")?;

    // 6. Find the tab_id from wezterm list
    let panes = wezterm::list_panes().context("failed to list panes")?;
    let tab_id = panes
        .iter()
        .find(|p| p.pane_id == claude_pane_id)
        .map(|p| p.tab_id)
        .ok_or_else(|| anyhow::anyhow!("could not find tab_id for pane {claude_pane_id}"))?;

    // 7. Save to state (duplicate check inside lock to avoid TOCTOU race)
    let session = Session {
        name: name.to_string(),
        tab_id,
        watcher_pane_id,
        claude_pane_id,
        shell_pane_id,
        cwd,
        created_at: Utc::now(),
    };

    let result = state::update(|state| {
        if state.sessions.iter().any(|s| s.name == name) {
            return Err(CcmError::SessionExists(name.to_string()));
        }
        state.active_session = Some(name.to_string());
        state.sessions.push(session.clone());
        Ok(())
    });

    if let Err(e) = result {
        cleanup(&[watcher_pane_id, shell_pane_id, claude_pane_id]);
        return Err(e.into());
    }

    println!("Created session '{name}' (tab {tab_id})");
    Ok(())
}

fn cmd_list() -> Result<()> {
    let state = state::load()?;
    let live_panes = wezterm::list_panes().unwrap_or_default();
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

        println!(
            "  {}{active_mark}{status}  (tab:{}, cwd:{})",
            session.name, session.tab_id, session.cwd
        );
    }

    Ok(())
}

fn cmd_switch(name: &str) -> Result<()> {
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

    wezterm::activate_tab(tab_id).context("failed to activate tab")?;

    println!("Switched to '{name}'");
    Ok(())
}

fn cmd_close(name: &str) -> Result<()> {
    let state = state::load()?;
    let session = state
        .sessions
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| CcmError::SessionNotFound(name.to_string()))?
        .clone();

    // Kill all three panes (ignore errors for already-dead panes)
    let _ = wezterm::kill_pane(session.watcher_pane_id);
    let _ = wezterm::kill_pane(session.shell_pane_id);
    let _ = wezterm::kill_pane(session.claude_pane_id);

    state::update(|state| {
        state.sessions.retain(|s| s.name != name);
        if state.active_session.as_deref() == Some(name) {
            state.active_session = None;
        }
        Ok(())
    })?;

    println!("Closed session '{name}'");
    Ok(())
}
