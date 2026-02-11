use crate::llm::models::provider_handle::ProviderClientFactory;
use crate::config::ProviderConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_reuses_client_for_same_provider_model_and_prompt() {
        let configs = vec![ProviderConfig {
            name: "openai".to_string(),
            brand: None,
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];

        let mut factory = ProviderClientFactory::default();
        let a = factory
            .get_or_create("openai", "gpt-4o-mini", &configs, Some("p".to_string()))
            .unwrap();
        let b = factory
            .get_or_create("openai", "gpt-4o-mini", &configs, Some("p".to_string()))
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn factory_recreates_client_when_prompt_changes() {
        let configs = vec![ProviderConfig {
            name: "openai".to_string(),
            brand: None,
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];

        let mut factory = ProviderClientFactory::default();
        let a = factory
            .get_or_create("openai", "gpt-4o-mini", &configs, Some("p1".to_string()))
            .unwrap();
        let b = factory
            .get_or_create("openai", "gpt-4o-mini", &configs, Some("p2".to_string()))
            .unwrap();
        assert!(!std::sync::Arc::ptr_eq(&a, &b));
    }
}