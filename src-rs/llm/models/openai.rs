use anyhow::{Context, Result};
use serde_json::Value;
use std::pin::Pin;
use tokio_stream::Stream;

use crate::llm::models::provider_base::{Message, ProviderClient};

fn extract_sse_frame_from_buffer(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let mut delimiter_len = 0usize;
    let delimiter_pos = if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
        delimiter_len = 4;
        Some(pos)
    } else {
        buffer.windows(2).position(|w| w == b"\n\n").map(|pos| {
            delimiter_len = 2;
            pos
        })
    }?;

    let frame = buffer.drain(..delimiter_pos).collect::<Vec<u8>>();
    buffer.drain(..delimiter_len);
    Some(frame)
}

fn sse_data_from_frame(frame: &str) -> Option<String> {
    let mut data_parts: Vec<&str> = Vec::new();

    for raw_line in frame.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            data_parts.push(rest);
        }
    }

    if data_parts.is_empty() {
        return None;
    }
    Some(data_parts.join("\n"))
}

fn sse_data_stream<T>(
    stream: Pin<Box<dyn Stream<Item = Result<T>> + Send>>,
) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>>
where
    T: AsRef<[u8]> + Send + 'static,
{
    Box::pin(async_stream::stream! {
        let mut buffer: Vec<u8> = Vec::new();
        let mut stream = stream;
        while let Some(chunk_result) = tokio_stream::StreamExt::next(&mut stream).await {
            let bytes = chunk_result?;
            buffer.extend_from_slice(bytes.as_ref());

            while let Some(frame_bytes) = extract_sse_frame_from_buffer(&mut buffer) {
                let frame = String::from_utf8_lossy(&frame_bytes);
                if let Some(data) = sse_data_from_frame(&frame) {
                    yield Ok(data);
                }
            }
        }

        if !buffer.is_empty() {
            let frame = String::from_utf8_lossy(&buffer);
            if let Some(data) = sse_data_from_frame(&frame) {
                yield Ok(data);
            }
        }
    })
}

#[derive(Debug, Clone)]
pub struct OpenAiClient {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: Option<String>,
    http_client: reqwest::Client,
}

impl OpenAiClient {
    pub fn new(api_base: String, api_key: String, model: String) -> Self {
        Self {
            api_base,
            api_key,
            model,
            system_prompt: None,
            http_client: reqwest::Client::new(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: Option<String>) -> Self {
        self.system_prompt = prompt;
        self
    }

    fn apply_system_prompt(&self, messages: Vec<Message>) -> Vec<Message> {
        if let Some(prompt) = &self.system_prompt {
            let has_system = messages.iter().any(|m| m.role == "system");
            if !has_system {
                let mut final_messages = Vec::with_capacity(messages.len() + 1);
                final_messages.push(Message {
                    role: "system".to_string(),
                    content: prompt.clone(),
                });
                final_messages.extend(messages);
                return final_messages;
            }
        }
        messages
    }

    pub async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>> {
        let messages = self.apply_system_prompt(messages);
        let request_body = build_chat_completions_request_body(&self.model, messages, true, tools);
        let url_candidates = chat_completions_url_candidates(&self.api_base);

        let response = send_first_successful_chat_completions_request(
            &self.http_client,
            &url_candidates,
            &self.api_key,
            &request_body,
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error ({}): {}", status, error_text);
        }

        let stream = response.bytes_stream();
        let stream = tokio_stream::StreamExt::map(stream, |chunk| {
            chunk.context("Failed to read stream chunk")
        });
        let stream = sse_data_stream(Box::pin(stream));

        let stream = Box::pin(async_stream::stream! {
            let mut stream = stream;
            while let Some(data_result) = tokio_stream::StreamExt::next(&mut stream).await {
                let data = data_result?;
                if data.trim() == "[DONE]" {
                    break;
                }
                let json: Value = serde_json::from_str(&data)
                    .context("Failed to parse JSON from SSE data")?;
                yield Ok(json);
            }
        });

        Ok(stream)
    }

    #[allow(dead_code)]
    pub async fn chat(&self, messages: Vec<Message>, tools: Option<Vec<Value>>) -> Result<Value> {
        let messages = self.apply_system_prompt(messages);
        let request_body = build_chat_completions_request_body(&self.model, messages, false, tools);
        let url_candidates = chat_completions_url_candidates(&self.api_base);

        let response = send_first_successful_chat_completions_request(
            &self.http_client,
            &url_candidates,
            &self.api_key,
            &request_body,
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error ({}): {}", status, error_text);
        }

        let json: Value = response
            .json()
            .await
            .context("Failed to parse response JSON")?;

        Ok(json)
    }
}

impl ProviderClient for OpenAiClient {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>> {
        self.stream_chat(messages, tools).await
    }

    async fn chat(&self, messages: Vec<Message>, tools: Option<Vec<Value>>) -> Result<Value> {
        self.chat(messages, tools).await
    }
}

pub fn create_openai(
    base_url: String,
    api_key: String,
    model_name: String,
    system_prompt: Option<String>,
) -> OpenAiClient {
    OpenAiClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt)
}

pub fn create_zhipuai(
    base_url: String,
    api_key: String,
    model_name: String,
    system_prompt: Option<String>,
) -> OpenAiClient {
    OpenAiClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt)
}

pub fn create_qwen(
    base_url: String,
    api_key: String,
    model_name: String,
    system_prompt: Option<String>,
) -> OpenAiClient {
    OpenAiClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt)
}

pub fn create_deepseek(
    base_url: String,
    api_key: String,
    model_name: String,
    system_prompt: Option<String>,
) -> OpenAiClient {
    OpenAiClient::new(base_url, api_key, model_name).with_system_prompt(system_prompt)
}

fn build_chat_completions_request_body(
    model: &str,
    messages: Vec<Message>,
    stream: bool,
    tools: Option<Vec<Value>>,
) -> Value {
    let mut request_body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": stream,
    });
    if let Some(tools) = tools {
        request_body["tools"] = Value::Array(tools);
    }
    request_body
}

fn chat_completions_url_candidates(api_base: &str) -> Vec<String> {
    let base = api_base.trim_end_matches('/');
    let mut out = Vec::new();
    out.push(format!("{}/chat/completions", base));
    out.push(format!("{}/v1/chat/completions", base));
    out
}

async fn send_first_successful_chat_completions_request(
    http_client: &reqwest::Client,
    url_candidates: &[String],
    api_key: &str,
    request_body: &Value,
) -> Result<reqwest::Response> {
    let mut last_err: Option<anyhow::Error> = None;

    for url in url_candidates {
        let response = http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(request_body)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::NOT_FOUND {
                    last_err = Some(anyhow::anyhow!("LLM API endpoint not found: {}", url));
                    continue;
                }
                return Ok(resp);
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!(e).context(format!(
                    "Failed to send request to LLM API ({})",
                    url
                )));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Failed to send request to LLM API")))
}

#[cfg(test)]
mod tests {
    use super::{extract_sse_frame_from_buffer, sse_data_from_frame};

    #[test]
    fn sse_data_from_frame_supports_data_without_space() {
        let frame = "data:{\"x\":1}\n";
        let data = sse_data_from_frame(frame).expect("should extract data");
        assert_eq!(data, "{\"x\":1}");
    }

    #[test]
    fn sse_data_from_frame_joins_multiple_data_lines() {
        let frame = "event: message\ndata: a\ndata: b\n";
        let data = sse_data_from_frame(frame).expect("should extract data");
        assert_eq!(data, "a\nb");
    }

    #[test]
    fn extract_sse_frame_from_buffer_handles_crlf_delimiter() {
        let mut buffer = b"data: 1\r\n\r\ndata: 2\r\n\r\n".to_vec();

        let frame1 = extract_sse_frame_from_buffer(&mut buffer).expect("frame1");
        let data1 = sse_data_from_frame(&String::from_utf8_lossy(&frame1)).expect("data1");
        assert_eq!(data1, "1");

        let frame2 = extract_sse_frame_from_buffer(&mut buffer).expect("frame2");
        let data2 = sse_data_from_frame(&String::from_utf8_lossy(&frame2)).expect("data2");
        assert_eq!(data2, "2");
    }

    #[test]
    fn extract_sse_frame_from_buffer_handles_lf_delimiter() {
        let mut buffer = b"data: 1\n\ndata: 2\n\n".to_vec();

        let frame1 = extract_sse_frame_from_buffer(&mut buffer).expect("frame1");
        let data1 = sse_data_from_frame(&String::from_utf8_lossy(&frame1)).expect("data1");
        assert_eq!(data1, "1");

        let frame2 = extract_sse_frame_from_buffer(&mut buffer).expect("frame2");
        let data2 = sse_data_from_frame(&String::from_utf8_lossy(&frame2)).expect("data2");
        assert_eq!(data2, "2");
    }
}
