use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;

use crate::config::ProviderConfig;
use crate::cons::provider_cons::LLMProvider;

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

    #[allow(dead_code)]
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
    let provider_enum = LLMProvider::from_name(provider).unwrap_or(LLMProvider::OpenAI);

    match provider_enum {
        LLMProvider::Claude => AnyProviderClient::Claude(
            ClaudeClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt),
        ),
        LLMProvider::Codex => AnyProviderClient::Codex(
            CodexClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt),
        ),
        LLMProvider::Gemini => AnyProviderClient::Gemini(
            GeminiClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt),
        ),
        LLMProvider::OpenAI => AnyProviderClient::OpenAI(create_openai(
            base_url,
            api_key,
            model_name,
            system_prompt,
        )),
        LLMProvider::ZhipuAI => AnyProviderClient::OpenAI(create_zhipuai(
            base_url,
            api_key,
            model_name,
            system_prompt,
        )),
        LLMProvider::DeepSeek => AnyProviderClient::OpenAI(create_deepseek(
            base_url,
            api_key,
            model_name,
            system_prompt,
        )),
        LLMProvider::Qwen => AnyProviderClient::OpenAI(create_qwen(
            base_url,
            api_key,
            model_name,
            system_prompt,
        )),
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

        let mut found: Option<&ProviderConfig> = None;
        for c in provider_configs {
            if c.name != provider_name {
                continue;
            }
            if !c.models.iter().any(|m| m == model_name) {
                continue;
            }
            if found.is_some() {
                return Err(anyhow::anyhow!(
                    "Duplicate provider config for {}:{}",
                    provider_name,
                    model_name
                ));
            }
            found = Some(c);
        }

        let config = found.ok_or_else(|| {
            anyhow::anyhow!("Provider/model not configured: {}:{}", provider_name, model_name)
        })?;

        let provider_id = config.name.as_str();
        let client = create_client(
            provider_id,
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

