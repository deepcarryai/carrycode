use crate::llm::config::AppConfig;
use crate::llm::utils::file_tracker::PathSecurity;
use crate::llm::utils::path_policy::PathPolicy;
use crate::llm::tools::tool_trait::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use crate::lsp::diagnostics::DiagnosticSummary;
use crate::lsp::LspManager;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime};

/// File history entry
#[derive(Debug, Clone)]
pub struct FileHistoryEntry {
    /// When the file was last written
    pub write_time: SystemTime,
    /// Modification time of the file at the time of write
    pub mod_time: SystemTime,
}

/// Global file write history tracker
/// Tracks file modification times to detect external changes
pub static FILE_WRITE_HISTORY: LazyLock<Mutex<HashMap<String, FileHistoryEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Write tool for creating or updating files
#[derive(Clone)]
pub struct WriteTool {
    /// Tool name identifier
    pub tool_name: String,
    /// Description of what this tool does
    pub description: String,
    /// Optional LSP manager for diagnostics
    pub lsp_manager: Option<Arc<LspManager>>,
    /// Timeout for LSP diagnostics
    pub lsp_timeout_ms: u64,
}

/// Write request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRequest {
    /// Path to the file to write
    pub file_path: String,
    /// Content to write to the file
    pub content: String,
}

/// Metadata for write operation
#[derive(Debug, Serialize, Deserialize)]
pub struct WriteMetadata {
    /// File difference in unified format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// Number of lines added
    pub additions: usize,
    /// Number of lines removed
    pub removals: usize,
}

/// Result of writing a file
#[derive(Debug, Serialize, Deserialize)]
pub struct WriteResult {
    /// Success message
    pub content: String,
    /// Metadata about the write operation
    pub metadata: WriteMetadata,
    /// LSP diagnostics (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<DiagnosticSummary>,
    /// Summary of the result
    pub response_summary: String,
}

impl WriteTool {
    /// Create a new WriteTool by loading configuration from config.toml
    ///
    /// If config.toml is not found or fails to parse, falls back to hardcoded defaults.
    pub fn new() -> Self {
        Self::new_with_lsp(None)
    }

    /// Create a new WriteTool with optional LSP manager
    pub fn new_with_lsp(lsp_manager: Option<Arc<LspManager>>) -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.tool_write.tool_name,
                description: config.tool_write.description,
                lsp_manager,
                lsp_timeout_ms: config.lsp.timeout_ms,
            },
            Err(e) => {
                log::warn!(
                    "Failed to load config.toml: {}, using hardcoded defaults",
                    e
                );
                let mut default = Self::default();
                default.lsp_manager = lsp_manager;
                default
            }
        }
    }

    /// Create a new WriteTool from a specific AppConfig
    ///
    /// # Arguments
    /// * `config` - The application configuration
    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.tool_write.tool_name.clone(),
            description: config.tool_write.description.clone(),
            lsp_manager: None,
            lsp_timeout_ms: config.lsp.timeout_ms,
        }
    }

    /// Create a new WriteTool with custom configuration
    #[allow(dead_code)]
    pub fn with_config(tool_name: String, description: String) -> Self {
        Self {
            tool_name,
            description,
            lsp_manager: None,
            lsp_timeout_ms: 5000,
        }
    }

    /// Write file content with advanced features
    ///
    /// # Arguments
    /// * `request` - Write request parameters
    ///
    /// # Returns
    /// * `Result<WriteResult>` - The result containing write metadata
    fn run_write(&self, request: &WriteRequest) -> Result<WriteResult> {
        // 1) Canonicalize and restrict path to current workspace
        let policy = PathPolicy::new()?;
        let normalized = policy.resolve(&request.file_path)?;
        let path = normalized.as_path();
        let absolute_path_str = normalized.to_string_lossy().to_string();

        let file_exists = path.exists();
        let diff_output;
        let additions;
        let mut removals = 0;

        if file_exists {
            // Read original content
            let original_content =
                fs::read_to_string(path).context("Failed to read existing file")?;

            // Content consistency check - if content is identical, skip write
            if original_content == request.content {
                return Ok(WriteResult {
                    content: "File content unchanged, no write performed".to_string(),
                    metadata: WriteMetadata {
                        diff: None,
                        additions: 0,
                        removals: 0,
                    },
                    diagnostics: None,
                    response_summary: "0 lines (unchanged)".to_string(),
                });
            }

            // File modification conflict check
            {
                let history = FILE_WRITE_HISTORY.lock().unwrap();
                if let Some(entry) = history.get(&absolute_path_str) {
                    // Check if file was modified externally since our last write
                    let current_mod_time = PathSecurity::get_modification_time(path)?;

                    if current_mod_time > entry.mod_time {
                        anyhow::bail!(
                            "File '{}' has been modified externally since last write. \
                            Last write: {:?}, Current modification: {:?}",
                            request.file_path,
                            entry.write_time,
                            current_mod_time
                        );
                    }
                }
            }

            // Calculate diff
            let diff = Self::calculate_diff(&original_content, &request.content);

            // Calculate additions and removals
            let (add, rem) = Self::count_changes(&original_content, &request.content);
            additions = add;
            removals = rem;

            diff_output = Some(diff);
        } else {
            // For new files, all lines are additions
            additions = request.content.lines().count();

            // Generate diff for new file
            let diff = Self::calculate_diff("", &request.content);
            diff_output = Some(diff);
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).context("Failed to create parent directories")?;
            }
        } else {
            // Should not happen because we validated above
            anyhow::bail!(
                "Invalid path '{}': missing parent directory",
                request.file_path
            );
        }

        // Write the file
        fs::write(path, &request.content).context("Failed to write file")?;

        // Update file history
        let current_mod_time = PathSecurity::get_modification_time(path)?;
        {
            let mut history = FILE_WRITE_HISTORY.lock().unwrap();
            history.insert(
                absolute_path_str.clone(),
                FileHistoryEntry {
                    write_time: SystemTime::now(),
                    mod_time: current_mod_time,
                },
            );
        }

        let success_message = if file_exists {
            format!("Successfully updated file: {}", request.file_path)
        } else {
            format!("Successfully created file: {}", request.file_path)
        };

        // Calculate line count for summary
        let line_count = request.content.lines().count();

        let mut result = WriteResult {
            content: success_message,
            metadata: WriteMetadata {
                diff: diff_output,
                additions,
                removals,
            },
            diagnostics: None,
            response_summary: format!("{} lines", line_count),
        };

        // Collect LSP diagnostics if available
        if let Some(lsp_manager) = &self.lsp_manager {
            result.diagnostics = self.collect_diagnostics(lsp_manager, &absolute_path_str);
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

        // Spawn a dedicated thread to run the async LSP call.
        // This is necessary because run_write is synchronous but called from an async context (Agent),
        // and calling block_on directly would panic.
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;

            rt.block_on(async {
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
        })
        .join()
        .unwrap_or(None)
    }

    /// Calculate diff between old and new content
    ///
    /// # Arguments
    /// * `old_content` - Original content
    /// * `new_content` - New content
    ///
    /// # Returns
    /// * Diff string in unified format with + and - markers
    fn calculate_diff(old_content: &str, new_content: &str) -> String {
        let diff = TextDiff::from_lines(old_content, new_content);

        let mut diff_output = String::new();

        for op in diff.ops() {
            for change in diff.iter_changes(op) {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };

                if let Some(line) = change.as_str() {
                    // Remove trailing newline for display
                    let line = line.trim_end_matches('\n');

                    diff_output.push_str(sign);
                    diff_output.push(' ');
                    diff_output.push_str(line);
                    diff_output.push('\n');
                }
            }
        }

        diff_output
    }

    /// Count additions and removals between old and new content
    ///
    /// # Arguments
    /// * `old_content` - Original content
    /// * `new_content` - New content
    ///
    /// # Returns
    /// * Tuple of (additions, removals)
    fn count_changes(old_content: &str, new_content: &str) -> (usize, usize) {
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

        (additions, removals)
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
                            "description": "The absolute path to the file to write"
                        },
                        "content": {
                            "type": "string",
                            "description": "The content to write to the file"
                        }
                    },
                    "required": ["file_path", "content"]
                }
            }
        })
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self {
            tool_name: "write".to_string(),
            description: "File writing tool that creates or updates files in the filesystem."
                .to_string(),
            lsp_manager: None,
            lsp_timeout_ms: 5000,
        }
    }
}

impl ToolSpec for WriteTool {
    type Args = WriteRequest;

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
        let result = self.run_write(&args)?;
        let response_summary = result.response_summary.clone();
        let stdout = result.content.clone();
        let data = serde_json::to_value(result)?;
        Ok(ToolResult::ok(
            self.tool_name.clone(),
            self.kind(),
            self.operation(),
            stdout,
            data,
        )
        .with_summary(response_summary))
    }
}
