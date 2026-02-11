use crate::ffi::session_util::{derive_event_success_from_raw, system_prompt_for_agent_mode};
use crate::config::AppConfig;
use crate::session::context::AgentMode;

fn embedded_config() -> AppConfig {
    toml::from_str(include_str!("../../../Config.toml")).expect("embedded Config.toml should parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_plan_uses_prompt_plan_template() {
        let cfg = embedded_config();
        let prompt = system_prompt_for_agent_mode(&cfg, &AgentMode::Plan, "").unwrap_or_default();
        assert!(prompt.contains("read-only mode"));
    }

    #[test]
    fn prompt_build_uses_prompt_build_template() {
        let cfg = embedded_config();
        let prompt = system_prompt_for_agent_mode(&cfg, &AgentMode::Build, "").unwrap_or_default();
        assert!(prompt.contains("## New Applications"));
    }

    #[test]
    fn disabled_prompt_build_falls_back_to_default() {
        let mut cfg = embedded_config();
        if let Some(pb) = cfg.prompt_build.as_mut() {
            pb.enabled = false;
        }
        let prompt = system_prompt_for_agent_mode(&cfg, &AgentMode::Build, "");
        assert_eq!(prompt, Some("You are a helpful coding assistant.".to_string()));
    }

    #[test]
    fn derive_success_prefers_success_field() {
        let raw = r#"{"success":false,"stderr":"","stdout":"","response_summary":"error"}"#;
        assert_eq!(derive_event_success_from_raw(raw), Some(false));
    }

    #[test]
    fn derive_success_falls_back_to_stderr_empty() {
        let raw = r#"{"stderr":"boom"}"#;
        assert_eq!(derive_event_success_from_raw(raw), Some(false));
        let raw = r#"{"stderr":""}"#;
        assert_eq!(derive_event_success_from_raw(raw), Some(true));
    }
}