use napi::bindgen_prelude::*;

use crate::config::{AppConfig, ProviderConfig, RuntimeSessionConfig};
use crate::session::context::{AgentMode, ApprovalMode};
use crate::llm::agents::agent::Agent as RustAgent;
use crate::llm::agents::agent::AgentResult as RustAgentResult;
use crate::llm::agents::agent::{StreamEvent, StreamStage};
use crate::llm::mcps::load_mcp_tools;
use crate::llm::models::provider_handle::Message;
use crate::llm::tools::list_available_tools;
use crate::llm::utils::network::measure_latency;
use crate::llm::tools::tool_trait::{Tool, ToolKind, ToolOperation as CoreToolOperation};
use crate::llm::utils::tool_access::{with_tool_access, ToolAccessLevel};
use crate::session::{
    approval_policy,
    emit_control_event,
    emit_stream_text,
    generate_request_id,
    get_confirmation_status,
    key_path_from_args,
    set_confirmation_status,
    set_response_stage,
    set_tool_operation,
    ConfirmationStatus,
    ResponseStage,
    SessionToolOperation,
    store,
    SESSION_MANAGER,
};
use crate::session::types::{
    CoreConfirmDecision,
    CoreConfirmationRequest,
    CoreEvent,
    CoreEventType,
    CORE_EVENT_PROTOCOL_VERSION,
};

use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub(crate) struct SessionOpenParts {
    pub(crate) inner: Arc<Mutex<RustAgent>>,
    pub(crate) session_id: String,
}

pub(crate) struct PendingConfirmation {
    pub(crate) request_id: String,
    pub(crate) sender: oneshot::Sender<String>,
}

fn map_tool_operation(op: CoreToolOperation) -> SessionToolOperation {
    match op {
        CoreToolOperation::Bash => SessionToolOperation::Bash,
        CoreToolOperation::Explored => SessionToolOperation::Explored,
        CoreToolOperation::Edited => SessionToolOperation::Edited,
        CoreToolOperation::Todo => SessionToolOperation::Todo,
        CoreToolOperation::Other => SessionToolOperation::Explored,
    }
}

fn truncate_utf8_with_ellipsis(s: &str, max_bytes: usize) -> String {
    let s = s.trim();
    if s.len() <= max_bytes {
        return s.to_string();
    }

    let mut end = 0usize;
    for (i, ch) in s.char_indices() {
        let next = i + ch.len_utf8();
        if next <= max_bytes {
            end = next;
        } else {
            break;
        }
    }

    if end == 0 {
        let first_end = s.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
        end = std::cmp::min(first_end, s.len());
    }

    format!("{}...", &s[..end])
}

fn session_op_str(op: SessionToolOperation) -> &'static str {
    match op {
        SessionToolOperation::Explored => "__EXPLORED__",
        SessionToolOperation::Edited => "__EDITED__",
        SessionToolOperation::Todo => "__TODO__",
        SessionToolOperation::Bash => "__BASH__",
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn session_snapshot(session_id: &str) -> serde_json::Value {
    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            let stage = ctx
                .response_stage
                .lock()
                .ok()
                .map(|v| *v)
                .map(|v| format!("{:?}", v))
                .unwrap_or_else(|| "Unknown".to_string());
            let tool_operation = ctx
                .tool_operation
                .lock()
                .ok()
                .and_then(|v| *v)
                .map(|v| format!("{:?}", v));
            let tool_confirm_len = ctx.tool_confirm.lock().ok().map(|m| m.len()).unwrap_or(0);

            return json!({
                "created_at": ctx.created_at,
                "updated_at": ctx.updated_at,
                "response_stage": stage,
                "tool_operation": tool_operation,
                "tool_confirm_len": tool_confirm_len
            });
        }
    }

    json!({})
}

fn log_session_event(session_id: &str, event: &str, extra: serde_json::Value) {
    let payload = json!({
        "ts": now_ms(),
        "event": event,
        "session_id": session_id,
        "session": session_snapshot(session_id),
        "extra": extra
    });
    log::info!(target: "carrycode_session", "{}", payload.to_string());
}

fn system_prompt_for_agent_mode(config: &AppConfig, agent_mode: &AgentMode) -> Option<String> {
    match agent_mode {
        AgentMode::Plan => config
            .prompt_plan
            .clone()
            .and_then(|p| if p.enabled { Some(p.prompt_template) } else { None })
            .or_else(|| Some("You are a planning assistant.".to_string())),
        AgentMode::Build => config
            .prompt_build
            .clone()
            .and_then(|p| if p.enabled { Some(p.prompt_template) } else { None })
            .or_else(|| Some("You are a helpful coding assistant.".to_string())),
    }
}

fn load_persisted_messages(session_id: &str) -> Option<Vec<Message>> {
    store::load_snapshot(session_id).ok().flatten().map(|s| s.messages)
}

fn persist_session_snapshot(session_id: &str, messages: Vec<Message>) -> Result<()> {
    let (agent_mode, approval_mode) = if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            (ctx.agent_mode.to_string(), ctx.approval_mode.to_string())
        } else {
            (AgentMode::default().to_string(), ApprovalMode::default().to_string())
        }
    } else {
        (AgentMode::default().to_string(), ApprovalMode::default().to_string())
    };

    store::save_snapshot(store::SessionSnapshot {
        version: store::SESSION_SNAPSHOT_VERSION,
        session_id: session_id.to_string(),
        created_at_ms: 0,
        updated_at_ms: 0,
        agent_mode,
        approval_mode,
        messages,
    })
    .map_err(|e| Error::from_reason(format!("Failed to persist session snapshot: {}", e)))
}

fn is_retryable_llm_error(e: &anyhow::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("failed to initiate llm stream")
        || msg.contains("failed to send request to llm api")
        || msg.contains("error sending request")
}

async fn execute_agent_with_retry(agent: &mut RustAgent) -> anyhow::Result<RustAgentResult> {
    const MAX_ATTEMPTS: usize = 3;
    for attempt in 1..=MAX_ATTEMPTS {
        match agent.execute().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt < MAX_ATTEMPTS && is_retryable_llm_error(&e) {
                    log::warn!(
                        "Agent execution failed (attempt {}/{}): {}",
                        attempt,
                        MAX_ATTEMPTS,
                        e
                    );
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                return Err(e);
            }
        }
    }
    unreachable!("loop returns on success or final failure")
}

pub(crate) fn open_session(session_id: String) -> Result<SessionOpenParts> {
    {
        let manager = SESSION_MANAGER
            .lock()
            .map_err(|_| Error::from_reason("Failed to lock session manager"))?;
        if let Some(ctx) = manager.get(&session_id) {
            let inner = Arc::clone(&ctx.inner);
            drop(manager);
            log_session_event(&session_id, "open_reuse", json!({}));
            return Ok(SessionOpenParts {
                inner,
                session_id,
            });
        }
    }

    crate::init_logger();
    let mut config = AppConfig::load().map_err(|e| Error::from_reason(format!("Failed to load config: {}", e)))?;

    // Determine AgentMode and ApprovalMode
    // 1. Try to find in runtime config
    let (agent_mode, approval_mode) = if let Some(session_config) = config.runtime.sessions.iter().find(|s| s.session_id == session_id) {
        (
            AgentMode::from(session_config.agent_mode.clone()),
            ApprovalMode::from(session_config.approval_mode.clone()),
        )
    } else {
        (AgentMode::default(), ApprovalMode::default())
    };

    // Save new session to runtime config if it doesn't exist
    if !config.runtime.sessions.iter().any(|s| s.session_id == session_id) {
        if config.runtime.theme.is_none() {
            config.runtime.theme = config
                .theme
                .clone()
                .or_else(|| config.welcome.as_ref().and_then(|w| w.theme.clone()));
        }
        config.runtime.sessions.push(RuntimeSessionConfig {
            session_id: session_id.clone(),
            agent_mode: agent_mode.to_string(),
            approval_mode: approval_mode.to_string(),
        });
        let _ = config.save_runtime();
    }

    let system_prompt = system_prompt_for_agent_mode(&config, &agent_mode);

    if let Some(legacy) = &config.llm_provider {
        if let Some(existing) = config
            .providers
            .iter_mut()
            .find(|p| p.name == legacy.provider_name)
        {
            if !existing.models.contains(&legacy.model_name) {
                existing.models.push(legacy.model_name.clone());
            }
        } else {
            config.providers.push(ProviderConfig {
                name: legacy.provider_name.clone(),
                base_url: legacy.base_url.clone(),
                api_key: legacy.api_key.clone(),
                models: vec![legacy.model_name.clone()],
            });
        }
    }

    let mut resolved: Option<(String, String)> = None;

    if let Some(default_model) = &config.default_model {
        let parts: Vec<&str> = default_model.split(':').collect();
        if parts.len() == 2 {
            let provider_name = parts[0].to_string();
            let model_name = parts[1].to_string();
            if config.providers.iter().any(|p| p.name == provider_name) {
                resolved = Some((provider_name, model_name));
            }
        } else if let Some(p) = config.providers.iter().find(|p| p.models.contains(default_model)) {
            resolved = Some((p.name.clone(), default_model.clone()));
        }
    }

    if resolved.is_none() {
        if let Some(p) = &config.llm_provider {
            resolved = Some((p.provider_name.clone(), p.model_name.clone()));
        } else if let Some(p) = config.providers.first() {
            if let Some(m) = p.models.first() {
                resolved = Some((p.name.clone(), m.clone()));
            }
        }
    }

    let (provider_name, model_name) = resolved.ok_or_else(|| Error::from_reason("No provider configured"))?;

    let mut tools: Vec<Box<dyn Tool>> = list_available_tools();
    let mcp_tools = load_mcp_tools(&config);
    tools.extend(mcp_tools);

    let mut agent = RustAgent::new(
        provider_name,
        model_name,
        system_prompt,
        config.providers.clone(),
        tools,
    )
    .map_err(|e| Error::from_reason(format!("Failed to create agent: {}", e)))?;

    if let Some(messages) = load_persisted_messages(&session_id) {
        agent.import_messages(messages);
    }

    let (inner, session_id_out) = {
        let mut manager = SESSION_MANAGER
            .lock()
            .map_err(|_| Error::from_reason("Failed to lock session manager"))?;
        let ctx = manager.add_with_context(session_id, agent, agent_mode, approval_mode);
        (Arc::clone(&ctx.inner), ctx.session_id.clone())
    };
    log_session_event(&session_id_out, "open_create", json!({}));

    Ok(SessionOpenParts {
        inner,
        session_id: session_id_out,
    })
}

pub(crate) async fn confirm_tool(
    session_id: &str,
    confirmation_sender: &Arc<Mutex<Option<PendingConfirmation>>>,
    decision: CoreConfirmDecision,
) -> Result<()> {
    let request_id = decision.request_id.clone();
    let decision_str = decision.decision.clone();
    log_session_event(
        session_id,
        "confirm_tool_called",
        json!({ "decision": decision_str, "request_id": request_id }),
    );

    let mut sender_guard = confirmation_sender.lock().await;
    if let Some(pending) = sender_guard.take() {
        if pending.request_id != decision.request_id {
            log_session_event(
                session_id,
                "confirm_tool_ignored",
                json!({ "reason": "request_id_mismatch", "pending_request_id": pending.request_id, "request_id": decision.request_id }),
            );
            return Ok(());
        }
        pending
            .sender
            .send(decision_str)
            .map_err(|_| Error::from_reason("Failed to send confirmation"))?;
        Ok(())
    } else {
        log_session_event(
            session_id,
            "confirm_tool_ignored",
            json!({ "reason": "no_active_request" }),
        );
        Ok(())
    }
}

pub(crate) async fn execute_session(
    session_id: &str,
    inner: &Arc<Mutex<RustAgent>>,
    confirmation_sender: &Arc<Mutex<Option<PendingConfirmation>>>,
    prompt: String,
) -> Result<RustAgentResult> {
    log_session_event(
        session_id,
        "execute_called",
        json!({ "prompt_chars": prompt.chars().count() }),
    );

    let agent_clone = Arc::clone(inner);
    let confirmation_sender_clone = Arc::clone(confirmation_sender);
    let session_id = session_id.to_string();

    let (result, messages_after) = {
        let mut agent = agent_clone.lock().await;

        let session_id_for_stream = session_id.clone();
        agent.set_stream_callback(move |event: StreamEvent| {
            match event {
                StreamEvent::Text(text) => {
                    if !text.is_empty() {
                        emit_stream_text(&session_id_for_stream, text);
                    }
                }
                StreamEvent::StageStart(stage) => {
                    let (stage_str, stage_state) = match stage {
                        StreamStage::Thinking => ("__THINKING__", ResponseStage::Thinking),
                        StreamStage::Answering => ("__ANSWERING__", ResponseStage::Answering),
                    };
                    set_response_stage(&session_id_for_stream, stage_state);
                    log_session_event(
                        &session_id_for_stream,
                        "stage_changed",
                        json!({ "stage": format!("{:?}", stage_state) }),
                    );
                    emit_control_event(
                        &session_id_for_stream,
                        CoreEvent {
                            protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                            session_id: session_id_for_stream.clone(),
                            ts_ms: now_ms(),
                            event_type: CoreEventType::StageStart,
                            seq: None,
                            text: None,
                            stage: Some(stage_str.to_string()),
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
                        },
                    );
                }
                StreamEvent::StageEnd(stage) => {
                    let stage_str = match stage {
                        StreamStage::Thinking => "__THINKING__",
                        StreamStage::Answering => "__ANSWERING__",
                    };
                    emit_control_event(
                        &session_id_for_stream,
                        CoreEvent {
                            protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                            session_id: session_id_for_stream.clone(),
                            ts_ms: now_ms(),
                            event_type: CoreEventType::StageEnd,
                            seq: None,
                            text: None,
                            stage: Some(stage_str.to_string()),
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
                        },
                    );
                }
                StreamEvent::End => {
                    set_response_stage(&session_id_for_stream, ResponseStage::End);
                    emit_control_event(
                        &session_id_for_stream,
                        CoreEvent {
                            protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                            session_id: session_id_for_stream.clone(),
                            ts_ms: now_ms(),
                            event_type: CoreEventType::End,
                            seq: None,
                            text: None,
                            stage: Some("__END__".to_string()),
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
                        },
                    );
                }
            }
        });

        let session_id_for_tool_executor = session_id.clone();
        agent.set_tool_executor_callback(Arc::new(
            move |tool: &Box<dyn Tool>, tool_name: &str, args: &str| {
                let tool_clone = tool.clone_box();
                let tool_name = tool_name.to_string();
                let args = args.to_string();
                let sender_arc = Arc::clone(&confirmation_sender_clone);
                let session_id_for_tool = session_id_for_tool_executor.clone();

                Box::pin(async move {
                    let key_path = key_path_from_args(&tool_name, &args);

                    let mut current_op: Option<SessionToolOperation> = None;
                    let args_summary = truncate_utf8_with_ellipsis(&args, 200);

                    let result = async {
                        let op = map_tool_operation(tool_clone.operation());
                        set_tool_operation(&session_id_for_tool, Some(op));
                        current_op = Some(op);
                        log_session_event(
                            &session_id_for_tool,
                            "tool_executor_op_set",
                            json!({
                                "tool_name": tool_name.clone(),
                                "key_path": key_path.clone(),
                                "tool_operation": format!("{:?}", op),
                                "args_summary": args_summary.clone()
                            }),
                        );

                        emit_control_event(
                            &session_id_for_tool,
                            CoreEvent {
                                protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                                session_id: session_id_for_tool.clone(),
                                ts_ms: now_ms(),
                                event_type: CoreEventType::ToolStart,
                                seq: None,
                                text: None,
                                stage: None,
                                tool_operation: Some(session_op_str(op).to_string()),
                                tool_name: Some(tool_name.clone()),
                                key_path: Some(key_path.clone()),
                                kind: Some(format!("{:?}", tool_clone.kind())),
                                args_summary: Some(args_summary.clone()),
                                response_summary: None,
                                display_text: None,
                                success: None,
                                confirm: None,
                                error_message: None,
                            },
                        );

                        let approval_mode = SESSION_MANAGER
                            .lock()
                            .ok()
                            .and_then(|m| m.get(&session_id_for_tool).map(|ctx| ctx.approval_mode.clone()))
                            .unwrap_or_default();
                        let kind = tool_clone.kind();
                        let access_level = if matches!(approval_mode, ApprovalMode::AgentFull) {
                            ToolAccessLevel::Full
                        } else {
                            ToolAccessLevel::Workspace
                        };

                        let mut effective_args = args.clone();
                        if tool_name == "bash" {
                            if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&effective_args) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert("confirmed".to_string(), serde_json::Value::Bool(true));
                                    effective_args = serde_json::to_string(&v).unwrap_or_else(|_| args.clone());
                                }
                            }
                        }

                        let requires_user_confirmation = match approval_mode {
                            ApprovalMode::ReadOnly => approval_policy::requires_confirmation(&approval_mode, kind),
                            ApprovalMode::Agent | ApprovalMode::AgentFull => false,
                        };

                        if !requires_user_confirmation {
                            return with_tool_access(access_level, || tool_clone.execute(&effective_args));
                        }

                        if let Some(status) =
                            get_confirmation_status(&session_id_for_tool, &tool_name, &key_path)
                        {
                            if status == ConfirmationStatus::AllowForSession {
                                return with_tool_access(access_level, || tool_clone.execute(&effective_args));
                            }
                        }

                        let kind = tool_clone.kind();
                        log_session_event(
                            &session_id_for_tool,
                            "confirm_requested",
                            json!({
                                "tool_name": tool_name.clone(),
                                "key_path": key_path.clone(),
                                "kind": format!("{:?}", kind),
                                "args_summary": args_summary.clone()
                            }),
                        );

                        let (tx, rx) = oneshot::channel();
                        let request_id = generate_request_id();

                        {
                            let mut sender_guard = sender_arc.lock().await;
                            *sender_guard = Some(PendingConfirmation {
                                request_id: request_id.clone(),
                                sender: tx,
                            });
                        }

                        emit_control_event(
                            &session_id_for_tool,
                            CoreEvent {
                                protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                                session_id: session_id_for_tool.clone(),
                                ts_ms: now_ms(),
                                event_type: CoreEventType::ConfirmationRequested,
                                seq: None,
                                text: None,
                                stage: None,
                                tool_operation: None,
                                tool_name: None,
                                key_path: None,
                                kind: None,
                                args_summary: None,
                                response_summary: None,
                                display_text: None,
                                success: None,
                                confirm: Some(CoreConfirmationRequest {
                                    request_id: request_id.clone(),
                                    tool_name: tool_name.clone(),
                                    arguments: args.clone(),
                                    kind: format!("{:?}", kind),
                                    key_path: key_path.clone(),
                                }),
                                error_message: None,
                            },
                        );

                        match rx.await {
                            Ok(decision) => match decision.as_str() {
                                "1" => {
                                    log_session_event(
                                        &session_id_for_tool,
                                        "confirm_decision",
                                        json!({
                                            "tool_name": tool_name.clone(),
                                            "key_path": key_path.clone(),
                                            "decision": "1"
                                        }),
                                    );
                                    with_tool_access(access_level, || tool_clone.execute(&effective_args))
                                }
                                "2" => {
                                    log_session_event(
                                        &session_id_for_tool,
                                        "confirm_decision",
                                        json!({
                                            "tool_name": tool_name.clone(),
                                            "key_path": key_path.clone(),
                                            "decision": "2"
                                        }),
                                    );
                                    set_confirmation_status(
                                        &session_id_for_tool,
                                        &tool_name,
                                        &key_path,
                                        ConfirmationStatus::AllowForSession,
                                    );
                                    log_session_event(
                                        &session_id_for_tool,
                                        "confirm_allow_for_session_set",
                                        json!({
                                            "tool_name": tool_name.clone(),
                                            "key_path": key_path.clone()
                                        }),
                                    );
                                    with_tool_access(access_level, || tool_clone.execute(&effective_args))
                                }
                                "3" => Ok(serde_json::to_string(
                                    &crate::llm::tools::tool_trait::ToolOutput::error(
                                        format!("tool call {} {}", tool_name, args),
                                        "User denied execution. Please ask for different approach.",
                                    ),
                                )
                                .unwrap()),
                                _ => Ok(serde_json::to_string(
                                    &crate::llm::tools::tool_trait::ToolOutput::error(
                                        format!("tool call {} {}", tool_name, args),
                                        "User denied execution.",
                                    ),
                                )
                                .unwrap()),
                            },
                            Err(_) => Ok(serde_json::to_string(
                                &crate::llm::tools::tool_trait::ToolOutput::error(
                                    format!("tool call {} {}", tool_name, args),
                                    "Confirmation channel closed.",
                                ),
                            )
                            .unwrap()),
                        }
                    }
                    .await;

                    if let Some(op) = current_op {
                        let status_for_log = if result.is_ok() {
                            "ok".to_string()
                        } else {
                            "error".to_string()
                        };

                        let response_summary_for_log = match &result {
                            Ok(s) => truncate_utf8_with_ellipsis(s, 200),
                            Err(e) => truncate_utf8_with_ellipsis(&e.to_string(), 200),
                        };

                        let is_todo_tool = matches!(tool_clone.kind(), ToolKind::Todo);

                        let (response_summary, stdout) = match &result {
                            Ok(raw) => {
                                if is_todo_tool {
                                    (raw.clone(), None)
                                } else {
                                    let v = serde_json::from_str::<serde_json::Value>(raw).ok();
                                    let summary = v
                                        .as_ref()
                                        .and_then(|v| {
                                            v.get("response_summary")
                                                .and_then(|s| s.as_str())
                                                .map(|s| s.to_string())
                                        })
                                        .unwrap_or_else(|| response_summary_for_log.clone());
                                    
                                    let out = v
                                        .as_ref()
                                        .and_then(|v| v.get("stdout").and_then(|s| s.as_str()))
                                        .map(|s| s.to_string());
                                        
                                    (summary, out)
                                }
                            }
                            Err(_) => (response_summary_for_log.clone(), None),
                        };

                        let display_text = if is_todo_tool {
                            None
                        } else {
                            let mut text = format!(
                                "{:?}({}) -> {}",
                                tool_clone.kind(),
                                key_path.clone(),
                                response_summary
                            );
                            if matches!(tool_clone.kind(), ToolKind::Edit) {
                                if let Some(diff) = stdout {
                                    text.push('\n');
                                    text.push_str(&diff);
                                }
                            }
                            Some(text)
                        };

                        emit_control_event(
                            &session_id_for_tool,
                            CoreEvent {
                                protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                                session_id: session_id_for_tool.clone(),
                                ts_ms: now_ms(),
                                event_type: CoreEventType::ToolOutput,
                                seq: None,
                                text: None,
                                stage: None,
                                tool_operation: Some(session_op_str(op).to_string()),
                                tool_name: Some(tool_name.clone()),
                                key_path: Some(key_path.clone()),
                                kind: Some(format!("{:?}", tool_clone.kind())),
                                args_summary: Some(args_summary.clone()),
                                response_summary: Some(response_summary.clone()),
                                display_text,
                                success: Some(result.is_ok()),
                                confirm: None,
                                error_message: None,
                            },
                        );

                        emit_control_event(
                            &session_id_for_tool,
                            CoreEvent {
                                protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                                session_id: session_id_for_tool.clone(),
                                ts_ms: now_ms(),
                                event_type: CoreEventType::ToolEnd,
                                seq: None,
                                text: None,
                                stage: None,
                                tool_operation: Some(session_op_str(op).to_string()),
                                tool_name: Some(tool_name.clone()),
                                key_path: Some(key_path.clone()),
                                kind: None,
                                args_summary: None,
                                response_summary: Some(response_summary.clone()),
                                display_text: None,
                                success: Some(result.is_ok()),
                                confirm: None,
                                error_message: None,
                            },
                        );

                        log_session_event(
                            &session_id_for_tool,
                            "tool_finished",
                            json!({
                                "tool_name": tool_name.clone(),
                                "key_path": key_path.clone(),
                                "tool_operation": format!("{:?}", op),
                                "status": status_for_log,
                                "response_summary": response_summary_for_log
                            }),
                        );
                    }

                    set_tool_operation(&session_id_for_tool, None);
                    log_session_event(
                        &session_id_for_tool,
                        "tool_executor_op_cleared",
                        json!({ "tool_name": tool_name.clone(), "key_path": key_path.clone() }),
                    );

                    result
                })
            },
        ));

        agent.add_user_message(prompt);
        let result = execute_agent_with_retry(&mut agent).await.map_err(|e| {
            let msg = format!("{:#}", e);
            log::error!("Agent execution failed: {:?}", e);
            emit_control_event(
                &session_id,
                CoreEvent {
                    protocol_version: CORE_EVENT_PROTOCOL_VERSION,
                    session_id: session_id.clone(),
                    ts_ms: now_ms(),
                    event_type: CoreEventType::Error,
                    seq: None,
                    text: None,
                    stage: None,
                    tool_operation: None,
                    tool_name: None,
                    key_path: None,
                    kind: None,
                    args_summary: None,
                    response_summary: None,
                    display_text: None,
                    success: Some(false),
                    confirm: None,
                    error_message: Some(msg.clone()),
                },
            );
            Error::from_reason(format!("Agent execution failed: {}", msg))
        })?;
        let messages_after = agent.export_messages();
        (result, messages_after)
    };

    let _ = persist_session_snapshot(&session_id, messages_after);
    Ok(result)
}

pub(crate) async fn clear_history(session_id: &str, inner: &Arc<Mutex<RustAgent>>) -> Result<()> {
    log_session_event(session_id, "history_cleared", json!({}));
    let mut agent = inner.lock().await;
    agent.clear_history();
    let messages_after = agent.export_messages();
    drop(agent);
    let _ = persist_session_snapshot(session_id, messages_after);
    Ok(())
}

#[napi_derive::napi(object)]
pub struct ProviderMessage {
    pub role: String,
    pub content: String,
}

#[napi_derive::napi(object)]
pub struct AvailableModel {
    pub provider: String,
    pub model: String,
}

#[napi_derive::napi(object)]
pub struct SavedSessionInfo {
    pub session_id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub message_count: u32,
}

pub(crate) fn get_saved_sessions() -> Result<Vec<SavedSessionInfo>> {
    let metas = store::list_saved_sessions()
        .map_err(|e| Error::from_reason(format!("Failed to list saved sessions: {}", e)))?;
    Ok(metas
        .into_iter()
        .map(|m| SavedSessionInfo {
            session_id: m.session_id,
            created_at_ms: m.created_at_ms,
            updated_at_ms: m.updated_at_ms,
            message_count: m.message_count as u32,
        })
        .collect())
}

pub(crate) async fn get_history(inner: &Arc<Mutex<RustAgent>>) -> Result<Vec<ProviderMessage>> {
    let agent = inner.lock().await;
    Ok(agent
        .export_messages()
        .into_iter()
        .map(|m| ProviderMessage {
            role: m.role,
            content: m.content,
        })
        .collect())
}

pub(crate) async fn get_available_models(inner: &Arc<Mutex<RustAgent>>) -> Result<Vec<AvailableModel>> {
    let agent = inner.lock().await;
    let models = agent.get_available_models();
    Ok(models
        .into_iter()
        .map(|(provider, model)| AvailableModel { provider, model })
        .collect())
}

pub(crate) async fn set_model(
    inner: &Arc<Mutex<RustAgent>>,
    provider: String,
    model: String,
) -> Result<()> {
    {
        let mut agent = inner.lock().await;
        agent.set_model(&provider, &model)
            .map_err(|e| Error::from_reason(e.to_string()))?;
    }

    let mut config =
        AppConfig::load().map_err(|e| Error::from_reason(format!("Failed to load config: {}", e)))?;
    config.runtime.default_model = Some(format!("{}:{}", provider, model));
    config
        .save_runtime()
        .map_err(|e| Error::from_reason(format!("Failed to save runtime config: {}", e)))?;
    Ok(())
}

pub struct LatencyInfo {
    pub latency_ms: u32,
    pub model_name: String,
}

pub(crate) async fn check_latency(inner: &Arc<Mutex<RustAgent>>) -> Result<LatencyInfo> {
    let (base_url, model_name) = {
        let agent = inner.lock().await;
        (agent.get_base_url(), agent.get_model_name())
    };
    
    let ms = measure_latency(&base_url).await.map_err(|e| Error::from_reason(e.to_string()))?;
    Ok(LatencyInfo {
        latency_ms: ms as u32,
        model_name,
    })
}

pub(crate) fn get_sessions() -> Result<Vec<String>> {
    let manager = SESSION_MANAGER
        .lock()
        .map_err(|_| Error::from_reason("Failed to lock session manager"))?;
    Ok(manager.list_ids())
}

pub(crate) fn get_agent_mode(session_id: &str) -> Result<String> {
    let manager = SESSION_MANAGER
        .lock()
        .map_err(|_| Error::from_reason("Failed to lock session manager"))?;
    let ctx = manager
        .get(session_id)
        .ok_or_else(|| Error::from_reason("Session not found"))?;
    Ok(ctx.agent_mode.to_string())
}

pub(crate) async fn set_agent_mode(
    session_id: &str,
    inner: &Arc<Mutex<RustAgent>>,
    mode: String,
) -> Result<()> {
    let agent_mode = AgentMode::from(mode);
    let approval_mode = {
        let mut manager = SESSION_MANAGER
            .lock()
            .map_err(|_| Error::from_reason("Failed to lock session manager"))?;
        let ctx = manager
            .get_mut(session_id)
            .ok_or_else(|| Error::from_reason("Session not found"))?;
        ctx.agent_mode = agent_mode.clone();
        ctx.approval_mode.clone()
    };

    let mut config =
        AppConfig::load().map_err(|e| Error::from_reason(format!("Failed to load config: {}", e)))?;
    let system_prompt = system_prompt_for_agent_mode(&config, &agent_mode);
    {
        let mut agent = inner.lock().await;
        agent
            .set_system_prompt(system_prompt)
            .map_err(|e| Error::from_reason(e.to_string()))?;
    }

    if let Some(s) = config
        .runtime
        .sessions
        .iter_mut()
        .find(|s| s.session_id == session_id)
    {
        s.agent_mode = agent_mode.to_string();
    } else {
        config.runtime.sessions.push(RuntimeSessionConfig {
            session_id: session_id.to_string(),
            agent_mode: agent_mode.to_string(),
            approval_mode: approval_mode.to_string(),
        });
    }
    config
        .save_runtime()
        .map_err(|e| Error::from_reason(format!("Failed to save runtime config: {}", e)))?;
    Ok(())
}

pub(crate) fn set_theme(theme: String) -> Result<()> {
    let mut config = AppConfig::load().map_err(|e| Error::from_reason(format!("Failed to load config: {}", e)))?;
    config.runtime.theme = Some(theme);
    config.save_runtime().map_err(|e| Error::from_reason(format!("Failed to save runtime config: {}", e)))?;
    Ok(())
}

pub(crate) fn get_approval_mode(session_id: &str) -> Result<String> {
    let manager = SESSION_MANAGER
        .lock()
        .map_err(|_| Error::from_reason("Failed to lock session manager"))?;
    let ctx = manager
        .get(session_id)
        .ok_or_else(|| Error::from_reason("Session not found"))?;
    Ok(ctx.approval_mode.to_string())
}

pub(crate) fn set_approval_mode(session_id: &str, mode: String) -> Result<()> {
    let mode = ApprovalMode::from(mode);
    {
        let mut manager = SESSION_MANAGER
            .lock()
            .map_err(|_| Error::from_reason("Failed to lock session manager"))?;
        let ctx = manager
            .get_mut(session_id)
            .ok_or_else(|| Error::from_reason("Session not found"))?;
        ctx.approval_mode = mode.clone();
    }

    let mut config =
        AppConfig::load().map_err(|e| Error::from_reason(format!("Failed to load config: {}", e)))?;
    if let Some(s) = config
        .runtime
        .sessions
        .iter_mut()
        .find(|s| s.session_id == session_id)
    {
        s.approval_mode = mode.to_string();
    } else {
        config.runtime.sessions.push(RuntimeSessionConfig {
            session_id: session_id.to_string(),
            agent_mode: AgentMode::default().to_string(),
            approval_mode: mode.to_string(),
        });
    }
    config
        .save_runtime()
        .map_err(|e| Error::from_reason(format!("Failed to save runtime config: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::system_prompt_for_agent_mode;
    use crate::config::AppConfig;
    use crate::session::context::AgentMode;

    fn embedded_config() -> AppConfig {
        toml::from_str(include_str!("../../Config.toml")).expect("embedded Config.toml should parse")
    }

    #[test]
    fn prompt_plan_uses_prompt_plan_template() {
        let cfg = embedded_config();
        let prompt = system_prompt_for_agent_mode(&cfg, &AgentMode::Plan).unwrap_or_default();
        assert!(prompt.contains("read-only mode"));
    }

    #[test]
    fn prompt_build_uses_prompt_build_template() {
        let cfg = embedded_config();
        let prompt = system_prompt_for_agent_mode(&cfg, &AgentMode::Build).unwrap_or_default();
        assert!(prompt.contains("## New Applications"));
    }

    #[test]
    fn disabled_prompt_build_falls_back_to_default() {
        let mut cfg = embedded_config();
        if let Some(pb) = cfg.prompt_build.as_mut() {
            pb.enabled = false;
        }
        let prompt = system_prompt_for_agent_mode(&cfg, &AgentMode::Build);
        assert_eq!(prompt, Some("You are a helpful coding assistant.".to_string()));
    }

}
