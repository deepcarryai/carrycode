use crate::config::{resolve_default_model, AppConfig, ProviderConfig, RuntimeConfig, ToolChunkingConfig};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_deserializes_without_default_model() {
    let json = r#"{"theme":"carrycode_coreapi-dark","sessions":[]}"#;
        let cfg: RuntimeConfig = serde_json::from_str(json).expect("should parse old runtime schema");
        assert!(cfg.default_model.is_none());
    }

    #[test]
    fn runtime_config_deserializes_with_default_model() {
    let json = r#"{"theme":"carrycode_coreapi-dark","default_model":"openai:gpt-4o-mini","sessions":[]}"#;
        let cfg: RuntimeConfig = serde_json::from_str(json).expect("should parse new runtime schema");
        assert_eq!(cfg.default_model.as_deref(), Some("openai:gpt-4o-mini"));
    }

    #[test]
    fn resolve_default_model_falls_back_when_runtime_missing() {
        let providers = vec![ProviderConfig {
            name: "openai".to_string(),
            brand: None,
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];
        let (v, should_save) = resolve_default_model(false, None, &providers);
        assert_eq!(v.as_deref(), Some("openai:gpt-4o-mini"));
        assert!(should_save);
    }

    #[test]
    fn resolve_default_model_falls_back_when_runtime_default_model_empty() {
        let providers = vec![ProviderConfig {
            name: "openai".to_string(),
            brand: None,
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];
        let (v, should_save) =
            resolve_default_model(true, Some("   ".to_string()), &providers);
        assert_eq!(v.as_deref(), Some("openai:gpt-4o-mini"));
        assert!(should_save);
    }

    #[test]
    fn resolve_default_model_uses_runtime_value_when_present() {
        let providers = vec![ProviderConfig {
            name: "openai".to_string(),
            brand: None,
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];
        let (v, should_save) =
            resolve_default_model(true, Some("openai:gpt-4o-mini".to_string()), &providers);
        assert_eq!(v.as_deref(), Some("openai:gpt-4o-mini"));
        assert!(!should_save);
    }

    #[test]
    fn tool_chunking_uses_default_when_model_missing() {
        let cfg = ToolChunkingConfig {
            default_chunk_limit_chars: 1000,
            model_max_tokens: HashMap::new(),
        };
        assert_eq!(cfg.limit_chars_for_model(None), 1000);
        assert_eq!(cfg.limit_chars_for_model(Some("openai:gpt-4o-mini")), 1000);
    }

    #[test]
    fn tool_chunking_uses_max_tokens_times_ratio() {
        let mut map = HashMap::new();
        map.insert("openai:gpt-4o-mini".to_string(), 5000);
        map.insert("claude-sonnet-4-5-20250929".to_string(), 4096);
        let cfg = ToolChunkingConfig {
            default_chunk_limit_chars: 1000,
            model_max_tokens: map,
        };
        assert_eq!(cfg.limit_chars_for_model(Some("openai:gpt-4o-mini")), 10000);
        assert_eq!(cfg.limit_chars_for_model(Some("claude-sonnet-4-5-20250929")), 8192);
    }

    #[test]
    fn apply_patch_dedupes_by_provider_and_model() {
        let default_str = include_str!("../../Config.toml");
        let mut config: AppConfig = toml::from_str(default_str).expect("should parse embedded Config.toml");

        let tmp = std::env::temp_dir().join(format!(
            "carrycode_coreapi-test-{}.json",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));

        let content = r#"{
    "providers": [
      {
        "provider_brand": "qwen",
                "provider_id": "qwen-prod",
        "model_name": "qwen2.0",
        "base_url": "https://u1.example/v1",
        "api_key": "k1"
      },
      {
        "provider_brand": "qwen",
                "provider_id": "qwen-prod",
        "model_name": "qwen2.5",
        "base_url": "https://u2.example/v1",
        "api_key": "k2"
      },
      {
        "provider_brand": "qwen",
                "provider_id": "qwen-prod",
        "model_name": "qwen2.0",
        "base_url": "https://u3.example/v1",
        "api_key": "k3"
      }
    ]
  }"#;

        fs::write(&tmp, content).expect("should write temp patch file");
        AppConfig::apply_patch(&mut config, &tmp);
        let _ = fs::remove_file(&tmp);

        let q20 = config
            .providers
            .iter()
            .find(|p| p.name == "qwen-prod" && p.models.iter().any(|m| m == "qwen2.0"))
            .expect("qwen2.0 provider should exist");
        assert_eq!(q20.base_url, "https://u3.example/v1");
        assert_eq!(q20.api_key, "k3");
        assert_eq!(q20.models, vec!["qwen2.0".to_string()]);

        let q25 = config
            .providers
            .iter()
            .find(|p| p.name == "qwen-prod" && p.models.iter().any(|m| m == "qwen2.5"))
            .expect("qwen2.5 provider should exist");
        assert_eq!(q25.base_url, "https://u2.example/v1");
        assert_eq!(q25.api_key, "k2");
        assert_eq!(q25.models, vec!["qwen2.5".to_string()]);

        assert_eq!(config.providers.len(), 2);
    }
}