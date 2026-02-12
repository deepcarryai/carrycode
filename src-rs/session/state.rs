use super::manager::SESSION_MANAGER;

use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi::Status;

use super::context::SessionEventSink;
use super::types::{CoreEvent, CoreEventType, ResponseStage, SessionToolOperation, CORE_EVENT_PROTOCOL_VERSION};

pub fn set_response_stage(session_id: &str, stage: ResponseStage) {
    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            if let Ok(mut guard) = ctx.response_stage.lock() {
                *guard = stage;
            }
        }
    }
}

pub fn set_tool_operation(session_id: &str, op: Option<SessionToolOperation>) {
    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            if let Ok(mut guard) = ctx.tool_operation.lock() {
                *guard = op;
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn set_event_sink(session_id: &str, sink: SessionEventSink) -> bool {
    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            if let Ok(mut guard) = ctx.event_sink.lock() {
                *guard = Some(sink);
            }
            if let Ok(mut seq) = ctx.event_seq.lock() {
                *seq = 0;
            }
            return true;
        }
    }
    false
}

pub fn clear_event_sink(session_id: &str) {
    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            if let Ok(mut guard) = ctx.event_sink.lock() {
                *guard = None;
            }
        }
    }
}

fn next_seq(session_id: &str) -> Option<i64> {
    let manager = SESSION_MANAGER.lock().ok()?;
    let ctx = manager.get(session_id)?;
    let mut guard = ctx.event_seq.lock().ok()?;
    *guard = guard.saturating_add(1);
    Some(*guard)
}

pub fn emit_stream_text(session_id: &str, text: String) {
    let seq = next_seq(session_id);
    let event = CoreEvent {
        protocol_version: CORE_EVENT_PROTOCOL_VERSION,
        session_id: session_id.to_string(),
        ts_ms: now_ms(),
        event_type: CoreEventType::Text,
        seq,
        text: Some(text),
        stage: None,
        tool_operation: None,
        tool_name: None,
        key_path: None,
        kind: None,
        args_summary: None,
        response_summary: None,
        display_text: None,
        success: None,
        confirm: None,
        error_message: None,
    };

    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            if let Ok(guard) = ctx.event_sink.lock() {
                if let Some(sink) = guard.as_ref() {
                    let _ = sink.handler.call(Ok(event), ThreadsafeFunctionCallMode::NonBlocking);
                }
            }
        }
    }
}

pub fn emit_control_event(session_id: &str, event: CoreEvent) {
    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            if let Ok(guard) = ctx.event_sink.lock() {
                if let Some(sink) = guard.as_ref() {
                    let mut event = event;
                    if event.seq.is_none() {
                        if let Ok(mut seq_guard) = ctx.event_seq.lock() {
                            *seq_guard = seq_guard.saturating_add(1);
                            event.seq = Some(*seq_guard);
                        }
                    }
                    let status =
                        sink.handler
                            .call(Ok(event.clone()), ThreadsafeFunctionCallMode::NonBlocking);
                    if status != Status::Ok {
                        let _ =
                            sink.handler.call(Ok(event), ThreadsafeFunctionCallMode::Blocking);
                    }
                }
            }
        }
    }
}
