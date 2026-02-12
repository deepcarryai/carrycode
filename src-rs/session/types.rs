use napi_derive::napi;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseStage {
    Thinking,
    Answering,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionToolOperation {
    Explored,
    Edited,
    Todo,
    Bash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationStatus {
    Ask,
    AllowForSession,
}

pub fn session_tool_operation_tag(op: SessionToolOperation) -> &'static str {
    match op {
        SessionToolOperation::Explored => "__EXPLORED__",
        SessionToolOperation::Edited => "__EDITED__",
        SessionToolOperation::Todo => "__TODO__",
        SessionToolOperation::Bash => "__BASH__",
    }
}

pub const CORE_EVENT_PROTOCOL_VERSION: u16 = 1;

#[napi(string_enum)]
pub enum CoreEventType {
    Text,
    StageStart,
    StageEnd,
    ToolStart,
    ToolOutput,
    ToolEnd,
    End,
    ConfirmationRequested,
    Error,
}

#[napi(object)]
#[derive(Clone)]
pub struct CoreConfirmationRequest {
    #[napi(js_name = "requestId")]
    pub request_id: String,
    #[napi(js_name = "toolName")]
    pub tool_name: String,
    pub arguments: String,
    pub kind: String,
    #[napi(js_name = "keyPath")]
    pub key_path: String,
}

#[napi(object)]
#[derive(Clone)]
pub struct CoreConfirmDecision {
    #[napi(js_name = "requestId")]
    pub request_id: String,
    pub decision: String,
}

#[napi(object)]
#[derive(Clone)]
pub struct CoreEvent {
    #[napi(js_name = "protocolVersion")]
    pub protocol_version: u16,
    #[napi(js_name = "sessionId")]
    pub session_id: String,
    #[napi(js_name = "tsMs")]
    pub ts_ms: i64,
    #[napi(js_name = "eventType")]
    pub event_type: CoreEventType,
    pub seq: Option<i64>,
    pub text: Option<String>,
    pub stage: Option<String>,
    #[napi(js_name = "toolOperation")]
    pub tool_operation: Option<String>,
    #[napi(js_name = "toolName")]
    pub tool_name: Option<String>,
    #[napi(js_name = "keyPath")]
    pub key_path: Option<String>,
    pub kind: Option<String>,
    #[napi(js_name = "argsSummary")]
    pub args_summary: Option<String>,
    #[napi(js_name = "responseSummary")]
    pub response_summary: Option<String>,
    #[napi(js_name = "displayText")]
    pub display_text: Option<String>,
    pub success: Option<bool>,
    pub confirm: Option<CoreConfirmationRequest>,
    #[napi(js_name = "errorMessage")]
    pub error_message: Option<String>,
}
