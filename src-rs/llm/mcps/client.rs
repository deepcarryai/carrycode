use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write, Read};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

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

enum Transport {
    Stdio {
        stdin: std::process::ChildStdin,
        reader: std::io::Lines<BufReader<std::process::ChildStdout>>,
        #[allow(dead_code)]
        child: Child,
    },
    Http {
        client: reqwest::blocking::Client,
        endpoint: Option<String>,
        reader: std::io::Lines<BufReader<Box<dyn Read + Send>>>,
    }
}

struct ClientInner {
    transport: Transport,
    request_id: u64,
}

pub struct McpClient {
    inner: Mutex<ClientInner>,
}

impl McpClient {
    pub fn new(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.envs(env);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        let mut child = cmd.spawn().context("Failed to spawn MCP server")?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("No stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("No stdout"))?;
        let reader = BufReader::new(stdout).lines();

        Ok(Self {
            inner: Mutex::new(ClientInner {
                transport: Transport::Stdio {
                    stdin,
                    reader,
                    child,
                },
                request_id: 0,
            }),
        })
    }

    pub fn new_http(
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<Self> {
        let client = reqwest::blocking::Client::new();
        let mut req_builder = client.get(url);
        
        for (k, v) in headers {
            req_builder = req_builder.header(k, v);
        }
        req_builder = req_builder.header("Accept", "text/event-stream");

        let response = req_builder.send().context("Failed to connect to MCP SSE endpoint")?;
        
        let status = response.status();
        if !status.is_success() {
             return Err(anyhow!("Failed to connect to MCP SSE endpoint: {}", status));
        }

        // We wrap the response body reader.
        // Note: This is a synchronous line reader. SSE events are usually line based.
        // We will need to parse the SSE format manually or find a way to reuse the reader.
        // For simplicity in a blocking context, manual parsing of "event: " and "data: " is feasible.
        let reader = BufReader::new(Box::new(response) as Box<dyn Read + Send>).lines();

        Ok(Self {
            inner: Mutex::new(ClientInner {
                transport: Transport::Http {
                    client,
                    endpoint: None,
                    reader,
                },
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

        match &mut inner.transport {
            Transport::Stdio { stdin, .. } => {
                let json_req = serde_json::to_string(&req)?;
                stdin.write_all(json_req.as_bytes())?;
                stdin.write_all(b"\n")?;
                stdin.flush()?;
            }
            Transport::Http { client, endpoint, .. } => {
                let endpoint_url = endpoint.as_ref().ok_or_else(|| anyhow!("MCP endpoint not initialized"))?;
                // Send POST request
                let res = client.post(endpoint_url)
                    .json(&req)
                    .send()?;
                
                if !res.status().is_success() {
                    let text = res.text().unwrap_or_default();
                    return Err(anyhow!("MCP request failed: {} - {}", endpoint_url, text));
                }
                
                // Return immediate result if the response is the JSON-RPC response already?
                // The spec says: "The server MUST send a POST request... for messages..."
                // But wait, the client sends messages via POST.
                // The server sends responses via SSE.
                // UNLESS the response comes back immediately in the POST body?
                // "The server may optionally return a response in the POST body."
                
                let text = res.text()?;
                if !text.is_empty() {
                     // If we got a response immediately, use it
                     if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&text) {
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
        }
        
        // Loop to find response
        loop {
            let line = match &mut inner.transport {
                Transport::Stdio { reader, .. } => {
                    match reader.next() {
                        Some(Ok(l)) => l,
                        Some(Err(e)) => return Err(anyhow!("Failed to read from MCP: {}", e)),
                        None => return Err(anyhow!("MCP stream ended")),
                    }
                }
                Transport::Http { reader, .. } => {
                    // Manual SSE parsing
                    // We need to accumulate "data:" lines until an empty line.
                    let mut event_type = String::new();
                    let mut data = String::new();
                    
                    loop {
                         match reader.next() {
                            Some(Ok(l)) => {
                                if l.is_empty() {
                                    // End of event
                                    break;
                                }
                                if l.starts_with("event: ") {
                                    event_type = l[7..].trim().to_string();
                                } else if l.starts_with("data: ") {
                                    data.push_str(&l[6..]);
                                }
                            }
                            Some(Err(e)) => return Err(anyhow!("Failed to read from MCP SSE: {}", e)),
                            None => return Err(anyhow!("MCP SSE stream ended")),
                        }
                    }
                    
                    // Allow filtering by event type if needed? 
                    // Usually JSON-RPC messages are in 'message' event or default.
                    if event_type == "endpoint" {
                        // This should be handled in initialize?
                        // If we see it here, we might just ignore or update.
                         log::info!("Received endpoint event during request: {}", data);
                         continue;
                    }
                    
                    data
                }
            };

            if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                if let Some(resp_id) = &resp.id {
                    if resp_id.as_u64() == Some(id) {
                        if let Some(err) = resp.error {
                            return Err(anyhow!("MCP Error {}: {}", err.code, err.message));
                        }
                        return Ok(resp.result.unwrap_or(Value::Null));
                    }
                }
                log::debug!("Ignored MCP message: {}", line);
            } else {
                 // Try to parse as notification (no id) or just log
                 log::debug!("MCP Raw Output: {}", line);
            }
        }
    }

    pub fn initialize(&self) -> Result<()> {
        // Special handling for HTTP: wait for endpoint event
        {
             let mut inner = self.inner.lock().map_err(|_| anyhow!("Failed to lock client"))?;
             if let Transport::Http { reader, endpoint, .. } = &mut inner.transport {
                 // We need to read the first event which should be 'endpoint'
                 // Loop until we find it
                 loop {
                    let mut event_type = String::new();
                    let mut data = String::new();
                    
                    loop {
                         match reader.next() {
                            Some(Ok(l)) => {
                                if l.is_empty() { break; }
                                if l.starts_with("event: ") {
                                    event_type = l[7..].trim().to_string();
                                } else if l.starts_with("data: ") {
                                    data.push_str(&l[6..]);
                                }
                            }
                            Some(Err(e)) => return Err(anyhow!("Failed to read from MCP SSE: {}", e)),
                            None => return Err(anyhow!("MCP SSE stream ended")),
                        }
                    }
                    
                    if event_type == "endpoint" {
                        *endpoint = Some(data.trim().to_string());
                        log::info!("MCP HTTP Endpoint discovered: {}", data);
                        break;
                    }
                 }
             }
        }

        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": {
                    "listChanged": true
                },
                "sampling": {}
            },
            "clientInfo": {
                "name": "carrycode-cli",
                "version": "0.1.0"
            }
        });

        self.request("initialize", Some(params))?;
        
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

        match &mut inner.transport {
            Transport::Stdio { stdin, .. } => {
                let json_req = serde_json::to_string(&req)?;
                stdin.write_all(json_req.as_bytes())?;
                stdin.write_all(b"\n")?;
                stdin.flush()?;
            }
             Transport::Http { client, endpoint, .. } => {
                 let endpoint_url = endpoint.as_ref().ok_or_else(|| anyhow!("MCP endpoint not initialized"))?;
                 client.post(endpoint_url)
                    .json(&req)
                    .send()?;
             }
        }
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
