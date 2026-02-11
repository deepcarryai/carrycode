use crate::llm::tools::builtin::core_bash::{CoreBashTool, CoreBashRequest};
use crate::llm::tools::builtin::core_tool_base::ToolSpec;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_bash_rejects_oversized_command() {
        let tool = CoreBashTool::default();
        let args = CoreBashRequest {
            command: "a".repeat(100000),
            workdir: None,
            timeout: None,
            confirmed: false,
        };
        let err = <CoreBashTool as ToolSpec>::run(&tool, args, false).unwrap_err();
        assert!(err.to_string().contains("command too long"));
    }
}