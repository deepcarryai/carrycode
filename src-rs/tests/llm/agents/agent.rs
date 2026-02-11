use crate::llm::agents::agent::Agent;
use crate::config::ProviderConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_model_selects_matching_provider_model_config() {
        let providers = vec![
            ProviderConfig {
                name: "qwen-prod".to_string(),
                brand: Some("qwen".to_string()),
                base_url: "https://u1.example/v1".to_string(),
                api_key: "k1".to_string(),
                models: vec!["qwen2.0".to_string()],
            },
            ProviderConfig {
                name: "qwen-prod".to_string(),
                brand: Some("qwen".to_string()),
                base_url: "https://u2.example/v1".to_string(),
                api_key: "k2".to_string(),
                models: vec!["qwen2.5".to_string()],
            },
        ];

        let mut agent =
            Agent::without_tools("qwen-prod".to_string(), "qwen2.0".to_string(), None, providers).unwrap();
        agent.set_model("qwen-prod", "qwen2.5").unwrap();
        assert_eq!(agent.get_base_url(), "https://u2.example/v1");
    }

    #[test]
    fn set_model_errors_on_duplicate_provider_model_config() {
        let providers = vec![
            ProviderConfig {
                name: "qwen-prod".to_string(),
                brand: Some("qwen".to_string()),
                base_url: "https://u1.example/v1".to_string(),
                api_key: "k1".to_string(),
                models: vec!["qwen2.0".to_string()],
            },
            ProviderConfig {
                name: "qwen-prod".to_string(),
                brand: Some("qwen".to_string()),
                base_url: "https://u2.example/v1".to_string(),
                api_key: "k2".to_string(),
                models: vec!["qwen2.5".to_string()],
            },
            ProviderConfig {
                name: "qwen-prod".to_string(),
                brand: Some("qwen".to_string()),
                base_url: "https://u3.example/v1".to_string(),
                api_key: "k3".to_string(),
                models: vec!["qwen2.5".to_string()],
            },
        ];

        let mut agent =
            Agent::without_tools("qwen-prod".to_string(), "qwen2.0".to_string(), None, providers).unwrap();
        let err = agent.set_model("qwen-prod", "qwen2.5").unwrap_err();
        assert!(err.to_string().contains("Duplicate provider config"));
    }
}