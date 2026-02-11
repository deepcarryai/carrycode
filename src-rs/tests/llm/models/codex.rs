use crate::llm::models::codex::{extract_sse_event, parse_codex_event, CodexEvent};
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_sse_event_splits_on_blank_line() {
        let mut buf = "event: text_delta\ndata: {\"text\":\"a\"}\n\nrest".to_string();
        let ev = extract_sse_event(&mut buf).expect("ev");
        assert!(ev.contains("event: text_delta"));
        assert_eq!(buf, "rest");
    }

    #[test]
    fn parse_codex_event_text_delta() {
        let raw = "event: text_delta\ndata: {\"text\":\"hi\"}\n";
        match parse_codex_event(raw).expect("event") {
            CodexEvent::TextDelta(t) => assert_eq!(t, "hi"),
            _ => panic!("unexpected"),
        }
    }

    #[test]
    fn parse_codex_event_tool_call() {
        let raw = "event: tool_call\ndata: {\"name\":\"grep\",\"args\":{\"pattern\":\"x\"}}\n";
        match parse_codex_event(raw).expect("event") {
            CodexEvent::ToolCall { name, args } => {
                assert_eq!(name, "grep");
                assert_eq!(args, json!({ "pattern": "x" }));
            }
            _ => panic!("unexpected"),
        }
    }
}