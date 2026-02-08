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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plans_pane_id: Option<u64>,
}

impl Session {
    /// Find a session from a slice by matching any of its pane IDs.
    pub fn find_by_pane_id(sessions: &[Session], pane_id: u64) -> Option<&Session> {
        sessions.iter().find(|s| {
            s.watcher_pane_id == pane_id
                || s.claude_pane_id == pane_id
                || s.shell_pane_id == pane_id
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_session() -> Session {
        Session {
            name: "test".to_string(),
            tab_id: 1,
            watcher_pane_id: 2,
            claude_pane_id: 3,
            shell_pane_id: 4,
            cwd: "/tmp".to_string(),
            created_at: Utc::now(),
            claude_status: None,
            plans_pane_id: None,
        }
    }

    #[test]
    fn serialize_roundtrip() {
        let session = sample_session();
        let json = serde_json::to_string(&session).unwrap();
        let restored: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, session.name);
        assert_eq!(restored.tab_id, session.tab_id);
        assert_eq!(restored.claude_status, None);
    }

    #[test]
    fn without_claude_status_omits_field() {
        let session = sample_session();
        let json = serde_json::to_string(&session).unwrap();
        assert!(!json.contains("claude_status"));
    }

    #[test]
    fn with_claude_status_includes_field() {
        let mut session = sample_session();
        session.claude_status = Some("idle".to_string());
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("claude_status"));
        assert!(json.contains("idle"));
    }

    #[test]
    fn deserialize_missing_claude_status() {
        let json = r#"{
            "name":"s","tab_id":1,"watcher_pane_id":2,
            "claude_pane_id":3,"shell_pane_id":4,
            "cwd":"/tmp","created_at":"2024-01-01T00:00:00Z"
        }"#;
        let session: Session = serde_json::from_str(json).unwrap();
        assert_eq!(session.claude_status, None);
    }

    #[test]
    fn deserialize_null_claude_status() {
        let json = r#"{
            "name":"s","tab_id":1,"watcher_pane_id":2,
            "claude_pane_id":3,"shell_pane_id":4,
            "cwd":"/tmp","created_at":"2024-01-01T00:00:00Z",
            "claude_status":null
        }"#;
        let session: Session = serde_json::from_str(json).unwrap();
        assert_eq!(session.claude_status, None);
    }

    #[test]
    fn chrono_datetime_roundtrip() {
        let session = sample_session();
        let json = serde_json::to_string(&session).unwrap();
        let restored: Session = serde_json::from_str(&json).unwrap();
        // chrono serializes to RFC3339 with nanosecond precision; comparing timestamps
        assert_eq!(
            session.created_at.timestamp(),
            restored.created_at.timestamp()
        );
    }

    #[test]
    fn find_by_pane_id_matches_watcher() {
        let sessions = vec![sample_session()];
        let found = Session::find_by_pane_id(&sessions, 2);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test");
    }

    #[test]
    fn find_by_pane_id_matches_claude() {
        let sessions = vec![sample_session()];
        let found = Session::find_by_pane_id(&sessions, 3);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test");
    }

    #[test]
    fn find_by_pane_id_matches_shell() {
        let sessions = vec![sample_session()];
        let found = Session::find_by_pane_id(&sessions, 4);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test");
    }

    #[test]
    fn find_by_pane_id_no_match() {
        let sessions = vec![sample_session()];
        let found = Session::find_by_pane_id(&sessions, 999);
        assert!(found.is_none());
    }

    #[test]
    fn find_by_pane_id_empty_slice() {
        let found = Session::find_by_pane_id(&[], 1);
        assert!(found.is_none());
    }

    #[test]
    fn find_by_pane_id_multiple_sessions() {
        let mut s1 = sample_session();
        s1.name = "first".to_string();
        s1.watcher_pane_id = 10;
        s1.claude_pane_id = 11;
        s1.shell_pane_id = 12;

        let mut s2 = sample_session();
        s2.name = "second".to_string();
        s2.watcher_pane_id = 20;
        s2.claude_pane_id = 21;
        s2.shell_pane_id = 22;

        let sessions = vec![s1, s2];
        let found = Session::find_by_pane_id(&sessions, 21);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "second");
    }
}
