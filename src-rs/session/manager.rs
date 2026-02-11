use std::collections::HashMap;
use std::sync::Mutex as StdMutex;

use lazy_static::lazy_static;

use crate::llm::agents::agent::Agent as RustAgent;
use crate::skills::registry::SkillRegistry;
use std::sync::Arc;

use super::context::{AgentMode, ApprovalMode, SessionContext};

pub struct SessionManager {
    sessions: HashMap<String, SessionContext>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn get(&self, session_id: &str) -> Option<&SessionContext> {
        self.sessions.get(session_id)
    }

    pub fn get_mut(&mut self, session_id: &str) -> Option<&mut SessionContext> {
        self.sessions.get_mut(session_id)
    }

    pub fn add(&mut self, session_id: String, agent: RustAgent, skill_registry: Arc<SkillRegistry>, enabled_skills: Vec<String>) -> &SessionContext {
        let ctx = SessionContext::new(session_id.clone(), agent, AgentMode::default(), ApprovalMode::default(), skill_registry, enabled_skills, None);
        self.sessions.insert(session_id.clone(), ctx);
        self.sessions.get(&session_id).expect("Just inserted")
    }

    pub fn add_with_context(&mut self, session_id: String, agent: RustAgent, agent_mode: AgentMode, approval_mode: ApprovalMode, skill_registry: Arc<SkillRegistry>, enabled_skills: Vec<String>, title: Option<String>) -> &SessionContext {
        let ctx = SessionContext::new(session_id.clone(), agent, agent_mode, approval_mode, skill_registry, enabled_skills, title);
        self.sessions.insert(session_id.clone(), ctx);
        self.sessions.get(&session_id).expect("Just inserted")
    }

    pub fn remove(&mut self, session_id: &str) -> Option<SessionContext> {
        self.sessions.remove(session_id)
    }

    pub fn list_ids(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }
}

lazy_static! {
    pub static ref SESSION_MANAGER: StdMutex<SessionManager> = StdMutex::new(SessionManager::new());
}
