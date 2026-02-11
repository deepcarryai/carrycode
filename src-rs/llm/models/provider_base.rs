use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use tokio_stream::Stream;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

#[allow(async_fn_in_trait)]
pub trait ProviderClient: Send + Sync {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>>;

    #[allow(dead_code)]
    async fn chat(&self, messages: Vec<Message>, tools: Option<Vec<Value>>) -> Result<Value>;
}

