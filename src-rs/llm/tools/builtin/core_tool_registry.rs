use super::core_tool_base::{Tool, ToolAdapter};
use super::core_bash::CoreBashTool;
use super::core_diagnostics::CoreDiagnosticsTool;
use super::core_edit::CoreEditTool;
use super::core_fetch::CoreFetchTool;
use super::core_glob::CoreGlobTool;
use super::core_grep::CoreGrepTool;
use super::core_ls::CoreLsTool;
use super::core_todo_write::CoreTodoWriteTool;
use super::core_view::CoreViewTool;
use super::core_write::CoreWriteTool;

/// Get list of available tools
pub fn list_available_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ToolAdapter(CoreBashTool::new())),
        Box::new(ToolAdapter(CoreDiagnosticsTool::new())),
        Box::new(ToolAdapter(CoreEditTool::new())),
        Box::new(ToolAdapter(CoreFetchTool::new())),
        Box::new(ToolAdapter(CoreGlobTool::new())),
        Box::new(ToolAdapter(CoreGrepTool::new())),
        Box::new(ToolAdapter(CoreLsTool::new())),
        Box::new(ToolAdapter(CoreTodoWriteTool::new())),
        Box::new(ToolAdapter(CoreViewTool::new())),
        Box::new(ToolAdapter(CoreWriteTool::new())),
    ]
}
