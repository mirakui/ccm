use std::collections::HashMap;

use crate::error::CcmError;
use crate::gj;
use crate::session::Session;
use crate::state::{self, State};
use crate::wezterm;

pub enum ConfirmAction {
    Close(String),
    CloseWithMerge(String),
}

pub struct App {
    pub sessions: Vec<Session>,
    pub active_session: Option<String>,
    pub selected_index: usize,
    pub should_quit: bool,
    pub confirm_action: Option<ConfirmAction>,
    pub last_version: u64,
    pub own_session: String,
    pub status_message: Option<String>,
    pub pane_titles: HashMap<u64, String>,
    wezterm_binary: String,
    manual_navigation: bool,
}

impl App {
    pub fn new(session_name: &str, wezterm_binary: &str) -> Self {
        let mut app = Self {
            sessions: Vec::new(),
            active_session: None,
            selected_index: 0,
            should_quit: false,
            confirm_action: None,
            last_version: 0,
            own_session: session_name.to_string(),
            status_message: None,
            pane_titles: HashMap::new(),
            wezterm_binary: wezterm_binary.to_string(),
            manual_navigation: false,
        };
        app.refresh_state();
        app
    }

    pub fn refresh_state(&mut self) {
        match state::load() {
            Ok(state) => {
                self.apply_state(state);
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading state: {e}"));
            }
        }
    }

    fn apply_state(&mut self, state: State) {
        if state.version == self.last_version && !self.sessions.is_empty() {
            return;
        }
        self.last_version = state.version;
        self.sessions = state.sessions;
        self.active_session = state.active_session;

        // 自動同期モードなら、selected_index をアクティブセッションに合わせる
        if !self.manual_navigation {
            self.sync_selected_to_active();
        }

        // Clamp selected index
        if !self.sessions.is_empty() && self.selected_index >= self.sessions.len() {
            self.selected_index = self.sessions.len() - 1;
        }
    }

    /// selected_index をアクティブセッションの位置に同期する。
    /// 自動同期モード（manual_navigation = false）のときに呼ばれる。
    fn sync_selected_to_active(&mut self) {
        if self.sessions.is_empty() {
            self.selected_index = 0;
            return;
        }

        if let Some(ref active_name) = self.active_session {
            // アクティブセッションの位置を検索
            if let Some(pos) = self.sessions.iter().position(|s| &s.name == active_name) {
                self.selected_index = pos;
                return;
            }
        }

        // フォールバック: active_session が None または見つからない場合
        // 現在の index が有効ならそのまま、無効なら 0 にリセット
        if self.selected_index >= self.sessions.len() {
            self.selected_index = 0;
        }
    }

    /// Reconcile state with live WezTerm panes.
    /// Remove sessions whose panes no longer exist.
    pub fn reconcile(&mut self) {
        let live_panes = match wezterm::list_panes(&self.wezterm_binary) {
            Ok(p) => p,
            Err(e) => {
                self.status_message = Some(format!("Reconcile error: {e}"));
                return;
            }
        };

        // Build pane title map from live panes
        let pane_title_map: HashMap<u64, &str> = live_panes
            .iter()
            .map(|p| (p.pane_id, p.title.as_str()))
            .collect();

        self.pane_titles.clear();
        for session in &self.sessions {
            if let Some(title) = pane_title_map.get(&session.claude_pane_id) {
                if !title.is_empty() {
                    self.pane_titles
                        .insert(session.claude_pane_id, title.to_string());
                }
            }
        }

        let live_pane_ids: std::collections::HashSet<u64> =
            live_panes.iter().map(|p| p.pane_id).collect();

        let dead_sessions: Vec<String> = self
            .sessions
            .iter()
            .filter(|s| {
                // A session is dead if none of its panes exist
                !live_pane_ids.contains(&s.claude_pane_id)
                    && !live_pane_ids.contains(&s.shell_pane_id)
                    && !live_pane_ids.contains(&s.watcher_pane_id)
            })
            .map(|s| s.name.clone())
            .collect();

        if dead_sessions.is_empty() {
            return;
        }

        match state::update(|state| {
            state
                .sessions
                .retain(|s| !dead_sessions.contains(&s.name));
            if let Some(ref active) = state.active_session {
                if dead_sessions.contains(active) {
                    state.active_session = None;
                }
            }
            Ok(())
        }) {
            Ok(new_state) => self.apply_state(new_state),
            Err(e) => {
                self.status_message = Some(format!("Reconcile save error: {e}"));
            }
        }
    }

    pub fn move_down(&mut self) {
        if !self.sessions.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.sessions.len();
            self.manual_navigation = true;
        }
    }

    pub fn move_up(&mut self) {
        if !self.sessions.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.sessions.len() - 1;
            } else {
                self.selected_index -= 1;
            }
            self.manual_navigation = true;
        }
    }

    pub fn switch_to_selected(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_index) {
            let name = session.name.clone();
            let tab_id = session.tab_id;

            if let Err(e) = wezterm::activate_tab(&self.wezterm_binary, tab_id) {
                self.status_message = Some(format!("Switch error: {e}"));
                return;
            }

            match state::update(|state| {
                state.active_session = Some(name.clone());
                Ok(())
            }) {
                Ok(new_state) => {
                    self.manual_navigation = false;
                    self.apply_state(new_state);
                }
                Err(e) => {
                    self.status_message = Some(format!("State update error: {e}"));
                }
            }
        }
    }

    pub fn request_close(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_index) {
            self.confirm_action = Some(ConfirmAction::Close(session.name.clone()));
        }
    }

    pub fn request_close_with_merge(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_index) {
            self.confirm_action = Some(ConfirmAction::CloseWithMerge(session.name.clone()));
        }
    }

    pub fn confirm_action_yes(&mut self) {
        if let Some(action) = self.confirm_action.take() {
            let (name, merge) = match action {
                ConfirmAction::Close(name) => (name, false),
                ConfirmAction::CloseWithMerge(name) => (name, true),
            };
            let is_own = name == self.own_session;
            if let Err(e) = self.do_close_session(&name, merge) {
                self.status_message = Some(format!("Close error: {e}"));
                return;
            }
            if is_own {
                self.should_quit = true;
            }
        }
    }

    pub fn confirm_action_no(&mut self) {
        self.confirm_action = None;
    }

    fn do_close_session(&mut self, name: &str, merge: bool) -> Result<(), CcmError> {
        // If merging, attempt merge BEFORE destroying session state.
        // On merge failure the session remains intact for the user to investigate.
        if merge {
            let state = state::load()?;
            let session = state
                .sessions
                .iter()
                .find(|s| s.name == name)
                .ok_or_else(|| CcmError::SessionNotFound(name.to_string()))?;
            gj::exit_worktree(&session.cwd, true)?;
        }

        // Remove session from state atomically under lock
        let mut removed_session = None;
        let new_state = state::update(|state| {
            let idx = state
                .sessions
                .iter()
                .position(|s| s.name == name)
                .ok_or_else(|| CcmError::SessionNotFound(name.to_string()))?;
            removed_session = Some(state.sessions.remove(idx));
            if state.active_session.as_deref() == Some(name) {
                state.active_session = None;
            }
            Ok(())
        })?;

        let session = removed_session.expect("session was just removed in update closure");

        // Kill panes (ignore errors for already-dead panes)
        // Kill watcher pane last so that own-session close completes shell/claude kills first
        let _ = wezterm::kill_pane(&self.wezterm_binary, session.shell_pane_id);
        let _ = wezterm::kill_pane(&self.wezterm_binary, session.claude_pane_id);
        let _ = wezterm::kill_pane(&self.wezterm_binary, session.watcher_pane_id);

        // Clean up git worktree (best-effort for non-merge path)
        if !merge {
            let _ = gj::exit_worktree(&session.cwd, false);
        }

        self.apply_state(new_state);
        Ok(())
    }

    pub fn select_by_click(&mut self, row: u16, area_width: u16) {
        use super::ui::wrap_text;

        let mut current_row: u16 = 2; // header + separator
        let indent = 3u16;
        let box_width = (area_width.saturating_sub(indent)) as usize;
        let inner_width = box_width.saturating_sub(4); // "│ " + " │"

        for (i, session) in self.sessions.iter().enumerate() {
            let session_start = current_row;
            current_row += 1; // session name line

            if let Some(title) = self.pane_titles.get(&session.claude_pane_id) {
                if !title.is_empty() && box_width > 4 {
                    let content_lines = wrap_text(title, inner_width).len().max(1);
                    current_row += (content_lines + 2) as u16; // top + content + bottom
                }
            }

            if row >= session_start && row < current_row {
                self.selected_index = i;
                self.manual_navigation = false;
                self.switch_to_selected();
                return;
            }
        }
    }
}
