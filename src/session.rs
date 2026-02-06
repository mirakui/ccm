use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub tab_id: u64,
    pub watcher_pane_id: u64,
    pub claude_pane_id: u64,
    pub shell_pane_id: u64,
    pub cwd: String,
    pub created_at: DateTime<Utc>,
}
