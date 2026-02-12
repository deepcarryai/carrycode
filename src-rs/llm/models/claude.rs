use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
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
        let mut line_acc: String = String::new();
        let mut stream = stream;
        let mut sse_chunk_count = 0u64;
        let mut emitted_count = 0u64;
        
        log::trace!("SSE parser started");
        
        while let Some(chunk_result) = tokio_stream::StreamExt::next(&mut stream).await {
            let bytes = chunk_result?;
            sse_chunk_count += 1;
            log::trace!("SSE parser received chunk #{}: {} bytes", sse_chunk_count, bytes.as_ref().len());
            buffer.extend_from_slice(bytes.as_ref());

            // First, try to parse by lines and emit single-line data events immediately
            while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let trimmed = line.trim_end_matches('\r').trim_end();
                log::trace!("SSE line: {:?}", trimmed);
                if trimmed.is_empty() {
                    if !line_acc.is_empty() {
                        emitted_count += 1;
                        log::debug!("SSE emitting event #{}: {} chars", emitted_count, line_acc.len());
                        yield Ok(line_acc.clone());
                        line_acc.clear();
                    }
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("data:") {
                    let rest = rest.strip_prefix(' ').unwrap_or(rest);
                    if !line_acc.is_empty() {
                        emitted_count += 1;
                        log::debug!("SSE emitting event #{}: {} chars", emitted_count, line_acc.len());
                        yield Ok(line_acc.clone());
                        line_acc.clear();
                    }
                    line_acc.push_str(rest);
                }
            }

            while let Some(frame_bytes) = extract_sse_frame_from_buffer(&mut buffer) {
                let frame = String::from_utf8_lossy(&frame_bytes);
                if let Some(data) = sse_data_from_frame(&frame) {
                    emitted_count += 1;
                    log::debug!("SSE emitting frame event #{}: {} chars", emitted_count, data.len());
                    yield Ok(data);
                }
            }
        }

        log::debug!("SSE stream ended. Remaining buffer: {} bytes, line_acc: {} chars", buffer.len(), line_acc.len());
        
        // 处理剩余的 buffer
        if !buffer.is_empty() {
            let frame = String::from_utf8_lossy(&buffer);
            log::debug!("SSE processing remaining buffer: {:?}", frame);
            
            // 首先尝试作为 SSE frame 解析
            if let Some(data) = sse_data_from_frame(&frame) {
                emitted_count += 1;
                log::debug!("SSE emitting final frame event #{}: {} chars", emitted_count, data.len());
                yield Ok(data);
            } else {
                // 如果不是 SSE 格式，直接把整个返回报文作为字符串返回给客户端
                let frame_trimmed = frame.trim();
                if !frame_trimmed.is_empty() {
                    emitted_count += 1;
                    log::debug!("SSE emitting raw response as event #{}: {} chars", emitted_count, frame_trimmed.len());
                    yield Ok(frame_trimmed.to_string());
                }
            }
        }
        
        // 处理剩余累积的行
        if !line_acc.is_empty() {
            emitted_count += 1;
            log::debug!("SSE emitting final line_acc event #{}: {} chars", emitted_count, line_acc.len());
            yield Ok(line_acc);
        }
        
        log::debug!("SSE parser finished. Total emitted: {} events", emitted_count);
    })
}

fn openai_tool_to_anthropic(tool: &Value) -> Option<Value> {
    if tool.get("type").and_then(|v| v.as_str()) != Some("function") {
        return None;
    }
    let function = tool.get("function")?;
    let name = function.get("name")?.as_str()?;
    let description = function
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let input_schema = function
        .get("parameters")
        .cloned()
        .unwrap_or_else(|| json!({ "type": "object" }));

    Some(json!({
        "name": name,
        "description": description,
        "input_schema": input_schema
    }))
}

fn tool_result_block_from_message_content(content: &str) -> Option<(String, Value)> {
    let prefix = "ToolResultJSON:";
    let rest = content.strip_prefix(prefix)?;
    let parsed: Value = serde_json::from_str(rest).ok()?;
    let tool_use_id = parsed.get("tool_use_id")?.as_str()?.to_string();
    let result = parsed.get("result").cloned().unwrap_or_else(|| json!({}));
    Some((tool_use_id, result))
}

fn extract_tool_calls_from_content(content: &str) -> Option<(String, Vec<Value>)> {
    if let Some(pos) = content.find("ToolCallsJSON:") {
        let text = content[..pos].trim().to_string();
        let json_str = &content[pos + "ToolCallsJSON:".len()..];
        if let Ok(calls) = serde_json::from_str::<Vec<Value>>(json_str) {
            return Some((text, calls));
        }
    }
    None
}

fn extract_text_from_anthropic_payload(v: &Value) -> Option<&str> {
    v.pointer("/delta/text")
        .and_then(|t| t.as_str())
        .or_else(|| v.pointer("/content_block/text").and_then(|t| t.as_str()))
        .or_else(|| v.pointer("/message/content/0/text").and_then(|t| t.as_str()))
        .or_else(|| v.pointer("/message/content/0/text/text").and_then(|t| t.as_str()))
}

fn unwrap_event_data(v: &Value) -> Option<Value> {
    if v.get("type").is_some() || v.get("choices").is_some() {
        return Some(v.clone());
    }
    if let Some(s) = v.get("data").and_then(|d| d.as_str()) {
        if let Ok(inner) = serde_json::from_str::<Value>(s) {
            return Some(inner);
        }
    }
    if let Some(obj) = v.get("data").and_then(|d| d.as_object()) {
        return Some(Value::Object(obj.clone()));
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeClient {
    #[serde(rename = "base_url")]
    pub base_url: String,
    #[serde(rename = "api_key")]
    pub api_key: String,
    #[serde(rename = "model_name")]
    pub model_name: String,
    #[serde(rename = "system_prompt")]
    pub system_prompt: Option<String>,
}

impl ClaudeClient {
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

impl ProviderClient for ClaudeClient {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<Value>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value>> + Send>>> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));

        let mut anthropic_messages = Vec::new();
        let mut system_prompt = self.system_prompt.clone();

        for msg in messages {
            if msg.role == "system" {
                if system_prompt.is_none() {
                    system_prompt = Some(msg.content.clone());
                } else {
                    system_prompt = Some(format!("{}\n{}", system_prompt.unwrap(), msg.content));
                }
            } else {
                if let Some((tool_use_id, result)) = tool_result_block_from_message_content(&msg.content) {
                    let block = json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": result.to_string()
                    });
                    
                    let last_is_user = anthropic_messages.last().map(|m: &Value| m["role"] == "user").unwrap_or(false);
                    if last_is_user {
                         let last = anthropic_messages.last_mut().unwrap();
                         if !last["content"].is_array() {
                             let text = last["content"].as_str().unwrap_or("").to_string();
                             last["content"] = json!([{ "type": "text", "text": text }]);
                         }
                         last["content"].as_array_mut().unwrap().push(block);
                    } else {
                        anthropic_messages.push(json!({
                            "role": "user",
                            "content": [block]
                        }));
                    }
                } else if msg.role == "assistant" {
                    if let Some((text, calls)) = extract_tool_calls_from_content(&msg.content) {
                        let mut content_arr = Vec::new();
                        if !text.is_empty() {
                            content_arr.push(json!({ "type": "text", "text": text }));
                        }
                        for call in calls {
                            let args_str = call.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                            let input_obj: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                            
                            content_arr.push(json!({
                                "type": "tool_use",
                                "id": call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                "name": call.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                "input": input_obj
                            }));
                        }
                        anthropic_messages.push(json!({
                            "role": "assistant",
                            "content": content_arr
                        }));
                    } else {
                        anthropic_messages.push(json!({
                            "role": "assistant",
                            "content": msg.content
                        }));
                    }
                } else {
                    if msg.role == "user" {
                        let last_is_user = anthropic_messages.last().map(|m: &Value| m["role"] == "user").unwrap_or(false);
                        if last_is_user {
                             let last = anthropic_messages.last_mut().unwrap();
                             if !last["content"].is_array() {
                                 let text = last["content"].as_str().unwrap_or("").to_string();
                                 last["content"] = json!([{ "type": "text", "text": text }]);
                             }
                             last["content"].as_array_mut().unwrap().push(json!({
                                 "type": "text",
                                 "text": msg.content
                             }));
                             continue;
                        }
                    }
                    anthropic_messages.push(json!({
                        "role": msg.role,
                        "content": msg.content
                    }));
                }
            }
        }

        let mut request_body = json!({
            "model": self.model_name,
            "messages": anthropic_messages,
            "stream": true,
            "max_tokens": 1024
        });

        if let Some(sys) = system_prompt {
            request_body["system"] = json!(sys);
        }

        if let Some(tools) = tools {
            let converted: Vec<Value> = tools.iter().filter_map(openai_tool_to_anthropic).collect();
            if !converted.is_empty() {
                request_body["tools"] = Value::Array(converted);
                request_body["tool_choice"] = json!({ "type": "auto" });
            }
        }

        // 配置超时: 连接超时30秒，初始请求超时60秒
        // 注意: reqwest 的 timeout() 只对初始请求有效，对流读取无效
        // 流读取的超时需要在下面单独处理
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(60))
            .tcp_keepalive(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .context("Failed to build HTTP client")?;
        
        let response = client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("accept", "text/event-stream")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Anthropic API (possible timeout or network error)")?;

        log::debug!("StreamChat, Claude Response: {:?}", response);

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            log::error!("StreamChat, Claude API error ({}): {}", status, error_text);
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }

        log::debug!("Response headers: content-type={:?}, transfer-encoding={:?}, content-length={:?}",
            response.headers().get("content-type"),
            response.headers().get("transfer-encoding"),
            response.headers().get("content-length")
        );

        // 为流读取添加超时保护
        // 每个 chunk 的读取超时为 60 秒，适合长时间思考的 LLM 响应
        let stream = response.bytes_stream();
        let stream = Box::pin(async_stream::stream! {
            let mut byte_stream = stream;
            let chunk_timeout = std::time::Duration::from_secs(60);
            let mut chunk_num = 0u64;
            
            log::debug!("Starting to read SSE stream from Claude API...");
            
            loop {
                log::trace!("Waiting for chunk #{} (timeout: {}s)...", chunk_num + 1, chunk_timeout.as_secs());
                
                match tokio::time::timeout(
                    chunk_timeout,
                    tokio_stream::StreamExt::next(&mut byte_stream)
                ).await {
                    Ok(Some(Ok(bytes))) => {
                        chunk_num += 1;
                        log::debug!("Received chunk #{}: {} bytes", chunk_num, bytes.len());
                        // 打印原始内容用于调试
                        if chunk_num <= 3 {
                            let content = String::from_utf8_lossy(&bytes);
                            log::debug!("Chunk #{} content: {:?}", chunk_num, content);
                        }
                        yield Ok(bytes);
                    }
                    Ok(Some(Err(e))) => {
                        log::error!("Stream read error on chunk #{}: {}", chunk_num + 1, e);
                        yield Err(anyhow::anyhow!("Stream read error: {}", e));
                        break;
                    }
                    Ok(None) => {
                        // 流正常结束
                        log::debug!("Stream ended normally after {} chunks", chunk_num);
                        break;
                    }
                    Err(_) => {
                        // 超时
                        log::error!(
                            "Stream chunk read timeout after {} seconds while waiting for chunk #{}. Received {} chunks total.",
                            chunk_timeout.as_secs(),
                            chunk_num + 1,
                            chunk_num
                        );
                        yield Err(anyhow::anyhow!(
                            "Stream chunk read timeout after {} seconds. The API may be unresponsive or the connection was lost.",
                            chunk_timeout.as_secs()
                        ));
                        break;
                    }
                }
            }
        });
        
        let stream = sse_data_stream(stream);

        let stream = Box::pin(async_stream::stream! {
            let mut stream = stream;
            let mut emitted_any = false;
            let mut saw_any_event = false;
            let mut parse_errors: u64 = 0;
            let mut unhandled_types: u64 = 0;
            let mut last_seen_type: Option<String> = None;
            let mut tool_index_counter: usize = 0;
            let mut tool_by_block_index: HashMap<u64, (usize, String, String)> = HashMap::new();
            let mut chunk_count: u64 = 0;
            let mut total_bytes: usize = 0;
            while let Some(data_result) = tokio_stream::StreamExt::next(&mut stream).await {
                chunk_count += 1;
                let data = data_result?;
                total_bytes += data.len();
                let data_trimmed = data.trim();
                
                if chunk_count <= 5 || chunk_count % 100 == 0 {
                    log::debug!("Claude SSE chunk #{}: {} bytes (total: {} bytes)", chunk_count, data.len(), total_bytes);
                }
                if data_trimmed == "[DONE]" {
                    emitted_any = true;
                    yield Ok(json!({
                        "choices": [{
                            "delta": { "content": "" },
                            "finish_reason": "stop"
                        }]
                    }));
                    break;
                }

                let parsed: Value = match serde_json::from_str(data_trimmed) {
                    Ok(v) => v,
                    Err(_) => {
                        parse_errors += 1;
                        if parse_errors == 1 {
                            log::debug!("Claude SSE JSON parse failed (len={})", data_trimmed.len());
                        }
                        if !data_trimmed.is_empty() && data_trimmed != "[DONE]" {
                            saw_any_event = true;
                            emitted_any = true;
                            yield Ok(json!({
                                "choices": [{
                                    "delta": { "content": data_trimmed }
                                }]
                            }));
                        }
                        continue;
                    }
                };

                let parsed = unwrap_event_data(&parsed).unwrap_or(parsed);
                saw_any_event = true;

                if parsed.get("choices").is_some() {
                    emitted_any = true;
                    yield Ok(parsed);
                    continue;
                }

                let event_type = parsed.get("type").and_then(|s| s.as_str());
                if let Some(t) = event_type {
                    last_seen_type = Some(t.to_string());
                }

                match event_type {
                    Some("message_start") => {
                        if let Some(text) = extract_text_from_anthropic_payload(&parsed) {
                            if !text.is_empty() {
                                emitted_any = true;
                                yield Ok(json!({
                                    "choices": [{
                                        "delta": { "content": text }
                                    }]
                                }));
                            }
                        }
                    }
                    Some("content_block_start") => {
                        let block_index = parsed
                            .get("index")
                            .and_then(|v| v.as_u64())
                            .or_else(|| parsed.get("content_block_index").and_then(|v| v.as_u64()))
                            .unwrap_or(0);
                        let block_type = parsed
                            .pointer("/content_block/type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if block_type == "text" {
                            if let Some(text) = parsed.pointer("/content_block/text").and_then(|v| v.as_str()) {
                                if !text.is_empty() {
                                    emitted_any = true;
                                    yield Ok(json!({
                                        "choices": [{
                                            "delta": { "content": text }
                                        }]
                                    }));
                                }
                            }
                        } else if block_type == "tool_use" {
                            let tool_use_id = parsed
                                .pointer("/content_block/id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let tool_name = parsed
                                .pointer("/content_block/name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let idx = tool_index_counter;
                            tool_index_counter += 1;
                            tool_by_block_index.insert(block_index, (idx, tool_use_id.clone(), tool_name.clone()));

                            emitted_any = true;
                            yield Ok(json!({
                                "choices": [{
                                    "delta": {
                                        "tool_calls": [{
                                            "index": idx,
                                            "id": tool_use_id,
                                            "type": "function",
                                            "function": {
                                                "name": tool_name,
                                                "arguments": ""
                                            }
                                        }]
                                    }
                                }]
                            }));

                            if let Some(input) = parsed.pointer("/content_block/input") {
                                // 如果 input 是空对象，不要发送，因为后续会有 content_block_delta
                                if let Some(obj) = input.as_object() {
                                    if obj.is_empty() {
                                        log::debug!("Ignoring empty input object in content_block_start");
                                        continue;
                                    }
                                }
                                
                                let args = if input.is_string() {
                                    input.as_str().unwrap_or("").to_string()
                                } else {
                                    input.to_string()
                                };
                                
                                if args != "null" && !args.is_empty() && args != "{}" {
                                    emitted_any = true;
                                    yield Ok(json!({
                                        "choices": [{
                                            "delta": {
                                                "tool_calls": [{
                                                    "index": idx,
                                                    "id": tool_use_id,
                                                    "type": "function",
                                                    "function": {
                                                        "name": tool_name,
                                                        "arguments": args
                                                    }
                                                }]
                                            }
                                        }]
                                    }));
                                }
                            }
                        }
                    }
                    Some("content_block_delta") => {
                        if let Some(text) = parsed.pointer("/delta/text").and_then(|t| t.as_str()) {
                            if !text.is_empty() {
                                emitted_any = true;
                                yield Ok(json!({
                                    "choices": [{
                                        "delta": { "content": text }
                                    }]
                                }));
                            }
                        }

                        let partial_json = parsed
                            .pointer("/delta/partial_json")
                            .and_then(|v| v.as_str())
                            .or_else(|| parsed.pointer("/delta/input_json_delta").and_then(|v| v.as_str()));
                        if let Some(args_chunk) = partial_json {
                            if !args_chunk.is_empty() {
                                let block_index = parsed
                                    .get("index")
                                    .and_then(|v| v.as_u64())
                                    .or_else(|| parsed.get("content_block_index").and_then(|v| v.as_u64()))
                                    .unwrap_or(0);
                                if let Some((idx, tool_use_id, tool_name)) = tool_by_block_index.get(&block_index) {
                                    emitted_any = true;
                                    yield Ok(json!({
                                        "choices": [{
                                            "delta": {
                                                "tool_calls": [{
                                                    "index": *idx,
                                                    "id": tool_use_id,
                                                    "type": "function",
                                                    "function": {
                                                        "name": tool_name,
                                                        "arguments": args_chunk
                                                    }
                                                }]
                                            }
                                        }]
                                    }));
                                }
                            }
                        }
                    }
                    Some("content_block_stop") => {
                        // 忽略此事件，这是正常的块结束信号
                    }
                    Some("message_delta") => {
                        if parsed.pointer("/delta/stop_reason").is_some_and(|v| !v.is_null()) {
                            emitted_any = true;
                            yield Ok(json!({
                                "choices": [{
                                    "delta": { "content": "" },
                                    "finish_reason": "stop"
                                }]
                            }));
                            break;
                        }
                    }
                    Some("message_stop") => {
                        emitted_any = true;
                        yield Ok(json!({
                            "choices": [{
                                "delta": { "content": "" },
                                "finish_reason": "stop"
                            }]
                        }));
                        break;
                    }
                    Some("error") => {
                        // 处理 Anthropic 标准错误 或 代理服务器错误
                        let msg = parsed
                            .pointer("/error/message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown API error");
                        
                        let error_type = parsed
                            .pointer("/error/type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("error");
                        
                        // 构建用户友好的错误消息
                        let error_content = format!("\n\n⚠️ **API Error** ({}): {}\n", error_type, msg);
                        
                        log::warn!("Claude API error: type={}, message={}", error_type, msg);
                        
                        // 将错误消息作为正常内容输出
                        emitted_any = true;
                        yield Ok(json!({
                            "choices": [{
                                "delta": { "content": error_content }
                            }]
                        }));
                        
                        // 然后发送 stop 信号正常结束
                        yield Ok(json!({
                            "choices": [{
                                "delta": { "content": "" },
                                "finish_reason": "stop"
                            }]
                        }));
                        break;
                    }
                    _ => {
                        unhandled_types += 1;
                        if unhandled_types <= 3 {
                            let t = event_type.unwrap_or("unknown");
                            log::debug!("Unhandled Claude SSE event type: {}", t);
                        }

                        if let Some(text) = extract_text_from_anthropic_payload(&parsed) {
                            if !text.is_empty() {
                                emitted_any = true;
                                yield Ok(json!({
                                    "choices": [{
                                        "delta": { "content": text }
                                    }]
                                }));
                            }
                        }
                    }
                }
            }

            log::debug!(
                "Claude stream ended: emitted_any={}, saw_any_event={}, chunks={}, bytes={}, parse_errors={}, unhandled={}",
                emitted_any, saw_any_event, chunk_count, total_bytes, parse_errors, unhandled_types
            );
            
            if !emitted_any {
                let t = last_seen_type.unwrap_or_else(|| "unknown".to_string());
                if saw_any_event {
                    log::warn!("Claude stream had events but nothing was emitted, sending stop signal");
                    yield Ok(json!({
                        "choices": [{
                            "delta": { "content": "" },
                            "finish_reason": "stop"
                        }]
                    }));
                } else {
                    let error_msg = if chunk_count == 0 {
                        format!(
                            "Claude stream ended without any chunks. Possible causes: API timeout, network interruption, or API error. Last event type: {}",
                            t
                        )
                    } else if total_bytes == 0 {
                        format!(
                            "Claude stream received {} empty chunks. Possible causes: API returned no data, or connection was closed prematurely. Last event type: {}",
                            chunk_count, t
                        )
                    } else {
                        format!(
                            "Claude stream ended without usable chunks (received {} chunks, {} bytes, {} parse errors). Last event type: {}. This may indicate API timeout or malformed response.",
                            chunk_count, total_bytes, parse_errors, t
                        )
                    };
                    log::error!("{}", error_msg);
                    yield Err(anyhow::anyhow!(error_msg));
                }
            }
        });

        Ok(stream)
    }

    async fn chat(&self, messages: Vec<Message>, tools: Option<Vec<Value>>) -> Result<Value> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));

        let mut anthropic_messages = Vec::new();
        let mut system_prompt = self.system_prompt.clone();

        for msg in messages {
            if msg.role == "system" {
                if system_prompt.is_none() {
                    system_prompt = Some(msg.content.clone());
                } else {
                    system_prompt = Some(format!("{}\n{}", system_prompt.unwrap(), msg.content));
                }
            } else {
                if let Some((tool_use_id, result)) = tool_result_block_from_message_content(&msg.content) {
                    let block = json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": result.to_string()
                    });
                    
                    let last_is_user = anthropic_messages.last().map(|m: &Value| m["role"] == "user").unwrap_or(false);
                    if last_is_user {
                         let last = anthropic_messages.last_mut().unwrap();
                         if !last["content"].is_array() {
                             let text = last["content"].as_str().unwrap_or("").to_string();
                             last["content"] = json!([{ "type": "text", "text": text }]);
                         }
                         last["content"].as_array_mut().unwrap().push(block);
                    } else {
                        anthropic_messages.push(json!({
                            "role": "user",
                            "content": [block]
                        }));
                    }
                } else if msg.role == "assistant" {
                    if let Some((text, calls)) = extract_tool_calls_from_content(&msg.content) {
                        let mut content_arr = Vec::new();
                        if !text.is_empty() {
                            content_arr.push(json!({ "type": "text", "text": text }));
                        }
                        for call in calls {
                            let args_str = call.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                            let input_obj: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                            
                            content_arr.push(json!({
                                "type": "tool_use",
                                "id": call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                "name": call.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                "input": input_obj
                            }));
                        }
                        anthropic_messages.push(json!({
                            "role": "assistant",
                            "content": content_arr
                        }));
                    } else {
                        anthropic_messages.push(json!({
                            "role": "assistant",
                            "content": msg.content
                        }));
                    }
                } else {
                    if msg.role == "user" {
                        let last_is_user = anthropic_messages.last().map(|m: &Value| m["role"] == "user").unwrap_or(false);
                        if last_is_user {
                             let last = anthropic_messages.last_mut().unwrap();
                             if !last["content"].is_array() {
                                 let text = last["content"].as_str().unwrap_or("").to_string();
                                 last["content"] = json!([{ "type": "text", "text": text }]);
                             }
                             last["content"].as_array_mut().unwrap().push(json!({
                                 "type": "text",
                                 "text": msg.content
                             }));
                             continue;
                        }
                    }
                    anthropic_messages.push(json!({
                        "role": msg.role,
                        "content": msg.content
                    }));
                }
            }
        }

        let mut request_body = json!({
            "model": self.model_name,
            "messages": anthropic_messages,
            "max_tokens": 4096
        });

        if let Some(sys) = system_prompt {
            request_body["system"] = json!(sys);
        }

        if let Some(tools) = tools {
            let converted: Vec<Value> = tools.iter().filter_map(openai_tool_to_anthropic).collect();
            if !converted.is_empty() {
                request_body["tools"] = Value::Array(converted);
                request_body["tool_choice"] = json!({ "type": "auto" });
            }
        }

        // 非流式请求也需要超时配置
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .context("Failed to build HTTP client")?;
        
        let response = client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Anthropic API (possible timeout or network error)")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Claude API error ({}): {}", status, error_text);
        }

        let json: Value = response.json().await?;

        let content = json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
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
    use super::{
        extract_sse_frame_from_buffer, openai_tool_to_anthropic, sse_data_from_frame,
        tool_result_block_from_message_content,
    };
    use serde_json::json;

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

    #[test]
    fn openai_tool_to_anthropic_converts_schema() {
        let openai_tool = json!({
            "type": "function",
            "function": {
                "name": "grep",
                "description": "search",
                "parameters": {
                    "type": "object",
                    "properties": { "pattern": { "type": "string" } }
                }
            }
        });
        let anthropic_tool = openai_tool_to_anthropic(&openai_tool).expect("tool");
        assert_eq!(anthropic_tool.get("name").and_then(|v| v.as_str()), Some("grep"));
        assert!(anthropic_tool.get("input_schema").is_some());
    }

    #[test]
    fn tool_result_block_from_message_content_parses_prefix() {
        let payload = json!({
            "tool_use_id": "toolu_123",
            "result": { "ok": true }
        });
        let s = format!("ToolResultJSON:{}", payload.to_string());
        let (id, result) = tool_result_block_from_message_content(&s).expect("parsed");
        assert_eq!(id, "toolu_123");
        assert_eq!(result.get("ok").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn unwrap_event_data_handles_data_string() {
        let outer = json!({
            "event": "message",
            "data": "{\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}"
        });
        let inner = super::unwrap_event_data(&outer).expect("inner");
        assert_eq!(
            inner.pointer("/delta/text").and_then(|v| v.as_str()),
            Some("hi")
        );
    }
}
