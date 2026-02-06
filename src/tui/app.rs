use crate::error::CcmError;
use crate::session::Session;
use crate::state::{self, State};
use crate::wezterm;

pub struct App {
    pub sessions: Vec<Session>,
    pub active_session: Option<String>,
    pub selected_index: usize,
    pub should_quit: bool,
    pub confirm_delete: Option<String>,
    pub last_version: u64,
    pub own_session: String,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(session_name: &str) -> Self {
        let mut app = Self {
            sessions: Vec::new(),
            active_session: None,
            selected_index: 0,
            should_quit: false,
            confirm_delete: None,
            last_version: 0,
            own_session: session_name.to_string(),
            status_message: None,
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
        // Clamp selected index
        if !self.sessions.is_empty() && self.selected_index >= self.sessions.len() {
            self.selected_index = self.sessions.len() - 1;
        }
    }

    /// Reconcile state with live WezTerm panes.
    /// Remove sessions whose panes no longer exist.
    pub fn reconcile(&mut self) {
        let live_panes = match wezterm::list_panes() {
            Ok(p) => p,
            Err(e) => {
                self.status_message = Some(format!("Reconcile error: {e}"));
                return;
            }
        };

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
        }
    }

    pub fn move_up(&mut self) {
        if !self.sessions.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.sessions.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn switch_to_selected(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_index) {
            let name = session.name.clone();
            let tab_id = session.tab_id;

            if let Err(e) = wezterm::activate_tab(tab_id) {
                self.status_message = Some(format!("Switch error: {e}"));
                return;
            }

            match state::update(|state| {
                state.active_session = Some(name.clone());
                Ok(())
            }) {
                Ok(new_state) => self.apply_state(new_state),
                Err(e) => {
                    self.status_message = Some(format!("State update error: {e}"));
                }
            }
        }
    }

    pub fn request_delete(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_index) {
            // Don't allow deleting our own session from the watcher
            if session.name == self.own_session {
                self.status_message = Some("Cannot delete own session from watcher".to_string());
                return;
            }
            self.confirm_delete = Some(session.name.clone());
        }
    }

    pub fn confirm_delete_yes(&mut self) {
        if let Some(name) = self.confirm_delete.take() {
            if let Err(e) = self.do_close_session(&name) {
                self.status_message = Some(format!("Delete error: {e}"));
            }
        }
    }

    pub fn confirm_delete_no(&mut self) {
        self.confirm_delete = None;
    }

    fn do_close_session(&mut self, name: &str) -> Result<(), CcmError> {
        let session = self
            .sessions
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| CcmError::SessionNotFound(name.to_string()))?
            .clone();

        // Kill panes (ignore errors for already-dead panes)
        let _ = wezterm::kill_pane(session.watcher_pane_id);
        let _ = wezterm::kill_pane(session.shell_pane_id);
        let _ = wezterm::kill_pane(session.claude_pane_id);

        let new_state = state::update(|state| {
            state.sessions.retain(|s| s.name != name);
            if state.active_session.as_deref() == Some(name) {
                state.active_session = None;
            }
            Ok(())
        })?;

        self.apply_state(new_state);
        Ok(())
    }

    pub fn select_by_click(&mut self, row: u16) {
        // row 0 = title, row 1 = separator, sessions start at row 2
        if row >= 2 {
            let index = (row - 2) as usize;
            if index < self.sessions.len() {
                self.selected_index = index;
                self.switch_to_selected();
            }
        }
    }
}
