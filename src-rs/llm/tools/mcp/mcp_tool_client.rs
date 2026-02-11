use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

use super::mcp_tool_base::{JsonRpcRequest, JsonRpcResponse};
use super::transport::{McpTransport, StdioTransport, LegacySseTransport, StreamableHttpTransport};

struct ClientInner {
    transport: Box<dyn McpTransport>,
    request_id: u64,
}

pub struct McpClient {
    inner: Mutex<ClientInner>,
}

impl McpClient {
    pub fn new_stdio(
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Result<Self> {
        let mut transport = StdioTransport::new(command, args, env);
        transport.start()?;
        
        Ok(Self {
            inner: Mutex::new(ClientInner {
                transport: Box::new(transport),
                request_id: 0,
            }),
        })
    }

    pub fn new_legacy_sse(
        mcp_url: String,
        post_url: Option<String>,
        headers: HashMap<String, String>,
    ) -> Result<Self> {
        let mut transport = LegacySseTransport::new(mcp_url, post_url, headers);
        transport.start()?;

        Ok(Self {
            inner: Mutex::new(ClientInner {
                transport: Box::new(transport),
                request_id: 0,
            }),
        })
    }

    pub fn new_streamable_http(
        mcp_url: String,
        headers: HashMap<String, String>,
    ) -> Result<Self> {
        let mut transport = StreamableHttpTransport::new(mcp_url, headers);
        transport.start()?;

        Ok(Self {
            inner: Mutex::new(ClientInner {
                transport: Box::new(transport),
                request_id: 0,
            }),
        })
    }

    pub fn request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("Failed to lock client"))?;
        
        inner.request_id += 1;
        let id = inner.request_id;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(id)),
            method: method.to_string(),
            params,
        };

        if let Some(res) = inner.transport.send_request(&req, id)? {
            return Ok(res);
        }
        
        // Loop for SSE response
        loop {
            let line = inner.transport.next_message()?;

            if line.starts_with("__SSE_EVENT_ENDPOINT__:") {
                let endpoint = line[23..].trim().to_string();
                inner.transport.set_endpoint(endpoint);
                continue;
            }

            if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                if let Some(resp_id) = &resp.id {
                    if resp_id.as_u64() == Some(id) {
                        if let Some(err) = resp.error {
                            return Err(anyhow!("MCP Error {}: {}", err.code, err.message));
                        }
                        return Ok(resp.result.unwrap_or(Value::Null));
                    }
                }
            }
        }
    }

    pub fn initialize(&self) -> Result<()> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": true },
                "sampling": {}
            },
            "clientInfo": {
                "name": "carrycode",
                "version": "0.5.0"
            }
        });

        // Loop to ensure we have endpoint if needed
        let init_res = self.request("initialize", Some(params.clone()));
        
        if let Err(e) = &init_res {
            if e.to_string().contains("MCP endpoint not initialized") {
                // Wait for endpoint
                let mut inner = self.inner.lock().map_err(|_| anyhow!("Failed to lock client"))?;
                loop {
                    let msg = inner.transport.next_message()?;
                    if msg.starts_with("__SSE_EVENT_ENDPOINT__:") {
                        let ep = msg[23..].trim().to_string();
                        inner.transport.set_endpoint(ep);
                        break;
                    }
                }
                drop(inner);
                // Retry
                self.request("initialize", Some(params))?;
                self.notify("notifications/initialized", None)?;
                return Ok(());
            }
        }
        
        init_res?;
        self.notify("notifications/initialized", None)?;
        
        Ok(())
    }

    pub fn notify(&self, method: &str, params: Option<Value>) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("Failed to lock client"))?;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params,
        };

        inner.transport.send_request(&req, 0)?;
        Ok(())
    }

    pub fn list_tools(&self) -> Result<Vec<Value>> {
        let response = self.request("tools/list", None)?;
        
        if let Some(tools) = response.get("tools").and_then(|t| t.as_array()) {
            Ok(tools.clone())
        } else {
            Ok(Vec::new())
        }
    }

    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let params = json!({
            "name": name,
            "arguments": args
        });
        self.request("tools/call", Some(params))
    }
}
