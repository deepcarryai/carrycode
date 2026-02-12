pub mod client;
pub mod config;
pub mod diagnostics;
pub mod protocol;
pub mod transport;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::lsp::client::LspClient;
use crate::lsp::config::{LspConfig, ServerConfig};
use crate::lsp::diagnostics::{format_diagnostics, DiagnosticSummary};
use crate::lsp::protocol::Diagnostic;

pub struct LspManager {
    clients: Arc<RwLock<HashMap<String, Arc<LspClient>>>>,
    config: LspConfig,
}

impl std::fmt::Debug for LspManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspManager")
            .field("config", &self.config)
            .finish()
    }
}

impl LspManager {
    pub async fn new(config: &LspConfig, workspace_root: Option<String>) -> Result<Self> {
        let mut clients = HashMap::new();

        for server_config in &config.servers {
            match Self::start_server(server_config, workspace_root.clone(), config.timeout_ms).await
            {
                Ok(client) => {
                    log::info!("Started LSP server: {}", server_config.name);
                    clients.insert(server_config.name.clone(), Arc::new(client));
                }
                Err(e) => {
                    log::warn!("Failed to start LSP server {}: {}", server_config.name, e);
                }
            }
        }

        Ok(Self {
            clients: Arc::new(RwLock::new(clients)),
            config: config.clone(),
        })
    }

    async fn start_server(
        server_config: &ServerConfig,
        workspace_root: Option<String>,
        timeout_ms: u64,
    ) -> Result<LspClient> {
        LspClient::new(
            server_config.name.clone(),
            server_config.command.clone(),
            server_config.args.clone(),
            workspace_root,
            timeout_ms,
        )
        .await
    }

    pub async fn get_diagnostics(&self, file_path: &str) -> Result<Option<DiagnosticSummary>> {
        let clients = self.clients.read().await;

        if clients.is_empty() {
            return Ok(None);
        }

        // Determine which server to use based on file extension
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        for (name, client) in clients.iter() {
            if let Some(server_config) = self
                .config
                .servers
                .iter()
                .find(|s| s.name == *name && s.file_extensions.contains(&ext.to_string()))
            {
                if !client.is_ready().await {
                    continue;
                }

                // Open file if not already open
                let language_id = &server_config.name;
                if let Ok(content) = tokio::fs::read_to_string(file_path).await {
                    let _ = client.open_file(file_path, language_id, content).await;
                }

                // Wait a bit for diagnostics
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let diagnostics = client.get_diagnostics(file_path).await?;
                if !diagnostics.is_empty() {
                    let mut map = HashMap::new();
                    map.insert(format!("file://{}", file_path), diagnostics);
                    return Ok(Some(format_diagnostics(map)));
                }
            }
        }

        Ok(None)
    }

    pub async fn get_all_diagnostics(&self) -> Result<DiagnosticSummary> {
        let clients = self.clients.read().await;
        let mut all_diagnostics: HashMap<String, Vec<Diagnostic>> = HashMap::new();

        for client in clients.values() {
            let diag_map = client.get_all_diagnostics().await;
            for (uri, diagnostics) in diag_map {
                all_diagnostics
                    .entry(uri)
                    .or_insert_with(Vec::new)
                    .extend(diagnostics);
            }
        }

        Ok(format_diagnostics(all_diagnostics))
    }
}
