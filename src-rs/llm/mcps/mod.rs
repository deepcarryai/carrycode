pub mod client;
pub mod tool;

pub use client::McpClient;
pub use tool::McpTool;

use crate::config::AppConfig;
use crate::llm::tools::tool_trait::Tool;
use std::sync::Arc;

pub fn load_mcp_tools(config: &AppConfig) -> Vec<Box<dyn Tool>> {
    let mut tools: Vec<Box<dyn Tool>> = Vec::new();

    for (name, server_config) in &config.mcp_servers {
        log::info!("Initializing MCP server: {}", name);
        let client_result = match server_config {
            crate::config::McpServerConfig::Stdio { command, args, env, .. } => {
                McpClient::new(command, args, env)
            }
            crate::config::McpServerConfig::Http { url, headers, .. } => {
                McpClient::new_http(url, headers)
            }
        };

        match client_result {
            Ok(client) => {
                if let Err(e) = client.initialize() {
                    log::error!("Failed to initialize MCP server {}: {}", name, e);
                    continue;
                }

                match client.list_tools() {
                    Ok(tool_defs) => {
                        let client_arc = Arc::new(client);
                        for def in tool_defs {
                            tools.push(Box::new(McpTool::new(client_arc.clone(), def, name)));
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to list tools for MCP server {}: {}", name, e);
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to start MCP server {}: {}", name, e);
            }
        }
    }

    tools
}
