use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::pin::Pin;
use std::time::Duration;
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

fn stream_value_from_gemini_event(event: &Value) -> Option<Value> {
    if event.get("choices").is_some() {
        return Some(event.clone());
    }

    let finish_reason = event
        .get("finishReason")
        .and_then(|v| v.as_str())
        .or_else(|| event.get("finish_reason").and_then(|v| v.as_str()));
    if finish_reason.is_some_and(|r| r.eq_ignore_ascii_case("stop")) {
        return Some(json!({
            "choices": [{
                "delta": { "content": "" },
                "finish_reason": "stop"
            }]
        }));
    }

    let text = event
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if text.is_empty() {
        return None;
    }

    Some(json!({
        "choices": [{
            "delta": { "content": text }
        }]
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiClient {
    #[serde(rename = "base_url")]
    pub base_url: String,
    #[serde(rename = "api_key")]
    pub api_key: String,
    #[serde(rename = "model_name")]
    pub model_name: String,
    #[serde(rename = "system_prompt")]
    pub system_prompt: Option<String>,
}

impl GeminiClient {
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
}

impl ProviderClient for GeminiClient {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        _tools: Option<Vec<Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>> {
        let url = format!(
            "{}/models/{}:streamGenerateContent?key={}",
            self.base_url.trim_end_matches('/'),
            self.model_name,
            self.api_key
        );

        let mut contents = Vec::new();
        let mut system_instruction = None;

        for msg in messages {
            if msg.role == "system" {
                system_instruction = Some(json!({
                    "parts": [{ "text": msg.content }]
                }));
            } else {
                let role = if msg.role == "assistant" { "model" } else { "user" };
                contents.push(json!({
                    "role": role,
                    "parts": [{ "text": msg.content }]
                }));
            }
        }

        let mut request_body = json!({
            "contents": contents
        });

        if let Some(sys) = system_instruction {
            request_body["systemInstruction"] = sys;
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .context("Failed to build HTTP client")?;
        let response = client
            .post(&url)
            .header("accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Gemini API error ({}): {}", status, error_text);
        }

        let stream = response.bytes_stream();
        let stream = tokio_stream::StreamExt::map(stream, |chunk| {
            chunk.context("Failed to read stream chunk")
        });

        let stream = Box::pin(async_stream::stream! {
            let mut raw_stream = stream;
            let mut buffer: Vec<u8> = Vec::new();

            while let Some(chunk_result) = tokio_stream::StreamExt::next(&mut raw_stream).await {
                let bytes = chunk_result?;
                buffer.extend_from_slice(bytes.as_ref());

                while let Some(frame_bytes) = extract_sse_frame_from_buffer(&mut buffer) {
                    let frame = String::from_utf8_lossy(&frame_bytes);
                    let Some(data) = sse_data_from_frame(&frame) else {
                        continue;
                    };

                    let data_trimmed = data.trim();
                    if data_trimmed.is_empty() {
                        continue;
                    }
                    if data_trimmed == "[DONE]" {
                        yield Ok(json!({
                            "choices": [{
                                "delta": { "content": "" },
                                "finish_reason": "stop"
                            }]
                        }));
                        return;
                    }

                    let parsed: Value = match serde_json::from_str(data_trimmed) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    match parsed {
                        Value::Array(arr) => {
                            for event in arr {
                                if let Some(out) = stream_value_from_gemini_event(&event) {
                                    let is_stop = out
                                        .pointer("/choices/0/finish_reason")
                                        .and_then(|v| v.as_str())
                                        .is_some_and(|r| r == "stop");
                                    yield Ok(out);
                                    if is_stop {
                                        return;
                                    }
                                }
                            }
                        }
                        _ => {
                            if let Some(out) = stream_value_from_gemini_event(&parsed) {
                                let is_stop = out
                                    .pointer("/choices/0/finish_reason")
                                    .and_then(|v| v.as_str())
                                    .is_some_and(|r| r == "stop");
                                yield Ok(out);
                                if is_stop {
                                    return;
                                }
                            }
                        }
                    }
                }

                while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                    let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let parsed: Value = match serde_json::from_str(line) {
                        Ok(v) => v,
                        Err(_) => {
                            buffer.splice(0..0, line_bytes.into_iter());
                            break;
                        }
                    };
                    match parsed {
                        Value::Array(arr) => {
                            for event in arr {
                                if let Some(out) = stream_value_from_gemini_event(&event) {
                                    let is_stop = out
                                        .pointer("/choices/0/finish_reason")
                                        .and_then(|v| v.as_str())
                                        .is_some_and(|r| r == "stop");
                                    yield Ok(out);
                                    if is_stop {
                                        return;
                                    }
                                }
                            }
                        }
                        _ => {
                            if let Some(out) = stream_value_from_gemini_event(&parsed) {
                                let is_stop = out
                                    .pointer("/choices/0/finish_reason")
                                    .and_then(|v| v.as_str())
                                    .is_some_and(|r| r == "stop");
                                yield Ok(out);
                                if is_stop {
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            if !buffer.is_empty() {
                if let Ok(text) = std::str::from_utf8(&buffer) {
                    let text = text.trim();
                    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                        match parsed {
                            Value::Array(arr) => {
                                for event in arr {
                                    if let Some(out) = stream_value_from_gemini_event(&event) {
                                        yield Ok(out);
                                    }
                                }
                            }
                            _ => {
                                if let Some(out) = stream_value_from_gemini_event(&parsed) {
                                    yield Ok(out);
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(stream)
    }

    async fn chat(&self, messages: Vec<Message>, _tools: Option<Vec<Value>>) -> Result<Value> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url.trim_end_matches('/'),
            self.model_name,
            self.api_key
        );

        let mut contents = Vec::new();
        let mut system_instruction = None;

        for msg in messages {
            if msg.role == "system" {
                system_instruction = Some(json!({
                    "parts": [{ "text": msg.content }]
                }));
            } else {
                let role = if msg.role == "assistant" { "model" } else { "user" };
                contents.push(json!({
                    "role": role,
                    "parts": [{ "text": msg.content }]
                }));
            }
        }

        let mut request_body = json!({
            "contents": contents
        });

        if let Some(sys) = system_instruction {
            request_body["systemInstruction"] = sys;
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .context("Failed to build HTTP client")?;
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Gemini API error ({}): {}", status, error_text);
        }

        let json: Value = response.json().await?;

        let content = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|cand| cand.get("content"))
            .and_then(|cont| cont.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default();

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

#[cfg(test)]
mod tests {
    use super::{extract_sse_frame_from_buffer, sse_data_from_frame, stream_value_from_gemini_event};
    use serde_json::json;

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
    fn sse_data_from_frame_joins_multiple_data_lines() {
        let frame = "event: message\ndata: a\ndata: b\n";
        let data = sse_data_from_frame(frame).expect("should extract data");
        assert_eq!(data, "a\nb");
    }

    #[test]
    fn stream_value_from_gemini_event_extracts_text() {
        let event = json!({
            "candidates": [{
                "content": { "parts": [{ "text": "hi" }] }
            }]
        });
        let out = stream_value_from_gemini_event(&event).expect("out");
        assert_eq!(out.pointer("/choices/0/delta/content").and_then(|v| v.as_str()), Some("hi"));
    }

    #[test]
    fn stream_value_from_gemini_event_handles_stop() {
        let event = json!({ "finishReason": "STOP" });
        let out = stream_value_from_gemini_event(&event).expect("out");
        assert_eq!(out.pointer("/choices/0/finish_reason").and_then(|v| v.as_str()), Some("stop"));
    }
}
