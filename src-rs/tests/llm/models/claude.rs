use crate::llm::models::claude::{extract_sse_frame_from_buffer, openai_tool_to_anthropic, sse_data_from_frame, tool_result_block_from_message_content, unwrap_event_data};
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(anthropic_tool.get("name").and_then(|v: &serde_json::Value| v.as_str()), Some("grep"));
        assert!(anthropic_tool.get("input_schema").is_some());
    }

    #[test]
    fn tool_result_block_from_message_content_parses_prefix() {
        let payload = json!({
            "tool_use_id": "toolu_123",
            "result": { "ok": true }
        });
        let s = format!("ToolResultJSON:{}", payload.to_string());
        let (id, result): (String, serde_json::Value) = tool_result_block_from_message_content(&s).expect("parsed");
        assert_eq!(id, "toolu_123");
        assert_eq!(result.get("ok").and_then(|v: &serde_json::Value| v.as_bool()), Some(true));
    }

    #[test]
    fn unwrap_event_data_handles_data_string() {
        let outer = json!({
            "event": "message",
            "data": "{\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}"
        });
        let inner = unwrap_event_data(&outer).expect("inner");
        assert_eq!(
            inner.pointer("/delta/text").and_then(|v: &serde_json::Value| v.as_str()),
            Some("hi")
        );
    }
}