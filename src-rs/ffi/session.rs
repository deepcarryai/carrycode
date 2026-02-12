use napi::bindgen_prelude::*;
use napi::JsFunction;
use napi_derive::napi;

use crate::llm::agents::agent::Agent as RustAgent;
use crate::session::generate_session_id;
use crate::session::types::CoreConfirmDecision;
use crate::session::{clear_event_sink, set_event_sink};
use crate::session::context::SessionEventSink;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::session_util::{self, AvailableModel, ProviderMessage, SavedSessionInfo};

#[napi]
pub fn create_session_id() -> String {
    generate_session_id()
}

#[napi]
pub struct Session {
    inner: Arc<Mutex<RustAgent>>,
    confirmation_sender: Arc<Mutex<Option<session_util::PendingConfirmation>>>,
    session_id: String,
}

#[napi(object)]
pub struct AgentResult {
    pub content: String,
    #[napi(js_name = "toolsUsed")]
    pub tools_used: bool,
}

#[napi]
impl Session {
    #[napi(factory)]
    pub fn open(session_id: String) -> Result<Self> {
        let parts = session_util::open_session(session_id)?;
        Ok(Self {
            inner: parts.inner,
            confirmation_sender: Arc::new(Mutex::new(None)),
            session_id: parts.session_id,
        })
    }

    #[napi]
    pub async fn confirm_tool(&self, decision: CoreConfirmDecision) -> Result<()> {
        session_util::confirm_tool(&self.session_id, &self.confirmation_sender, decision).await
    }

    #[napi]
    pub fn subscribe(&self, on_event: JsFunction) -> Result<()> {
        let tsfn = on_event.create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))?;

        let sink = SessionEventSink {
            handler: tsfn,
        };

        if !set_event_sink(&self.session_id, sink) {
            return Err(Error::from_reason("Session not found"));
        }

        Ok(())
    }

    #[napi]
    pub fn unsubscribe(&self) -> Result<()> {
        clear_event_sink(&self.session_id);
        Ok(())
    }

    #[napi]
    pub async fn execute(&self, prompt: String) -> Result<AgentResult> {
        let result = session_util::execute_session(
            &self.session_id,
            &self.inner,
            &self.confirmation_sender,
            prompt,
        )
        .await?;
        Ok(AgentResult {
            content: result.content,
            tools_used: result.tools_used,
        })
    }

    #[napi]
    pub async fn clear_history(&self) -> Result<()> {
        session_util::clear_history(&self.session_id, &self.inner).await
    }

    #[napi]
    pub async fn get_history(&self) -> Result<Vec<ProviderMessage>> {
        session_util::get_history(&self.inner).await
    }

    #[napi]
    pub async fn get_available_models(&self) -> Result<Vec<AvailableModel>> {
        session_util::get_available_models(&self.inner).await
    }

    #[napi]
    pub async fn set_model(&self, provider: String, model: String) -> Result<()> {
        session_util::set_model(&self.inner, provider, model).await
    }

    #[napi]
    pub async fn check_latency(&self) -> Result<LatencyInfo> {
        let info = session_util::check_latency(&self.inner).await?;
        Ok(LatencyInfo {
            latency_ms: info.latency_ms,
            model_name: info.model_name,
        })
    }

    #[napi]
    pub fn get_sessions() -> Result<Vec<String>> {
        session_util::get_sessions()
    }

    #[napi]
    pub fn get_saved_sessions() -> Result<Vec<SavedSessionInfo>> {
        session_util::get_saved_sessions()
    }

    #[napi]
    pub fn set_theme(theme: String) -> Result<()> {
        session_util::set_theme(theme)
    }

    #[napi]
    pub fn get_agent_mode(&self) -> Result<String> {
        session_util::get_agent_mode(&self.session_id)
    }

    #[napi]
    pub async fn set_agent_mode(&self, mode: String) -> Result<()> {
        session_util::set_agent_mode(&self.session_id, &self.inner, mode).await
    }

    #[napi]
    pub fn get_approval_mode(&self) -> Result<String> {
        session_util::get_approval_mode(&self.session_id)
    }

    #[napi]
    pub fn set_approval_mode(&self, mode: String) -> Result<()> {
        session_util::set_approval_mode(&self.session_id, mode)
    }
}

#[napi(object)]
pub struct LatencyInfo {
    pub latency_ms: u32,
    pub model_name: String,
}
