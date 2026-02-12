// Tool definitions and implementations

pub mod bash;
pub mod diagnostics;
pub mod edit;
pub mod fetch;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod todo_write;
pub mod tool_trait;
pub mod view;
pub mod write;

// Re-export main types
pub use bash::BashTool;
pub use diagnostics::DiagnosticsTool;
pub use edit::EditTool;
pub use fetch::FetchTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use ls::LsTool;
pub use todo_write::TodoWriteTool;
pub use tool_trait::{Tool, ToolAdapter};
pub use view::ViewTool;
pub use write::WriteTool;

/// Get list of available tools
pub fn list_available_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ToolAdapter(BashTool::new())),
        Box::new(ToolAdapter(DiagnosticsTool::new())),
        Box::new(ToolAdapter(EditTool::new())),
        Box::new(ToolAdapter(FetchTool::new())),
        Box::new(ToolAdapter(GlobTool::new())),
        Box::new(ToolAdapter(GrepTool::new())),
        Box::new(ToolAdapter(LsTool::new())),
        Box::new(ToolAdapter(TodoWriteTool::new())),
        Box::new(ToolAdapter(ViewTool::new())),
        Box::new(ToolAdapter(WriteTool::new())),
    ]
}

#[cfg(test)]
mod tool_contract_tests;
