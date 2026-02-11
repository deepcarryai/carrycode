use crate::llm::config::AppConfig;
use crate::llm::utils::file_tracker::{PathSecurity, FILE_HISTORY_TRACKER, FILE_READ_TRACKER};
use crate::policy::path_policy::PathPolicy;
use crate::llm::tools::builtin::core_tool_base::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use crate::lsp::diagnostics::DiagnosticSummary;
use crate::lsp::LspManager;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::sync::Arc;
use std::time::Duration;

/// Edit tool for modifying files
#[derive(Clone)]
pub struct CoreEditTool {
    /// Tool name identifier
    pub tool_name: String,
    /// Description of what this tool does
    pub description: String,
    /// Optional LSP manager for diagnostics
    pub lsp_manager: Option<Arc<LspManager>>,
    /// Timeout for LSP diagnostics
    pub lsp_timeout_ms: u64,
}

/// Edit request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreEditRequest {
    /// Path to the file to edit
    pub file_path: String,
    /// Text to replace (unique in file)
    #[serde(default)]
    pub old_string: String,
    /// New text to replace with
    #[serde(default)]
    pub new_string: String,
}

/// Result of editing a file
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EditResult {
    /// File path
    pub file_path: String,
    /// Whether the edit was successful
    pub success: bool,
    /// Whether this is an error result
    pub is_error: bool,
    /// Number of replacements made
    pub replacements: usize,
    /// Diff preview of the changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// Number of lines added
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additions: Option<usize>,
    /// Number of lines removed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removals: Option<usize>,
    /// LSP diagnostics (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<DiagnosticSummary>,
    /// Summary of the result
    pub response_summary: String,
}

impl CoreEditTool {
    /// Create a new EditTool by loading configuration from config.toml
    ///
    /// If config.toml is not found or fails to parse, falls back to hardcoded defaults.
    pub fn new() -> Self {
        Self::new_with_lsp(None)
    }

    /// Create a new EditTool with optional LSP manager
    pub fn new_with_lsp(lsp_manager: Option<Arc<LspManager>>) -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.core_edit.tool_name,
                description: config.core_edit.description,
                lsp_manager,
                lsp_timeout_ms: config.lsp.timeout_ms,
            },
            Err(e) => {
                log::warn!(
                    "Failed to load config.toml: {}, using hardcoded defaults",
                    e
                );
                Self {
                    lsp_manager,
                    ..Self::default()
                }
            }
        }
    }

    /// Create a new EditTool from a specific AppConfig
    ///
    /// # Arguments
    /// * `config` - The application configuration
    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.core_edit.tool_name.clone(),
            description: config.core_edit.description.clone(),
            lsp_manager: None,
            lsp_timeout_ms: config.lsp.timeout_ms,
        }
    }

    /// Create a new EditTool with custom configuration
    #[allow(dead_code)]
    pub fn with_config(tool_name: String, description: String) -> Self {
        Self {
            tool_name,
            description,
            lsp_manager: None,
            lsp_timeout_ms: 5000,
        }
    }

    /// Edit file content
    ///
    /// # Arguments
    /// * `request` - Edit request parameters
    ///
    /// # Returns
    /// * `Result<EditResult>` - The result containing edit metadata
    fn run_edit(&self, request: &CoreEditRequest) -> Result<EditResult> {
        let path_policy = PathPolicy::new()?;
        let path_buf = path_policy.resolve(&request.file_path)?;
        let absolute_path = path_buf.to_string_lossy().to_string();
        let path = path_buf.as_path();

        // Check if path is a directory
        if path.exists() && path.is_dir() {
            anyhow::bail!(
                "Path '{}' is a directory, not a file. Cannot edit directories.",
                request.file_path
            );
        }

        let original_content;

        // Handle empty strings case first (no file operations needed)
        if request.old_string.is_empty() && request.new_string.is_empty() {
            return Ok(EditResult {
                file_path: request.file_path.clone(),
                success: false,
                is_error: true,
                response_summary: "0 lines".to_string(),
                ..Default::default()
            });
        }

        // Handle file creation case
        if request.old_string.is_empty() && !request.new_string.is_empty() {
            // Creating new file - check if file already exists
            if path.exists() {
                anyhow::bail!("File already exists: {}", request.file_path);
            }
            original_content = String::new();
        } else if path.exists() {
            // Read-before-write validation
            {
                let tracker = FILE_READ_TRACKER.lock().unwrap();

                if !tracker.has_been_read(&absolute_path) {
                    anyhow::bail!(
                        "File '{}' has not been read. Use the 'view' tool to read the file before editing it.",
                        request.file_path
                    );
                }

                // Check if file has been modified since last read
                if let Some(last_read_time) = tracker.get_last_read_time(&absolute_path) {
                    let current_mod_time = PathSecurity::get_modification_time(path)?;

                    if current_mod_time > last_read_time {
                        anyhow::bail!(
                            "File '{}' has been modified since it was last read. Please re-read the file before editing.",
                            request.file_path
                        );
                    }
                }
            }

            original_content = fs::read_to_string(path).context("Failed to read file")?;
        } else {
            anyhow::bail!("File not found: {}", request.file_path);
        }

        let new_content;
        let replacements;

        if request.old_string.is_empty() {
            new_content = request.new_string.clone();
            replacements = 1;
        } else if request.new_string.is_empty() {
            let occurrence_count = original_content.matches(&request.old_string).count();
            if occurrence_count == 0 {
                anyhow::bail!("old_string not found in file");
            }
            if occurrence_count > 1 {
                anyhow::bail!(
                    "old_string found {} times. It must be unique. Add more context to make it unique.",
                    occurrence_count
                );
            }
            new_content = original_content.replace(&request.old_string, "");
            replacements = 1;
        } else {
            let occurrence_count = original_content.matches(&request.old_string).count();
            if occurrence_count == 0 {
                anyhow::bail!("old_string not found in file");
            }
            if occurrence_count > 1 {
                anyhow::bail!(
                    "old_string found {} times. It must be unique. Add more context to make it unique.",
                    occurrence_count
                );
            }
            new_content = original_content.replace(&request.old_string, &request.new_string);
            replacements = 1;
        }

        // Check if content is actually changing
        if original_content == new_content {
            anyhow::bail!("New content is the same as old content. No changes made.");
        }

        // Calculate diff before writing
        let (diff_str, additions, removals) = Self::calculate_diff(&original_content, &new_content, &request.file_path);
        
        let diff = Some(diff_str);

        // Create parent directories if they don't exist (for new files)
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Check if file content changed externally (intermediate version detection)
        if path.exists() {
            let history_tracker = FILE_HISTORY_TRACKER.lock().unwrap();
            if let Some(latest_version) = history_tracker.get_latest_version(&absolute_path) {
                if latest_version.content != original_content {
                    // Content changed externally, save intermediate version
                    drop(history_tracker);
                    let mut history_tracker = FILE_HISTORY_TRACKER.lock().unwrap();
                    history_tracker.record_version(&absolute_path, original_content.clone());
                }
            }
        }

        fs::write(path, &new_content).context("Failed to write file")?;

        // Record file history
        {
            let mut history_tracker = FILE_HISTORY_TRACKER.lock().unwrap();
            history_tracker.record_version(&absolute_path, new_content);
        }

        // Mark file as read after write
        {
            let mut read_tracker = FILE_READ_TRACKER.lock().unwrap();
            read_tracker.record_read(&absolute_path);
        }

        let mut result = EditResult {
            file_path: request.file_path.clone(),
            success: true,
            is_error: false,
            replacements,
            diff,
            additions: Some(additions),
            removals: Some(removals),
            diagnostics: None,
            response_summary: format!("{} lines", additions + removals),
        };

        // Collect LSP diagnostics if available
        if let Some(lsp_manager) = &self.lsp_manager {
            result.diagnostics = self.collect_diagnostics(lsp_manager, &absolute_path);
        }

        Ok(result)
    }

    /// Collect LSP diagnostics with timeout
    fn collect_diagnostics(
        &self,
        lsp_manager: &Arc<LspManager>,
        file_path: &str,
    ) -> Option<DiagnosticSummary> {
        let lsp = lsp_manager.clone();
        let path = file_path.to_string();
        let timeout = Duration::from_millis(self.lsp_timeout_ms);

        // Use tokio runtime to run async code
        let runtime = tokio::runtime::Handle::try_current()
            .or_else(|_| tokio::runtime::Runtime::new().map(|rt| rt.handle().clone()))
            .ok()?;

        runtime.block_on(async move {
            match tokio::time::timeout(timeout, lsp.get_diagnostics(&path)).await {
                Ok(Ok(Some(diagnostics))) => Some(diagnostics),
                Ok(Ok(None)) => None,
                Ok(Err(e)) => {
                    log::warn!("Failed to get LSP diagnostics: {}", e);
                    None
                }
                Err(_) => {
                    log::warn!("LSP diagnostic collection timed out");
                    None
                }
            }
        })
    }

    /// Calculate diff between old and new content in standard unified format
    ///
    /// Shows only +/- lines with a small amount of surrounding context.
    ///
    /// # Arguments
    /// * `old_content` - Original content
    /// * `new_content` - New content
    /// * `file_path` - Path to the file (for diff header)
    ///
    /// # Returns
    /// * Tuple of (diff string, additions count, removals count)
    pub(crate) fn calculate_diff(old_content: &str, new_content: &str, file_path: &str) -> (String, usize, usize) {
        let diff = TextDiff::from_lines(old_content, new_content);
        let mut additions = 0;
        let mut removals = 0;

        for op in diff.ops() {
            for change in diff.iter_changes(op) {
                match change.tag() {
                    ChangeTag::Delete => removals += 1,
                    ChangeTag::Insert => additions += 1,
                    ChangeTag::Equal => {}
                }
            }
        }

        let mut output = format!("diff --git a/{file_path} b/{file_path}\n");
        let unified_diff = diff.unified_diff()
            .context_radius(2)
            .header(&format!("a/{file_path}"), &format!("b/{file_path}"))
            .to_string();
        
        output.push_str(&unified_diff);

        (output, additions, removals)
    }

    /// Get tool definition as JSON for LLM
    fn to_tool_definition_json(&self) -> serde_json::Value {
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
                            "description": "The path to the file to modify (will be converted to absolute path). Must be within current working directory."
                        },
                        "old_string": {
                            "type": "string",
                            "description": "The text to replace (must be unique within the file). Leave empty when creating a new file."
                        },
                        "new_string": {
                            "type": "string",
                            "description": "The edited text to replace the old_string with. Leave empty to delete content."
                        }
                    },
                    "required": ["file_path"]
                }
            }
        })
    }
}

impl Default for CoreEditTool {
    fn default() -> Self {
        Self {
            tool_name: "core_edit".to_string(),
            description: "[CORE SYSTEM] Edits files by replacing text, creating new files, or deleting content."
                .to_string(),
            lsp_manager: None,
            lsp_timeout_ms: 5000,
        }
    }
}

impl ToolSpec for CoreEditTool {
    type Args = CoreEditRequest;

    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Edit
    }

    fn operation(&self) -> ToolOperation {
        ToolOperation::Edited
    }

    fn to_tool_definition(&self) -> serde_json::Value {
        self.to_tool_definition_json()
    }

    fn run(&self, args: Self::Args, _confirmed: bool) -> Result<ToolResult> {
        let result = self.run_edit(&args)?;
        let response_summary = result.response_summary.clone();
        let success = result.success && !result.is_error;
        let stdout = result
            .diff
            .as_deref()
            .unwrap_or(&result.response_summary)
            .to_string();
        let data = serde_json::to_value(&result)?;
        let mut tr = ToolResult::ok(
            self.tool_name.clone(),
            self.kind(),
            self.operation(),
            stdout,
            data,
        )
        .with_summary(response_summary);
        tr.success = success;
        if !success {
            tr.stderr = result.response_summary;
        }
        Ok(tr)
    }
}

