use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::pin::Pin;
use tokio_stream::Stream;

use crate::llm::models::provider_base::{Message, ProviderClient};

#[derive(Debug)]
pub(crate) enum CodexEvent {
    TextDelta(String),
    Thought(String),
    Diff { file: PathBuf, patch: String },
    ToolCall { name: String, args: Value },
    Done,
}

pub(crate) fn extract_sse_event(buffer: &mut String) -> Option<String> {
    if let Some(pos) = buffer.find("\n\n") {
        let event = buffer[..pos].to_string();
        buffer.drain(..pos + 2);
        Some(event)
    } else {
        None
    }
}

pub(crate) fn parse_codex_event(raw: &str) -> Option<CodexEvent> {
    let mut event_type: Option<&str> = None;
    let mut data: Option<&str> = None;

    for line in raw.lines() {
        if let Some(v) = line.strip_prefix("event:") {
            event_type = Some(v.trim());
        } else if let Some(v) = line.strip_prefix("data:") {
            data = Some(v.trim_start());
        }
    }

    let event_type = event_type?;
    let data = data?;

    if data == "[DONE]" {
        return Some(CodexEvent::Done);
    }

    let json: Value = serde_json::from_str(data).ok()?;

    match event_type {
        "text_delta" => Some(CodexEvent::TextDelta(
            json.get("text")?.as_str()?.to_string(),
        )),
        "thought" => Some(CodexEvent::Thought(
            json.get("content")?.as_str()?.to_string(),
        )),
        "diff" => Some(CodexEvent::Diff {
            file: json.get("file")?.as_str()?.into(),
            patch: json.get("patch")?.as_str()?.to_string(),
        }),
        "tool_call" => Some(CodexEvent::ToolCall {
            name: json.get("name")?.as_str()?.to_string(),
            args: json.get("args")?.clone(),
        }),
        "done" => Some(CodexEvent::Done),
        _ => None,
    }
}

fn codex_event_to_chunk(
    event: CodexEvent,
    tool_call_index: &mut usize,
) -> Option<Value> {
    match event {
        CodexEvent::TextDelta(t) => {
            if t.is_empty() {
                None
            } else {
                Some(json!({
                    "choices": [{
                        "delta": { "content": t }
                    }]
                }))
            }
        }
        CodexEvent::Thought(t) => {
            if t.is_empty() {
                None
            } else {
                Some(json!({
                    "choices": [{
                        "delta": { "reasoning_content": t }
                    }]
                }))
            }
        }
        CodexEvent::Diff { file, patch } => {
            let text = format!("--- {:?} ---\n{}", file, patch);
            Some(json!({
                "choices": [{
                    "delta": { "content": text }
                }]
            }))
        }
        CodexEvent::ToolCall { name, args } => {
            let idx = *tool_call_index;
            *tool_call_index += 1;
            Some(json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": idx,
                            "id": format!("codex_tool_{}", idx),
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": args.to_string()
                            }
                        }]
                    }
                }]
            }))
        }
        CodexEvent::Done => Some(json!({
            "choices": [{
                "delta": { "content": "" },
                "finish_reason": "stop"
            }]
        })),
    }
}

fn codex_url_candidates(api_base: &str) -> Vec<String> {
    let base = api_base.trim_end_matches('/');
    vec![
        format!("{}/v1/codex", base),
        format!("{}/v1/agent", base),
        format!("{}/codex", base),
        format!("{}/agent", base),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexClient {
    #[serde(rename = "base_url")]
    pub base_url: String,
    #[serde(rename = "api_key")]
    pub api_key: String,
    #[serde(rename = "model_name")]
    pub model_name: String,
    #[serde(rename = "system_prompt")]
    pub system_prompt: Option<String>,
}

impl CodexClient {
    pub fn new(base_url: String, api_key: String, model_name: String) -> Self {
        Self {
            base_url,
            api_key,
            model_name,
            system_prompt: None,
        }
    }

    pub fn with_system_prompt(mut self, prompt: Option<String>) -> Self {
        self.system_prompt = prompt;
        self
    }

    fn messages_to_task(&self, messages: Vec<Message>) -> String {
        let mut out = String::new();
        if let Some(sys) = &self.system_prompt {
            if !sys.trim().is_empty() {
                out.push_str(sys.trim());
                out.push_str("\n\n");
            }
        }

        for msg in messages {
            if msg.role == "system" {
                if !msg.content.trim().is_empty() {
                    out.push_str(msg.content.trim());
                    out.push_str("\n\n");
                }
                continue;
            }
            out.push_str(&format!("{}: {}\n", msg.role, msg.content));
        }
        out.trim().to_string()
    }
}

impl ProviderClient for CodexClient {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        _tools: Option<Vec<Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>> {
        let url_candidates = codex_url_candidates(&self.base_url);
        let task = self.messages_to_task(messages);

        let client = reqwest::Client::new();
        let mut last_err: Option<anyhow::Error> = None;
        let mut response_opt: Option<reqwest::Response> = None;
        for url in &url_candidates {
            let resp = client
                .post(url)
                .header("authorization", format!("Bearer {}", self.api_key))
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .json(&json!({
                    "stream": true,
                    "task": task,
                    "capabilities": { "diff": true, "tool_call": true }
                }))
                .send()
                .await;

            match resp {
                Ok(r) => {
                    response_opt = Some(r);
                    break;
                }
                Err(e) => last_err = Some(anyhow::anyhow!(e).context(format!("Failed to send request ({})", url))),
            }
        }

        let response = response_opt.ok_or_else(|| {
            last_err.unwrap_or_else(|| anyhow::anyhow!("Failed to send request to Codex API"))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Codex API error ({}): {}", status, text);
        }

        let stream = response.bytes_stream();
        let stream = tokio_stream::StreamExt::map(stream, |chunk| {
            chunk.context("Failed to read stream chunk")
        });

        let stream = Box::pin(async_stream::stream! {
            let mut raw_stream = stream;
            let mut buffer = String::new();
            let mut tool_call_index: usize = 0;

            while let Some(chunk_result) = tokio_stream::StreamExt::next(&mut raw_stream).await {
                let bytes = chunk_result?;
                buffer.push_str(&String::from_utf8_lossy(bytes.as_ref()));

                while let Some(event_raw) = extract_sse_event(&mut buffer) {
                    if let Some(event) = parse_codex_event(&event_raw) {
                        if let Some(chunk) = codex_event_to_chunk(event, &mut tool_call_index) {
                            let is_stop = chunk
                                .pointer("/choices/0/finish_reason")
                                .and_then(|v| v.as_str())
                                .is_some_and(|r| r == "stop");
                            yield Ok(chunk);
                            if is_stop {
                                return;
                            }
                        }
                    }
                }
            }

            yield Ok(json!({
                "choices": [{
                    "delta": { "content": "" },
                    "finish_reason": "stop"
                }]
            }));
        });

        Ok(stream)
    }

    async fn chat(&self, messages: Vec<Message>, tools: Option<Vec<Value>>) -> Result<Value> {
        let mut content = String::new();
        let mut stream = self.stream_chat(messages, tools).await?;
        while let Some(chunk) = tokio_stream::StreamExt::next(&mut stream).await {
            let chunk = chunk?;
            if let Some(t) = chunk.pointer("/choices/0/delta/content").and_then(|v| v.as_str()) {
                content.push_str(t);
            }
            if chunk
                .pointer("/choices/0/finish_reason")
                .and_then(|v| v.as_str())
                .is_some_and(|r| r == "stop")
            {
                break;
            }
        }
        Ok(json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": content
                }
            }]
        }))
    }
}

