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

    pub(crate) fn apply_state(&mut self, state: State) {
        if state.version == self.last_version && !self.sessions.is_empty() {
            return;
        }
        self.last_version = state.version;
        self.sessions = state.sessions;
        self.active_session = state.active_session;

        // Reflect claude_status from state into pane_titles (overrides WezTerm polling)
        for session in &self.sessions {
            if let Some(ref status) = session.claude_status {
                self.pane_titles
                    .insert(session.claude_pane_id, status.clone());
            }
        }

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
            // claude_status from state.json takes precedence (real-time via PTY wrapper)
            if let Some(ref status) = session.claude_status {
                self.pane_titles
                    .insert(session.claude_pane_id, status.clone());
            } else if let Some(title) = pane_title_map.get(&session.claude_pane_id) {
                // Fallback to WezTerm pane title
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
                    && !s.plans_pane_id.is_some_and(|id| live_pane_ids.contains(&id))
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
        if let Some(plans_pane_id) = session.plans_pane_id {
            let _ = wezterm::kill_pane(&self.wezterm_binary, plans_pane_id);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    impl App {
        fn new_for_test() -> Self {
            Self {
                sessions: Vec::new(),
                active_session: None,
                selected_index: 0,
                should_quit: false,
                confirm_action: None,
                last_version: 0,
                own_session: "test-watcher".to_string(),
                status_message: None,
                pane_titles: HashMap::new(),
                wezterm_binary: "wezterm".to_string(),
                manual_navigation: false,
            }
        }
    }

    fn sample_session(name: &str, claude_pane_id: u64) -> Session {
        Session {
            name: name.to_string(),
            tab_id: 100,
            watcher_pane_id: 200,
            claude_pane_id,
            shell_pane_id: 400,
            cwd: "/tmp".to_string(),
            created_at: Utc::now(),
            claude_status: None,
            plans_pane_id: None,
        }
    }

    fn state_with_sessions(names: &[&str]) -> State {
        let sessions: Vec<Session> = names
            .iter()
            .enumerate()
            .map(|(i, name)| sample_session(name, 300 + i as u64))
            .collect();
        State {
            sessions,
            active_session: None,
            version: 1,
        }
    }

    // ---------------------------------------------------------------
    // move_down / move_up
    // ---------------------------------------------------------------

    #[test]
    fn move_down_increments() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["a", "b", "c"]));
        app.selected_index = 0;
        app.move_down();
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn move_down_wraps() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["a", "b", "c"]));
        app.selected_index = 2;
        app.move_down();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn move_down_empty_noop() {
        let mut app = App::new_for_test();
        app.move_down();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn move_up_decrements() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["a", "b", "c"]));
        app.selected_index = 2;
        app.move_up();
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn move_up_wraps() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["a", "b", "c"]));
        app.selected_index = 0;
        app.move_up();
        assert_eq!(app.selected_index, 2);
    }

    #[test]
    fn move_up_empty_noop() {
        let mut app = App::new_for_test();
        app.move_up();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn move_sets_manual_navigation() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["a", "b"]));
        assert!(!app.manual_navigation);
        app.move_down();
        assert!(app.manual_navigation);
    }

    // ---------------------------------------------------------------
    // request_close / confirm
    // ---------------------------------------------------------------

    #[test]
    fn request_close_sets_confirm() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["sess1"]));
        app.request_close();
        assert!(matches!(app.confirm_action, Some(ConfirmAction::Close(ref n)) if n == "sess1"));
    }

    #[test]
    fn request_close_empty_noop() {
        let mut app = App::new_for_test();
        app.request_close();
        assert!(app.confirm_action.is_none());
    }

    #[test]
    fn request_close_with_merge() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["sess1"]));
        app.request_close_with_merge();
        assert!(
            matches!(app.confirm_action, Some(ConfirmAction::CloseWithMerge(ref n)) if n == "sess1")
        );
    }

    #[test]
    fn confirm_no_clears() {
        let mut app = App::new_for_test();
        app.apply_state(state_with_sessions(&["sess1"]));
        app.request_close();
        assert!(app.confirm_action.is_some());
        app.confirm_action_no();
        assert!(app.confirm_action.is_none());
    }

    // ---------------------------------------------------------------
    // apply_state
    // ---------------------------------------------------------------

    #[test]
    fn apply_state_updates() {
        let mut app = App::new_for_test();
        let mut state = state_with_sessions(&["a", "b"]);
        state.active_session = Some("b".to_string());
        state.version = 5;
        app.apply_state(state);
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_session, Some("b".to_string()));
        assert_eq!(app.last_version, 5);
    }

    #[test]
    fn apply_state_skips_same_version() {
        let mut app = App::new_for_test();
        let state = state_with_sessions(&["a"]);
        app.apply_state(state);
        assert_eq!(app.last_version, 1);

        // Second apply with same version should be skipped (sessions not empty)
        let mut state2 = state_with_sessions(&["a", "b"]);
        state2.version = 1;
        app.apply_state(state2);
        // sessions should still be 1 (skipped)
        assert_eq!(app.sessions.len(), 1);
    }

    #[test]
    fn apply_state_syncs_selected() {
        let mut app = App::new_for_test();
        let mut state = state_with_sessions(&["a", "b", "c"]);
        state.active_session = Some("c".to_string());
        state.version = 1;
        app.apply_state(state);
        // manual_navigation is false, so should sync to "c" at index 2
        assert_eq!(app.selected_index, 2);
    }

    #[test]
    fn apply_state_clamps_index() {
        let mut app = App::new_for_test();
        let state = state_with_sessions(&["a", "b", "c"]);
        app.apply_state(state);
        app.selected_index = 2;

        // Shrink sessions
        let mut state2 = state_with_sessions(&["a"]);
        state2.version = 2;
        app.manual_navigation = true; // prevent sync to active
        app.apply_state(state2);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn apply_state_claude_status_to_pane_titles() {
        let mut app = App::new_for_test();
        let mut state = state_with_sessions(&["a"]);
        state.sessions[0].claude_status = Some("thinking...".to_string());
        state.version = 1;
        app.apply_state(state);
        let pane_id = app.sessions[0].claude_pane_id;
        assert_eq!(app.pane_titles.get(&pane_id).unwrap(), "thinking...");
    }

    #[test]
    fn manual_nav_prevents_sync() {
        let mut app = App::new_for_test();
        let mut state = state_with_sessions(&["a", "b", "c"]);
        state.active_session = Some("a".to_string());
        state.version = 1;
        app.apply_state(state);
        assert_eq!(app.selected_index, 0);

        // Manual navigation: move to index 2
        app.move_down();
        app.move_down();
        assert_eq!(app.selected_index, 2);
        assert!(app.manual_navigation);

        // apply_state with new version but active still "a"
        let mut state2 = state_with_sessions(&["a", "b", "c"]);
        state2.active_session = Some("a".to_string());
        state2.version = 2;
        app.apply_state(state2);
        // should stay at 2 because manual_navigation is true
        assert_eq!(app.selected_index, 2);
    }
}
