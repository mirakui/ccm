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
}
