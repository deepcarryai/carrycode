use crate::llm::models::gemini::{extract_sse_frame_from_buffer, sse_data_from_frame, stream_value_from_gemini_event};
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(out.pointer("/choices/0/delta/content").and_then(|v: &serde_json::Value| v.as_str()), Some("hi"));
    }

    #[test]
    fn stream_value_from_gemini_event_handles_stop() {
        let event = json!({ "finishReason": "STOP" });
        let out = stream_value_from_gemini_event(&event).expect("out");
        assert_eq!(out.pointer("/choices/0/finish_reason").and_then(|v: &serde_json::Value| v.as_str()), Some("stop"));
    }
}