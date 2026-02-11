use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write, Read};
use std::process::{Child, Command, Stdio};

use super::mcp_tool_base::{JsonRpcRequest, JsonRpcResponse};

pub trait McpTransport: Send {
    fn start(&mut self) -> Result<()>;
    fn send_request(&mut self, req: &JsonRpcRequest, id: u64) -> Result<Option<Value>>;
    fn next_message(&mut self) -> Result<String>;
    fn close(&mut self) -> Result<()>;
    fn set_endpoint(&mut self, _endpoint: String) {}
}

pub struct StdioTransport {
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    child: Option<Child>,
    stdin: Option<std::process::ChildStdin>,
    reader: Option<std::io::Lines<BufReader<std::process::ChildStdout>>>,
}

impl StdioTransport {
    pub fn new(command: String, args: Vec<String>, env: HashMap<String, String>) -> Self {
        Self {
            command,
            args,
            env,
            child: None,
            stdin: None,
            reader: None,
        }
    }
}

impl McpTransport for StdioTransport {
    fn start(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        cmd.envs(&self.env);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        let stderr_mode = std::env::var("CARRY_MCP_STDERR").unwrap_or_default();
        if stderr_mode.eq_ignore_ascii_case("inherit") {
            cmd.stderr(Stdio::inherit());
        } else {
            cmd.stderr(Stdio::null());
        }

        let mut child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn MCP server: {}", e))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("No stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("No stdout"))?;
        let reader = BufReader::new(stdout).lines();

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.reader = Some(reader);

        Ok(())
    }

    fn send_request(&mut self, req: &JsonRpcRequest, _id: u64) -> Result<Option<Value>> {
        let stdin = self.stdin.as_mut().ok_or_else(|| anyhow!("Transport not started"))?;
        let json_req = serde_json::to_string(&req)?;
        stdin.write_all(json_req.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(None)
    }

    fn next_message(&mut self) -> Result<String> {
        let reader = self.reader.as_mut().ok_or_else(|| anyhow!("Transport not started"))?;
        match reader.next() {
            Some(Ok(l)) => Ok(l),
            Some(Err(e)) => Err(anyhow!("Failed to read from MCP Stdio: {}", e)),
            None => Err(anyhow!("MCP Stdio stream ended")),
        }
    }

    fn close(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdin = None;
        self.reader = None;
        Ok(())
    }
}

pub struct LegacySseTransport {
    mcp_url: String,
    post_url: Option<String>,
    client: reqwest::blocking::Client,
    reader: Option<std::io::Lines<BufReader<Box<dyn Read + Send>>>>,
    headers: HashMap<String, String>,
}

impl LegacySseTransport {
    pub fn new(mcp_url: String, post_url: Option<String>, headers: HashMap<String, String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("carrycode-cli/0.5.0")
            .timeout(None) 
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .no_gzip()
            .no_deflate()
            .no_brotli()
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            mcp_url,
            post_url,
            client,
            reader: None,
            headers,
        }
    }
}

impl McpTransport for LegacySseTransport {
    fn start(&mut self) -> Result<()> {
        log::debug!("Connecting to MCP SSE: {}", self.mcp_url);
        let mut req_builder = self.client.get(&self.mcp_url);
        
        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }
        req_builder = req_builder.header("Accept", "text/event-stream");

        let response = req_builder.send().map_err(|e| anyhow!("Failed to connect to MCP SSE endpoint: {}", e))?;
        
        log::debug!("MCP SSE Status: {}", response.status());
        log::debug!("MCP SSE Headers: {:?}", response.headers());

        if !response.status().is_success() {
             let status = response.status();
             let text = response.text().unwrap_or_default();
             return Err(anyhow!("Failed to connect to MCP SSE endpoint: {} - {}", status, text));
        }

        self.reader = Some(BufReader::new(Box::new(response) as Box<dyn Read + Send>).lines());
        Ok(())
    }

    fn send_request(&mut self, req: &JsonRpcRequest, id: u64) -> Result<Option<Value>> {
        let endpoint_str = self.post_url.as_ref().ok_or_else(|| anyhow!("MCP endpoint not initialized (waiting for 'endpoint' event)"))?;
        
        // Resolve relative URL if needed
        let resolved_url = if endpoint_str.starts_with("http://") || endpoint_str.starts_with("https://") {
            endpoint_str.clone()
        } else {
            let base = reqwest::Url::parse(&self.mcp_url)
                .map_err(|e| anyhow!("Failed to parse base SSE URL: {}", e))?;
            
            let mut resolved = base.join(endpoint_str)
                .map_err(|e| anyhow!("Failed to join endpoint URL: {}", e))?;
            
            // If the original URL had an Authorization query param, and the new one doesn't, copy it over
            if let Some(auth) = reqwest::Url::parse(&self.mcp_url).ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "Authorization").map(|(_, v)| v.into_owned())) {
                if !resolved.query_pairs().any(|(k, _)| k == "Authorization") {
                    resolved.query_pairs_mut().append_pair("Authorization", &auth);
                }
            }
            resolved.to_string()
        };

        log::debug!("MCP POST Request to: {}", resolved_url);
        let mut req_builder = self.client.post(&resolved_url);
        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }
        req_builder = req_builder.header("Accept", "application/json");
        req_builder = req_builder.header("Accept-Language", "en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7");
        req_builder = req_builder.header("Connection", "keep-alive");

        let res = req_builder
            .json(&req)
            .send()?;
        
        if !res.status().is_success() {
            let text = res.text().unwrap_or_default();
            return Err(anyhow!("MCP request failed: {} - {}", resolved_url, text));
        }
        
        let text = res.text()?;
        if !text.is_empty() {
            if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&text) {
                if let Some(resp_id) = &resp.id {
                    if resp_id.as_u64() == Some(id) {
                        if let Some(err) = resp.error {
                            return Err(anyhow!("MCP Error {}: {}", err.code, err.message));
                        }
                        return Ok(Some(resp.result.unwrap_or(Value::Null)));
                    }
                }
            }
        }
        
        Ok(None)
    }

    fn next_message(&mut self) -> Result<String> {
        let reader = self.reader.as_mut().ok_or_else(|| anyhow!("Transport not started"))?;
        
        let mut event_type = String::new();
        let mut data = String::new();
        
        loop {
            match reader.next() {
                Some(Ok(l)) => {
                    log::debug!("MCP SSE Line: [{}]", l);
                    if l.is_empty() {
                        // End of event block
                        if !data.is_empty() || !event_type.is_empty() {
                            break;
                        }
                        continue; 
                    }
                    if l.starts_with("event:") {
                        event_type = l[6..].trim().to_string();
                    } else if l.starts_with("data:") {
                        let content = if l.starts_with("data: ") {
                            &l[6..]
                        } else {
                            &l[5..]
                        };
                        data.push_str(content);
                    } else if l.starts_with(":") {
                        // Comment, ignore
                        continue;
                    }
                }
                Some(Err(e)) => return Err(anyhow!("Failed to read from MCP SSE: {}", e)),
                None => return Err(anyhow!("MCP SSE stream ended")),
            }
        }
        
        if event_type == "endpoint" {
            // We need to return this to the caller to update the transport state if needed,
            // or we handle it here.
            // But post_url is in self. We can update it if we are allowed to.
            // We can't update self.post_url here because self is borrowed mutably for reader?
            // Wait, if I borrow `self.reader` I can't borrow `self` again?
            // Actually, `reader` is a field. I can update `post_url`.
            // But I did `let reader = self.reader.as_mut()...`. This borrows `self.reader` mutably.
            // Does it borrow all of `self`? No, disjoint borrows are allowed in Rust.
            // BUT, `self.reader` is inside `self`.
            // If I want to update `self.post_url`, I need mutable access to `self`.
            // Rust borrow checker might complain if I access `self.post_url` while `self.reader` is borrowed.
            
            // To avoid this, I will just return the special message and let the client handle it?
            // Or I can return it as a special string.
            return Ok(format!("__SSE_EVENT_ENDPOINT__:{}", data));
        }
        
        Ok(data)
    }

    fn close(&mut self) -> Result<()> {
        self.reader = None;
        Ok(())
    }

    fn set_endpoint(&mut self, endpoint: String) {
        self.post_url = Some(endpoint);
    }
}

pub struct StreamableHttpTransport {
    mcp_url: String,
    client: reqwest::blocking::Client,
    reader: Option<std::io::Lines<BufReader<Box<dyn Read + Send>>>>,
    headers: HashMap<String, String>,
    session_id: Option<String>,
}

impl StreamableHttpTransport {
    pub fn new(mcp_url: String, headers: HashMap<String, String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("carrycode-cli/0.5.0")
            .timeout(None) 
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            mcp_url,
            client,
            reader: None,
            headers,
            session_id: None,
        }
    }
}

impl McpTransport for StreamableHttpTransport {
    fn start(&mut self) -> Result<()> {
        let mut req_builder = self.client.get(&self.mcp_url);
        
        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }
        req_builder = req_builder.header("Accept", "text/event-stream");

        let response = req_builder.send().map_err(|e| anyhow!("Failed to connect to MCP Streamable endpoint: {}", e))?;
        
        if !response.status().is_success() {
             return Err(anyhow!("Failed to connect to MCP Streamable endpoint: {}", response.status()));
        }

        if let Some(val) = response.headers().get("Mcp-Session-Id") {
            if let Ok(s) = val.to_str() {
                self.session_id = Some(s.to_string());
            }
        }

        self.reader = Some(BufReader::new(Box::new(response) as Box<dyn Read + Send>).lines());
        Ok(())
    }

    fn send_request(&mut self, req: &JsonRpcRequest, id: u64) -> Result<Option<Value>> {
        let mut req_builder = self.client.post(&self.mcp_url);
        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }
        
        if let Some(sid) = &self.session_id {
            req_builder = req_builder.header("Mcp-Session-Id", sid);
        }

        let res = req_builder
            .json(&req)
            .send()?;
        
        if !res.status().is_success() {
            let text = res.text().unwrap_or_default();
            return Err(anyhow!("MCP request failed: {} - {}", self.mcp_url, text));
        }
        
        let text = res.text()?;
        if !text.is_empty() {
             if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&text) {
                if let Some(resp_id) = &resp.id {
                    if resp_id.as_u64() == Some(id) {
                        if let Some(err) = resp.error {
                            return Err(anyhow!("MCP Error {}: {}", err.code, err.message));
                        }
                        return Ok(Some(resp.result.unwrap_or(Value::Null)));
                    }
                }
            }
        }
        
        Ok(None)
    }

    fn next_message(&mut self) -> Result<String> {
        let reader = self.reader.as_mut().ok_or_else(|| anyhow!("Transport not started"))?;
        
        let mut _event_type = String::new();
        let mut data = String::new();
        
        loop {
            match reader.next() {
                Some(Ok(l)) => {
                    if l.is_empty() {
                        break;
                    }
                    if l.starts_with("event: ") {
                        _event_type = l[7..].trim().to_string();
                    } else if l.starts_with("data: ") {
                        data.push_str(&l[6..]);
                    }
                }
                Some(Err(e)) => return Err(anyhow!("Failed to read from MCP SSE: {}", e)),
                None => return Err(anyhow!("MCP SSE stream ended")),
            }
        }
        
        Ok(data)
    }

    fn close(&mut self) -> Result<()> {
        if let Some(sid) = &self.session_id {
            let mut req_builder = self.client.delete(&self.mcp_url);
            for (k, v) in &self.headers {
                req_builder = req_builder.header(k, v);
            }
            req_builder = req_builder.header("Mcp-Session-Id", sid);
            let _ = req_builder.send();
        }
        self.reader = None;
        Ok(())
    }
}
