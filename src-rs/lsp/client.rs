use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::lsp::protocol::*;
use crate::lsp::transport::{MessageReader, MessageWriter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Starting,
    Ready,
}

pub struct LspClient {
    server_name: String,
    _process: Arc<Mutex<Child>>,
    writer: Arc<Mutex<MessageWriter>>,
    request_id: Arc<AtomicU32>,
    pending_requests: Arc<Mutex<HashMap<u32, oneshot::Sender<Message>>>>,
    diagnostics: Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>,
    state: Arc<RwLock<ServerState>>,
    timeout_ms: u64,
    _message_loop: JoinHandle<()>,
}

impl LspClient {
    pub async fn new(
        server_name: String,
        command: String,
        args: Vec<String>,
        root_path: Option<String>,
        timeout_ms: u64,
    ) -> Result<Self> {
        let mut child = Command::new(&command)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context(format!("Failed to spawn LSP server: {}", command))?;

        let stdin = child.stdin.take().context("Failed to get stdin")?;
        let stdout = child.stdout.take().context("Failed to get stdout")?;

        let writer = Arc::new(Mutex::new(MessageWriter::new(stdin)));
        let mut reader = MessageReader::new(stdout);

        let request_id = Arc::new(AtomicU32::new(1));
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let diagnostics = Arc::new(RwLock::new(HashMap::new()));
        let state = Arc::new(RwLock::new(ServerState::Starting));

        // Spawn message reading loop
        let pending_clone = pending_requests.clone();
        let diagnostics_clone = diagnostics.clone();
        let message_loop = tokio::spawn(async move {
            loop {
                match reader.read_message().await {
                    Ok(message) => {
                        Self::handle_message(message, &pending_clone, &diagnostics_clone).await;
                    }
                    Err(e) => {
                        log::error!("Error reading message: {}", e);
                        break;
                    }
                }
            }
        });

        let client = Self {
            server_name: server_name.clone(),
            _process: Arc::new(Mutex::new(child)),
            writer,
            request_id,
            pending_requests,
            diagnostics,
            state,
            timeout_ms,
            _message_loop: message_loop,
        };

        // Initialize
        client.initialize(root_path).await?;

        Ok(client)
    }

    async fn handle_message(
        message: Message,
        pending: &Arc<Mutex<HashMap<u32, oneshot::Sender<Message>>>>,
        diagnostics: &Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>,
    ) {
        // Handle response
        if let Some(id) = message.id {
            let mut pending = pending.lock().await;
            if let Some(sender) = pending.remove(&id) {
                let _ = sender.send(message);
            }
            return;
        }

        // Handle notification
        if let Some(method) = &message.method {
            if method == "textDocument/publishDiagnostics" {
                if let Some(params) = message.params {
                    if let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(params) {
                        let mut diag_map = diagnostics.write().await;
                        diag_map.insert(params.uri, params.diagnostics);
                    }
                }
            }
        }
    }

    async fn send_request(&self, method: &str, params: serde_json::Value) -> Result<Message> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        let message = Message {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: Some(method.to_string()),
            params: Some(params),
            result: None,
            error: None,
        };

        {
            let mut writer = self.writer.lock().await;
            writer.write_message(&message).await?;
        }

        let response = tokio::time::timeout(std::time::Duration::from_millis(self.timeout_ms), rx)
            .await
            .context("Request timeout")??;

        Ok(response)
    }

    async fn initialize(&self, root_path: Option<String>) -> Result<()> {
        let root_uri = root_path.map(|p| {
            let path = Path::new(&p);
            format!("file://{}", path.display())
        });

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri,
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                        related_information: Some(true),
                    }),
                }),
            },
        };

        let response = self
            .send_request("initialize", serde_json::to_value(params)?)
            .await?;

        if response.error.is_some() {
            anyhow::bail!("Initialize failed: {:?}", response.error);
        }

        // Send initialized notification
        let initialized_msg = Message {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some("initialized".to_string()),
            params: Some(serde_json::json!({})),
            result: None,
            error: None,
        };

        {
            let mut writer = self.writer.lock().await;
            writer.write_message(&initialized_msg).await?;
        }

        *self.state.write().await = ServerState::Ready;
        log::info!("LSP server '{}' initialized", self.server_name);

        Ok(())
    }

    pub async fn open_file(
        &self,
        file_path: &str,
        language_id: &str,
        content: String,
    ) -> Result<()> {
        let uri = format!("file://{}", file_path);

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text: content,
            },
        };

        let message = Message {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some("textDocument/didOpen".to_string()),
            params: Some(serde_json::to_value(params)?),
            result: None,
            error: None,
        };

        let mut writer = self.writer.lock().await;
        writer.write_message(&message).await?;

        Ok(())
    }

    pub async fn get_diagnostics(&self, file_path: &str) -> Result<Vec<Diagnostic>> {
        let uri = format!("file://{}", file_path);
        let diagnostics = self.diagnostics.read().await;
        Ok(diagnostics.get(&uri).cloned().unwrap_or_default())
    }

    pub async fn get_all_diagnostics(&self) -> HashMap<String, Vec<Diagnostic>> {
        self.diagnostics.read().await.clone()
    }

    pub async fn is_ready(&self) -> bool {
        *self.state.read().await == ServerState::Ready
    }
}
