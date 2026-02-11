use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::pin::Pin;
use tokio_stream::Stream;

use crate::llm::models::provider_base::{Message, ProviderClient};

pub(crate) fn extract_sse_frame_from_buffer(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
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

pub(crate) fn sse_data_from_frame(frame: &str) -> Option<String> {
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
                    reasoning_content: None,
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
    let mut final_prompt = system_prompt;
    if let Some(extra) = super::prompt_extra::get_extra_prompt_for_provider("zhipuai") {
        let mut p = final_prompt.unwrap_or_default();
        p.push_str(extra);
        final_prompt = Some(p);
    }
    OpenAiClient::new(base_url, api_key, model_name).with_system_prompt(final_prompt)
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
    let converted_messages: Vec<Value> = messages
        .into_iter()
        .map(|msg| {
            let mut m = json!({
                "role": msg.role,
                "content": msg.content.clone(),
            });

            if let Some(reasoning) = &msg.reasoning_content {
                m["reasoning_content"] = json!(reasoning);
            }

            if msg.role == "user" && msg.content.starts_with("ToolResult:\n") {
                let json_str = &msg.content["ToolResult:\n".len()..];
                if let Ok(res_val) = serde_json::from_str::<Value>(json_str) {
                    let tool_call_id = res_val.get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    
                    // OpenAI tool message expects the result in the 'content' field.
                    // We'll use the stdout/stderr or the full JSON if those are missing.
                    let tool_output = res_val.get("stdout").and_then(|v| v.as_str())
                        .or_else(|| res_val.get("stderr").and_then(|v| v.as_str()))
                        .unwrap_or(json_str);

                    return json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": tool_output,
                    });
                }
            }

            if msg.role == "assistant" {
                let start_tag = "<agent_tool_calls_json>";
                let end_tag = "</agent_tool_calls_json>";
                
                if let Some(start_pos) = msg.content.find(start_tag) {
                    let text_before = msg.content[..start_pos].trim();
                    let rest = &msg.content[start_pos + start_tag.len()..];
                    
                    let (json_str, text_after) = if let Some(end_pos) = rest.find(end_tag) {
                        (&rest[..end_pos], rest[end_pos + end_tag.len()..].trim())
                    } else {
                        (rest, "")
                    };

                    if let Ok(calls) = serde_json::from_str::<Vec<Value>>(json_str.trim()) {
                        let mut openai_tool_calls = Vec::new();
                        for call in calls {
                            let id = call.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let name = call.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let args = call.get("arguments");
                            
                            let args_str = if let Some(Value::String(s)) = args {
                                s.clone()
                            } else if let Some(v) = args {
                                v.to_string()
                            } else {
                                "{}".to_string()
                            };

                            openai_tool_calls.push(json!({
                                "id": if id.is_empty() { format!("call_{}", name) } else { id.to_string() },
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": args_str
                                }
                            }));
                        }

                        if !openai_tool_calls.is_empty() {
                            m["tool_calls"] = serde_json::Value::Array(openai_tool_calls);
                            
                            // Join text before and after, removing the internal tags
                            let final_content = if !text_before.is_empty() && !text_after.is_empty() {
                                format!("{}\n\n{}", text_before, text_after)
                            } else if !text_before.is_empty() {
                                text_before.to_string()
                            } else {
                                text_after.to_string()
                            };
                            
                            m["content"] = serde_json::Value::String(final_content);
                        }
                    }
                }
            }
            m
        })
        .collect();

    let mut request_body = serde_json::json!({
        "model": model,
        "messages": converted_messages,
        "stream": stream,
    });
    if let Some(tools) = tools {
        request_body["tools"] = Value::Array(tools);
        if request_body.get("tool_choice").is_none() {
            request_body["tool_choice"] = json!("auto");
        }
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

