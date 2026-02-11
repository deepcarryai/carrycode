use super::mcp_tool_base::McpTool;
use crate::config::AppConfig;
use crate::llm::tools::builtin::core_tool_base::Tool;
use std::sync::Arc;
use futures::future::join_all;

pub async fn load_mcp_tools(config: &AppConfig) -> Vec<Box<dyn Tool>> {
    let mut tasks = Vec::new();

    for (name, server_config) in &config.mcp_servers {
        let name = name.clone();
        let server_config = server_config.clone();

        tasks.push(tokio::task::spawn_blocking(move || {
            log::info!("Initializing MCP server: {}", name);
            let client_result = server_config.create_client();

            match client_result {
                Ok(client) => {
                    if let Err(e) = client.initialize() {
                        log::error!("Failed to initialize MCP server {}: {}", name, e);
                        return Vec::new();
                    }

                    match client.list_tools() {
                        Ok(tool_defs) => {
                            let client_arc = Arc::new(client);
                            let mut tools: Vec<Box<dyn Tool>> = Vec::new();
                            for def in tool_defs {
                                tools.push(Box::new(McpTool::new(client_arc.clone(), def, &name)));
                            }
                            tools
                        }
                        Err(e) => {
                            log::error!("Failed to list tools for MCP server {}: {}", name, e);
                            Vec::new()
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to start MCP server {}: {}", name, e);
                    Vec::new()
                }
            }
        }));
    }

    let results = join_all(tasks).await;
    let mut tools = Vec::new();

    for res in results {
        match res {
            Ok(server_tools) => tools.extend(server_tools),
            Err(e) => log::error!("MCP initialization task panicked: {}", e),
        }
    }

    tools
}
