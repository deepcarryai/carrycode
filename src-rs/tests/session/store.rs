use crate::session::store::*;
use crate::llm::models::provider_handle::Message;
use std::env;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_session_id_allows_simple_ids() {
        assert!(validate_session_id("abc").is_ok());
        assert!(validate_session_id("abc-DEF_123").is_ok());
        assert!(validate_session_id("").is_err());
        assert!(validate_session_id("../x").is_err());
        assert!(validate_session_id("a b").is_err());
    }

    #[test]
    fn snapshot_roundtrip() {
        let original_home = env::var("HOME").ok();
    let tmp_home = env::temp_dir().join(format!("carrycode_coreapi-test-home-{}", now_ms()));
        std::fs::create_dir_all(&tmp_home).unwrap();
        env::set_var("HOME", &tmp_home);

        let session_id = "test_session_1";
        let snapshot = SessionSnapshot {
            version: SESSION_SNAPSHOT_VERSION,
            session_id: session_id.to_string(),
            created_at_ms: 0,
            updated_at_ms: 0,
            agent_mode: "build".to_string(),
            approval_mode: "agent".to_string(),
            enabled_skills: None,
            title: None,
            messages: vec![Message {
                role: "user".to_string(),
                content: "hello".to_string(),
                reasoning_content: None,
            }],
        };
        save_snapshot(snapshot).unwrap();

        let loaded = load_snapshot(session_id).unwrap().unwrap();
        assert_eq!(loaded.session_id, session_id);
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].role, "user");
        assert_eq!(loaded.messages[0].content, "hello");

        match original_home {
            Some(v) => env::set_var("HOME", v),
            None => env::remove_var("HOME"),
        }
    }
}