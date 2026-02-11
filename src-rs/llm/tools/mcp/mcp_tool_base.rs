use super::mcp_tool_client::McpClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub struct McpTool {
    pub(crate) client: Arc<McpClient>,
    pub(crate) definition: Value,
    pub(crate) original_name: String,
}

impl McpTool {
    pub fn new(client: Arc<McpClient>, mut definition: Value, server_name: &str) -> Self {
        let original_name = definition
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown_mcp_tool")
            .to_string();

        let exposed_name = if original_name.starts_with(server_name) {
            original_name.clone()
        } else {
            format!("{}_{}", server_name, original_name)
        };
        
        log::info!("MCP Tool Loaded: server={}, original={}, exposed={}", server_name, original_name, exposed_name);

        definition["name"] = json!(exposed_name);

        Self {
            client,
            definition,
            original_name,
        }
    }
}
