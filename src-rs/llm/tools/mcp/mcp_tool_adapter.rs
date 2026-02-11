use super::mcp_tool_base::McpTool;
use crate::llm::tools::builtin::core_tool_base::{Tool, ToolKind, ToolOperation, ToolOutput};
use anyhow::Result;
use serde_json::{json, Value};

impl Tool for McpTool {
    fn name(&self) -> &str {
        self.definition
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown_mcp_tool")
    }

    fn description(&self) -> &str {
        self.definition
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn operation(&self) -> ToolOperation {
        ToolOperation::Other
    }

    fn to_tool_definition(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": self.definition.get("inputSchema").unwrap_or(&json!({}))
            }
        })
    }

    fn execute(&self, arguments: &str) -> Result<String> {
        let args_val: Value = serde_json::from_str(arguments)?;
        // Use original_name to call the server
        let result = self.client.call_tool(&self.original_name, args_val)?;

        // MCP result is { content: [ { type: "text", text: "..." } ], isError: bool }
        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut output = String::new();
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            for item in content {
                if let Some(text) = item.get("text").and_then(|t: &Value| t.as_str()) {
                    output.push_str(text);
                }
            }
        }

        if is_error {
            Ok(serde_json::to_string(&ToolOutput::error(
                format!("mcp call {}", self.name()),
                output,
            ))?)
        } else {
            Ok(serde_json::to_string(&ToolOutput::success(
                format!("mcp call {}", self.name()),
                output,
            ))?)
        }
    }

    fn clone_box(&self) -> Box<dyn Tool> {
        Box::new(McpTool {
            client: self.client.clone(),
            definition: self.definition.clone(),
            original_name: self.original_name.clone(),
        })
    }
}
