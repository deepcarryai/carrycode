use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction};
use tokio::sync::Mutex;

use crate::llm::agents::agent::Agent as RustAgent;

use super::types::{ConfirmationStatus, CoreEvent, ResponseStage, SessionToolOperation};

pub struct SessionEventSink {
    pub handler: ThreadsafeFunction<CoreEvent, ErrorStrategy::CalleeHandled>,
}

#[derive(Debug, Clone)]
pub enum AgentMode {
    Plan,
    Build,
}

impl Default for AgentMode {
    fn default() -> Self {
        AgentMode::Build
    }
}

impl From<String> for AgentMode {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "plan" => AgentMode::Plan,
            _ => AgentMode::Build,
        }
    }
}

impl ToString for AgentMode {
    fn to_string(&self) -> String {
        match self {
            AgentMode::Plan => "plan".to_string(),
            AgentMode::Build => "build".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ApprovalMode {
    ReadOnly,
    Agent,
    AgentFull,
}

impl Default for ApprovalMode {
    fn default() -> Self {
        ApprovalMode::Agent
    }
}

impl From<String> for ApprovalMode {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "read-only" => ApprovalMode::ReadOnly,
            "agent-full" => ApprovalMode::AgentFull,
            _ => ApprovalMode::Agent,
        }
    }
}

impl ToString for ApprovalMode {
    fn to_string(&self) -> String {
        match self {
            ApprovalMode::ReadOnly => "read-only".to_string(),
            ApprovalMode::AgentFull => "agent-full".to_string(),
            ApprovalMode::Agent => "agent".to_string(),
        }
    }
}
pub struct SessionContext {
    pub inner: Arc<Mutex<RustAgent>>,
    pub session_id: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub tool_confirm: Arc<StdMutex<HashMap<(String, String), ConfirmationStatus>>>,
    pub response_stage: Arc<StdMutex<ResponseStage>>,
    pub tool_operation: Arc<StdMutex<Option<SessionToolOperation>>>,
    pub event_sink: Arc<StdMutex<Option<SessionEventSink>>>,
    pub event_seq: Arc<StdMutex<i64>>,
    pub agent_mode: AgentMode,
    pub approval_mode: ApprovalMode,
}

impl SessionContext {
    pub fn new(session_id: String, agent: RustAgent, agent_mode: AgentMode, approval_mode: ApprovalMode) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            inner: Arc::new(Mutex::new(agent)),
            session_id,
            created_at: now,
            updated_at: now,
            tool_confirm: Arc::new(StdMutex::new(HashMap::new())),
            response_stage: Arc::new(StdMutex::new(ResponseStage::Thinking)),
            tool_operation: Arc::new(StdMutex::new(None)),
            event_sink: Arc::new(StdMutex::new(None)),
            event_seq: Arc::new(StdMutex::new(0)),
            agent_mode,
            approval_mode,
        }
    }
}
