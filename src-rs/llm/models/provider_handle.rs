use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;

use crate::config::ProviderConfig;

use super::claude::ClaudeClient;
use super::codex::CodexClient;
use super::gemini::GeminiClient;
use super::openai::{
    create_deepseek, create_openai, create_qwen, create_zhipuai, OpenAiClient,
};
pub use super::provider_base::{Message, ProviderClient};

pub enum AnyProviderClient {
    Claude(ClaudeClient),
    Codex(CodexClient),
    Gemini(GeminiClient),
    OpenAI(OpenAiClient),
}

impl ProviderClient for AnyProviderClient {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>> {
        match self {
            AnyProviderClient::Claude(c) => c.stream_chat(messages, tools).await,
            AnyProviderClient::Codex(c) => c.stream_chat(messages, tools).await,
            AnyProviderClient::Gemini(c) => c.stream_chat(messages, tools).await,
            AnyProviderClient::OpenAI(c) => c.stream_chat(messages, tools).await,
        }
    }

    async fn chat(&self, messages: Vec<Message>, tools: Option<Vec<Value>>) -> Result<Value> {
        match self {
            AnyProviderClient::Claude(c) => c.chat(messages, tools).await,
            AnyProviderClient::Codex(c) => c.chat(messages, tools).await,
            AnyProviderClient::Gemini(c) => c.chat(messages, tools).await,
            AnyProviderClient::OpenAI(c) => c.chat(messages, tools).await,
        }
    }
}

pub fn create_client(
    provider: &str,
    base_url: String,
    api_key: String,
    model_name: String,
    system_prompt: Option<String>,
) -> AnyProviderClient {
    match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => AnyProviderClient::Claude(
            ClaudeClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt),
        ),
        "codex" => AnyProviderClient::Codex(
            CodexClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt),
        ),
        "gemini" => AnyProviderClient::Gemini(
            GeminiClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt),
        ),
        "openai" => AnyProviderClient::OpenAI(create_openai(base_url, api_key, model_name, system_prompt)),
        "zhipuai" => AnyProviderClient::OpenAI(create_zhipuai(base_url, api_key, model_name, system_prompt)),
        "deepseek" => AnyProviderClient::OpenAI(create_deepseek(base_url, api_key, model_name, system_prompt)),
        "qwen" => AnyProviderClient::OpenAI(create_qwen(base_url, api_key, model_name, system_prompt)),
        _ => AnyProviderClient::OpenAI(create_openai(base_url, api_key, model_name, system_prompt)),
    }
}

#[derive(Default)]
pub struct ProviderClientFactory {
    cache: HashMap<(String, String), Arc<AnyProviderClient>>,
}

impl ProviderClientFactory {
    pub fn get_or_create(
        &mut self,
        provider_name: &str,
        model_name: &str,
        provider_configs: &[ProviderConfig],
        system_prompt: Option<String>,
    ) -> Result<Arc<AnyProviderClient>> {
        let key = (provider_name.to_string(), model_name.to_string());

        if let Some(existing) = self.cache.get(&key) {
            let existing_prompt = system_prompt_of(existing);
            if existing_prompt == system_prompt.as_ref() {
                return Ok(Arc::clone(existing));
            }
        }

        let config = provider_configs
            .iter()
            .find(|c| c.name == provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_name))?;

        let client = create_client(
            provider_name,
            config.base_url.clone(),
            config.api_key.clone(),
            model_name.to_string(),
            system_prompt,
        );

        let client = Arc::new(client);
        self.cache.insert(key, Arc::clone(&client));
        Ok(client)
    }
}

fn system_prompt_of(client: &AnyProviderClient) -> Option<&String> {
    match client {
        AnyProviderClient::Claude(c) => c.system_prompt.as_ref(),
        AnyProviderClient::Codex(c) => c.system_prompt.as_ref(),
        AnyProviderClient::Gemini(c) => c.system_prompt.as_ref(),
        AnyProviderClient::OpenAI(c) => c.system_prompt.as_ref(),
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderClientFactory;
    use crate::config::ProviderConfig;

    #[test]
    fn factory_reuses_client_for_same_provider_model_and_prompt() {
        let configs = vec![ProviderConfig {
            name: "openai".to_string(),
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
