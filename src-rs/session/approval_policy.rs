use crate::llm::tools::tool_trait::ToolKind;

use super::context::ApprovalMode;

pub fn requires_confirmation(approval_mode: &ApprovalMode, kind: ToolKind) -> bool {
    match approval_mode {
        ApprovalMode::ReadOnly => matches!(
            kind,
            ToolKind::Edit | ToolKind::Delete | ToolKind::Move | ToolKind::Execute | ToolKind::Fetch | ToolKind::Other
        ),
        ApprovalMode::Agent | ApprovalMode::AgentFull => false,
    }
}

