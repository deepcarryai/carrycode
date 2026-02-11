use crate::llm::config::AppConfig;
use crate::llm::utils::file_tracker::FILE_READ_TRACKER;
use crate::policy::path_policy::PathPolicy;
use crate::policy::policy_text::truncate_to_width_with_ellipsis;
use crate::llm::tools::builtin::core_tool_base::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// View tool for reading file contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreViewTool {
    /// Tool name identifier
    pub tool_name: String,
    /// Description of what this tool does
    pub description: String,
    /// Maximum file size to read
    pub max_file_size: usize,
}

use crate::llm::utils::serde_util::deserialize_usize_opt_lax;

/// View request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreViewRequest {
    /// Path to the file to view
    pub file_path: String,
    /// Line number to start reading from (1-based, matching OpenCode)
    #[serde(default, deserialize_with = "deserialize_usize_opt_lax")]
    pub offset: Option<usize>,
    /// Number of lines to read
    #[serde(default, deserialize_with = "deserialize_usize_opt_lax")]
    pub limit: Option<usize>,
}

/// Metadata for view result (OpenCode format)
#[derive(Debug, Serialize, Deserialize)]
pub struct ViewMetadata {
    /// File path
    pub filepath: String,
    /// Preview of file content (first line or description)
    pub preview: String,
    /// Original file content (without line numbers)
    pub content_original: String,
}

/// Result of viewing a file (OpenCode format: {content, metadata})
#[derive(Debug, Serialize, Deserialize)]
pub struct ViewResult {
    /// File content with line numbers (tab-separated format)
    pub content: String,
    /// Metadata about the file
    pub metadata: ViewMetadata,
    /// Summary of the result
    pub response_summary: String,
}

/// Common image file extensions
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "webp", "tiff", "tif",
];

use crate::llm::utils::string_util::truncate_utf8_with_ellipsis;

impl CoreViewTool {
    /// Create a new ViewTool by loading configuration from config.toml
    ///
    /// If config.toml is not found or fails to parse, falls back to hardcoded defaults.
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.core_view.tool_name,
                description: config.core_view.description,
                max_file_size: config.core_view.max_file_size,
            },
            Err(e) => {
                log::warn!(
                    "Failed to load config.toml: {}, using hardcoded defaults",
                    e
                );
                Self::default()
            }
        }
    }

    /// Create a new ViewTool from a specific AppConfig
    ///
    /// # Arguments
    /// * `config` - The application configuration
    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.core_view.tool_name.clone(),
            description: config.core_view.description.clone(),
            max_file_size: config.core_view.max_file_size,
        }
    }

    /// Create a new ViewTool with custom configuration
    #[allow(dead_code)]
    pub fn with_config(tool_name: String, description: String, max_file_size: usize) -> Self {
        Self {
            tool_name,
            description,
            max_file_size,
        }
    }

    /// Check if a file is an image based on extension
    fn is_image_file(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            IMAGE_EXTENSIONS.contains(&ext_str.as_str())
        } else {
            false
        }
    }

    /// Find similar file names in the same directory
    fn find_similar_files(path: &Path) -> Vec<String> {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_lowercase(),
            None => return vec![],
        };

        let parent_dir = match path.parent() {
            Some(dir) => dir,
            None => return vec![],
        };

        let mut similar_files = Vec::new();

        if let Ok(entries) = fs::read_dir(parent_dir) {
            for entry in entries.flatten() {
                let entry_name = entry.file_name().to_string_lossy().to_lowercase();

                // Simple similarity check: check if names share common substrings
                if entry_name != file_name && Self::is_similar(&file_name, &entry_name) {
                    if let Ok(full_path) = entry.path().canonicalize() {
                        similar_files.push(full_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        // Limit to top 5 suggestions
        similar_files.truncate(5);
        similar_files
    }

    /// Check if two strings are similar (simple Levenshtein-like check)
    fn is_similar(s1: &str, s2: &str) -> bool {
        // Check if one contains significant portion of the other
        let min_len = s1.len().min(s2.len());
        if min_len < 3 {
            return false;
        }

        // Calculate similarity score
        let common_prefix = s1
            .chars()
            .zip(s2.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let common_suffix = s1
            .chars()
            .rev()
            .zip(s2.chars().rev())
            .take_while(|(a, b)| a == b)
            .count();

        let similarity = (common_prefix + common_suffix) as f64 / min_len as f64;
        similarity > 0.5
    }

    /// Read file content
    ///
    /// # Arguments
    /// * `request` - View request parameters
    ///
    /// # Returns
    /// * `Result<ViewResult>` - The result containing file content in OpenCode format
    fn run_view(&self, request: &CoreViewRequest) -> Result<ViewResult> {
        // Convert to absolute path
        let path_policy = PathPolicy::new()?;
        let absolute_path = path_policy.resolve(&request.file_path)?;
        let absolute_path_str = absolute_path.to_string_lossy().to_string();

        // Try to validate in workspace, but if it fails (e.g., for test directories),
        // just use absolute path
        let path_buf = absolute_path;
        let path = path_buf.as_path();

        // Check if file exists
        if !path.exists() {
            // Find similar files and provide suggestions
            let similar_files = Self::find_similar_files(path);

            let mut error_msg = format!("File not found: {}", request.file_path);
            if !similar_files.is_empty() {
                error_msg.push_str("\n\nDid you mean one of these?");
                for (i, similar) in similar_files.iter().enumerate() {
                    error_msg.push_str(&format!("\n  {}. {}", i + 1, similar));
                }
            }

            anyhow::bail!(error_msg);
        }

        // Check if it's an image file
        if Self::is_image_file(path) {
            return Ok(ViewResult {
                content: format!(
                    "This is an image file: {}\n\nImage files cannot be displayed as text. \
                    The file has extension: {}",
                    path.display(),
                    path.extension().unwrap_or_default().to_string_lossy()
                ),
                metadata: ViewMetadata {
                    filepath: absolute_path_str.clone(),
                    preview: format!(
                        "[Image file: {}]",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    content_original: String::new(),
                },
                response_summary: "Image file".to_string(),
            });
        }

        let metadata = fs::metadata(path).context("Failed to read file metadata")?;
        let file_size = metadata.len() as usize;

        if file_size > self.max_file_size {
            anyhow::bail!(
                "File too large ({} bytes). Maximum size is {} bytes.",
                file_size,
                self.max_file_size
            );
        }

        // Record file read for read-before-write validation
        {
            let mut tracker = FILE_READ_TRACKER.lock().unwrap();
            tracker.record_read(&absolute_path_str);
        }

        // Stream read file with offset and limit
        let offset = request.offset.unwrap_or(0);
        let limit = request.limit.unwrap_or(2000);

        let file = File::open(path).context("Failed to open file")?;
        let reader = BufReader::new(file);
        let mut lines_iter = reader.lines();

        // Skip offset lines
        let mut current_line = 0;
        while current_line < offset {
            if lines_iter.next().is_none() {
                break;
            }
            current_line += 1;
        }

        // Check if offset is beyond file
        if current_line < offset {
            // Need to read full content for content_original
            let content = fs::read_to_string(path).context("Failed to read file")?;
            return Ok(ViewResult {
                content: String::new(),
                metadata: ViewMetadata {
                    filepath: absolute_path_str.clone(),
                    preview: "(empty or offset beyond file)".to_string(),
                    content_original: content,
                },
                response_summary: "0 lines".to_string(),
            });
        }

        // Read limited lines and format with line numbers
        let mut content_with_numbers = String::new();
        let mut lines_read = 0;
        let mut first_line_for_preview = String::new();
        let mut has_more = false;

        for line_result in lines_iter {
            if lines_read >= limit {
                has_more = true;
                break;
            }

            let line = line_result.context("Failed to read line")?;
            let line_num = offset + lines_read + 1;

            let truncated_line = truncate_utf8_with_ellipsis(&line, 2000);

            // Save first line for preview
            if lines_read == 0 {
                first_line_for_preview = truncated_line.clone();
            }

            content_with_numbers.push_str(&format!("{}  {}\n", line_num, truncated_line));
            lines_read += 1;
        }

        // Add truncation notice if there are more lines
        if has_more {
            content_with_numbers.push_str(&format!(
                "\n(File has more lines. Use 'offset' parameter to read beyond line {})",
                offset + lines_read
            ));
        }

        // Generate preview
        let preview = truncate_to_width_with_ellipsis(&first_line_for_preview, 80).into_owned();

        // Read full content for content_original (required for compatibility)
        let content_original = fs::read_to_string(path).context("Failed to read file")?;

        Ok(ViewResult {
            content: content_with_numbers,
            metadata: ViewMetadata {
                filepath: absolute_path_str,
                preview,
                content_original,
            },
            response_summary: format!("{} lines", lines_read),
        })
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
                            "description": "The path to the file to view (will be converted to absolute path). Must be within current working directory."
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Optional line number to start reading from (0-based). Defaults to 0."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Optional number of lines to read. Defaults to 2000."
                        }
                    },
                    "required": ["file_path"]
                }
            }
        })
    }
}

impl Default for CoreViewTool {
    fn default() -> Self {
        Self {
            tool_name: "core_view".to_string(),
            description:
                "[CORE SYSTEM] File viewing tool that reads and displays the contents of files with line numbers."
                    .to_string(),
            max_file_size: 256000, // ~250KB, matching OpenCode
        }
    }
}

impl ToolSpec for CoreViewTool {
    type Args = CoreViewRequest;

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
        self.to_tool_definition_json()
    }

    fn run(&self, args: Self::Args, _confirmed: bool) -> Result<ToolResult> {
        let result = self.run_view(&args)?;
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
