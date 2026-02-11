use crate::llm::config::AppConfig;
use crate::llm::tools::builtin::core_tool_base::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use crate::lsp::LspManager;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct CoreDiagnosticsTool {
    pub tool_name: String,
    pub description: String,
    lsp_manager: Arc<Mutex<Option<Arc<LspManager>>>>,
}

impl std::fmt::Debug for CoreDiagnosticsTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreDiagnosticsTool")
            .field("tool_name", &self.tool_name)
            .field("description", &self.description)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreDiagnosticsRequest {
    #[serde(default)]
    pub file_path: String,
}

impl Default for CoreDiagnosticsTool {
    fn default() -> Self {
        Self {
            tool_name: "core_diagnostics".to_string(),
            description:
                "[CORE SYSTEM] Get LSP diagnostics (errors, warnings, hints) for files using language servers"
                    .to_string(),
            lsp_manager: Arc::new(Mutex::new(None)),
        }
    }
}

impl CoreDiagnosticsTool {
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.core_diagnostics.tool_name,
                description: config.core_diagnostics.description,
                lsp_manager: Arc::new(Mutex::new(None)),
            },
            Err(_) => Self::default(),
        }
    }

    async fn get_or_init_lsp_manager(&self) -> Result<Arc<LspManager>> {
        // Check if manager already exists
        {
            let manager_lock = self.lsp_manager.lock().unwrap();
            if let Some(manager) = manager_lock.as_ref() {
                return Ok(Arc::clone(manager));
            }
        }

        let config = AppConfig::load()?;
        if !config.lsp.enabled {
            anyhow::bail!("LSP not enabled in config");
        }

        let manager = LspManager::new(
            &config.lsp,
            Some(std::env::current_dir()?.to_string_lossy().to_string()),
        )
        .await?;

        let manager_arc = Arc::new(manager);
        *self.lsp_manager.lock().unwrap() = Some(Arc::clone(&manager_arc));
        Ok(manager_arc)
    }

    pub async fn run_diagnostics(&self, request: &CoreDiagnosticsRequest) -> Result<String> {
        let lsp_manager = self.get_or_init_lsp_manager().await?;

        if request.file_path.is_empty() {
            // Project-wide diagnostics
            let summary = lsp_manager.get_all_diagnostics().await?;
            Ok(summary.to_string())
        } else {
            // Single file diagnostics
            if !std::path::Path::new(&request.file_path).exists() {
                anyhow::bail!("File not found: {}", request.file_path);
            }

            match lsp_manager.get_diagnostics(&request.file_path).await? {
                Some(summary) => Ok(summary.to_string()),
                None => Ok(format!(
                    "No diagnostics available for {}",
                    request.file_path
                )),
            }
        }
    }
}

impl ToolSpec for CoreDiagnosticsTool {
    type Args = CoreDiagnosticsRequest;

    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn operation(&self) -> ToolOperation {
        ToolOperation::Explored
    }

    fn to_tool_definition(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.tool_name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to analyze (leave empty for project-wide diagnostics)"
                        }
                    },
                    "required": []
                }
            }
        })
    }

    fn run(&self, args: Self::Args, _confirmed: bool) -> Result<ToolResult> {
        let file_path = args.file_path.clone();
        let self_clone = self.clone();
        let stdout = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async move { self_clone.run_diagnostics(&args).await })
        })?;
        Ok(ToolResult::ok(
            self.tool_name.clone(),
            self.kind(),
            self.operation(),
            stdout,
            json!({ "file_path": file_path }),
        )
        .with_summary("diagnostics".to_string()))
    }
}
