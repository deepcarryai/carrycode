use crate::llm::config::AppConfig;
use crate::llm::utils::path_policy::PathPolicy;
use crate::llm::tools::tool_trait::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// Grep tool for searching file contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepTool {
    /// Tool name identifier
    pub tool_name: String,
    /// Description of what this tool does
    pub description: String,
    /// Maximum number of results to return
    pub max_grep_results: usize,
}

use crate::llm::utils::serde_util::{deserialize_bool_lax, deserialize_usize_lax};

/// Grep request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepRequest {
    /// Regex pattern or literal text to search for
    pub pattern: String,
    /// Whether to treat pattern as literal text (no regex interpretation)
    #[serde(default, deserialize_with = "deserialize_bool_lax")]
    pub literal_text: bool,
    /// Starting directory for search (defaults to current working directory)
    pub path: Option<String>,
    /// Glob pattern to filter which files to search (e.g., "*.rs", "*.{js,ts}")
    pub include: Option<String>,
    /// Number of context lines after match
    #[serde(default, deserialize_with = "deserialize_usize_lax")]
    pub context_after: usize,
    /// Number of context lines before match
    #[serde(default, deserialize_with = "deserialize_usize_lax")]
    pub context_before: usize,
}

/// A single grep result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepMatch {
    /// File path (relative to search directory)
    pub path: String,
    /// Line number where match was found
    pub line_number: usize,
    /// Line content containing the match
    pub line: String,
}

/// Result of grep search
#[derive(Debug, Serialize, Deserialize)]
pub struct GrepResult {
    /// List of matches found
    pub matches: Vec<GrepMatch>,
    /// Whether results were truncated due to max limit
    pub truncated: bool,
    /// Total number of matches found
    pub total_count: usize,
    /// Search pattern used
    pub pattern: String,
    /// Summary of the result
    pub response_summary: String,
}

impl GrepTool {
    /// Create a new GrepTool by loading configuration from config.toml
    ///
    /// If config.toml is not found or fails to parse, falls back to hardcoded defaults.
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.tool_grep.tool_name,
                description: config.tool_grep.description,
                max_grep_results: config.tool_grep.max_grep_results,
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

    /// Create a new GrepTool from a specific AppConfig
    ///
    /// # Arguments
    /// * `config` - The application configuration
    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.tool_grep.tool_name.clone(),
            description: config.tool_grep.description.clone(),
            max_grep_results: config.tool_grep.max_grep_results,
        }
    }

    /// Create a new GrepTool with custom configuration
    #[allow(dead_code)]
    pub fn with_config(tool_name: String, description: String, max_grep_results: usize) -> Self {
        Self {
            tool_name,
            description,
            max_grep_results,
        }
    }

    /// Execute grep search
    ///
    /// # Arguments
    /// * `request` - Grep search parameters
    ///
    /// # Returns
    /// * `Result<GrepResult>` - The result containing matches and metadata
    pub fn run_grep(&self, request: &GrepRequest) -> Result<GrepResult> {
        // Try ripgrep first if available
        if let Ok(result) = self.try_ripgrep(request) {
            return Ok(result);
        }

        // Fallback to native implementation
        self.run_grep_native(request)
    }

    /// Try to use ripgrep for better performance
    fn try_ripgrep(&self, request: &GrepRequest) -> Result<GrepResult> {
        let policy = PathPolicy::new()?;
        let base_path = request
            .path
            .as_deref()
            .map(|p| policy.resolve(p))
            .transpose()?
            .unwrap_or_else(|| policy.resolve(".").expect("workspace path should resolve"));

        let mut cmd = Command::new("rg");
        cmd.arg("--line-number")
            .arg("--no-heading")
            .arg("--with-filename");

        if request.literal_text {
            cmd.arg("--fixed-strings");
        }

        if let Some(ref include) = request.include {
            cmd.arg("--glob").arg(include);
        }

        if request.context_after > 0 {
            cmd.arg("-A").arg(request.context_after.to_string());
        }
        if request.context_before > 0 {
            cmd.arg("-B").arg(request.context_before.to_string());
        }

        cmd.arg("--").arg(&request.pattern).arg(&base_path);

        let output = cmd.output().context("Failed to execute ripgrep")?;

        // Exit code 1 means no matches, which is fine - fall back to native
        // Exit code 2+ means error
        if output.status.code() == Some(2)
            || (!output.status.success() && output.stdout.is_empty() && !output.stderr.is_empty())
        {
            anyhow::bail!("ripgrep error: {}", String::from_utf8_lossy(&output.stderr));
        }

        // If no output, fall back to native
        if output.stdout.is_empty() {
            anyhow::bail!("ripgrep found no matches, falling back to native");
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let mut matches = Vec::new();
        let mut total_count = 0;

        for line in content.lines() {
            // Skip separator lines
            if line == "--" {
                continue;
            }

            // Parse format: path:line_number:content
            // Find the first colon (after path)
            if let Some(first_colon) = line.find(':') {
                let path = &line[..first_colon];
                let rest = &line[first_colon + 1..];

                // Find the second colon (after line number)
                if let Some(second_colon) = rest.find(':') {
                    let line_num_str = &rest[..second_colon];
                    let line_content = &rest[second_colon + 1..];

                    if let Ok(line_number) = line_num_str.parse::<usize>() {
                        total_count += 1;
                        let relative_path = PathBuf::from(path)
                            .strip_prefix(&base_path)
                            .unwrap_or(Path::new(path))
                            .to_string_lossy()
                            .to_string();

                        let final_content =
                            if request.context_before == 0 && request.context_after == 0 {
                                line_content.to_string()
                            } else {
                                // Include line number prefix for context
                                format!("{}:{}", line_number, line_content)
                            };

                        matches.push(GrepMatch {
                            path: relative_path,
                            line_number,
                            line: final_content,
                        });

                        if matches.len() >= self.max_grep_results {
                            break;
                        }
                    }
                }
            }
        }

        Ok(GrepResult {
            matches,
            truncated: total_count > self.max_grep_results,
            total_count,
            pattern: request.pattern.clone(),
            response_summary: format!("{} lines", total_count),
        })
    }

    /// Native grep implementation (fallback)
    fn run_grep_native(&self, request: &GrepRequest) -> Result<GrepResult> {
        let base_path = request
            .path
            .as_ref()
            .map(|p| PathBuf::from(p))
            .unwrap_or_else(|| PathBuf::from("."));

        let regex = if request.literal_text {
            // Escape special regex characters for literal search
            let pattern = regex::escape(&request.pattern);
            Regex::new(&pattern).context("Failed to compile literal pattern")?
        } else {
            Regex::new(&request.pattern).context("Failed to compile regex pattern")?
        };

        log::info!(
            "Starting native grep in '{}' for pattern '{}' (literal: {})",
            base_path.display(),
            request.pattern,
            request.literal_text
        );

        let mut file_matches: Vec<(PathBuf, std::time::SystemTime, Vec<(usize, String)>)> =
            Vec::new();
        let mut total_count = 0;

        // Build the walker
        let walker = WalkDir::new(&base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                if entry.depth() == 0 {
                    return true;
                }
                if entry
                    .file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
                {
                    return false;
                }
                true
            })
            .filter_map(|e| e.ok());

        for entry in walker {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            log::debug!("Checking file: {}", path.display());

            // Skip if doesn't match include pattern
            if let Some(ref include) = request.include {
                let matches = glob_match(include, path);
                log::debug!("  Glob match '{}': {}", include, matches);
                if !matches {
                    continue;
                }
            }

            log::debug!("  Reading file: {}", path.display());

            // Try to read file
            let content = match fs::read_to_string(path) {
                Ok(content) => {
                    log::debug!("  File read successfully, {} bytes", content.len());
                    content
                }
                Err(e) => {
                    log::debug!("  Failed to read file: {}", e);
                    continue;
                }
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut file_line_matches = Vec::new();

            // Search for matches
            for (line_number, line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    total_count += 1;
                    log::debug!("  Match found at line {}", line_number + 1);

                    // Collect context lines
                    let line_content = if request.context_before == 0 && request.context_after == 0
                    {
                        // No context, just return the line
                        line.to_string()
                    } else {
                        let mut context_lines = Vec::new();

                        // Before context
                        if request.context_before > 0 {
                            let start = line_number.saturating_sub(request.context_before);
                            for i in start..line_number {
                                context_lines.push(format!("{}-{}", i + 1, lines[i]));
                            }
                        }

                        // Match line
                        context_lines.push(format!("{}:{}", line_number + 1, line));

                        // After context
                        if request.context_after > 0 {
                            let end = (line_number + 1 + request.context_after).min(lines.len());
                            for i in (line_number + 1)..end {
                                context_lines.push(format!("{}-{}", i + 1, lines[i]));
                            }
                        }

                        context_lines.join("\n")
                    };

                    file_line_matches.push((line_number + 1, line_content));

                    if total_count >= self.max_grep_results {
                        break;
                    }
                }
            }

            if !file_line_matches.is_empty() {
                let mtime = fs::metadata(path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                file_matches.push((path.to_path_buf(), mtime, file_line_matches));
            }

            if total_count >= self.max_grep_results {
                break;
            }
        }

        // Sort by modification time (newest first)
        file_matches.sort_by(|a, b| b.1.cmp(&a.1));

        // Flatten to GrepMatch
        let mut matches = Vec::new();
        for (path, _, line_matches) in file_matches {
            let relative_path = path
                .strip_prefix(&base_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            for (line_number, line_content) in line_matches {
                matches.push(GrepMatch {
                    path: relative_path.clone(),
                    line_number,
                    line: line_content,
                });

                if matches.len() >= self.max_grep_results {
                    break;
                }
            }

            if matches.len() >= self.max_grep_results {
                break;
            }
        }

        let truncated = total_count > self.max_grep_results;

        Ok(GrepResult {
            matches,
            truncated,
            total_count,
            pattern: request.pattern.clone(),
            response_summary: format!("{} lines", total_count),
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
                        "pattern": {
                            "type": "string",
                            "description": "The regex pattern or literal text to search for in file contents"
                        },
                        "literal_text": {
                            "type": "boolean",
                            "description": "If true, treats pattern as literal text (no regex interpretation). Recommended for non-regex users.",
                            "default": false
                        },
                        "path": {
                            "type": "string",
                            "description": "The directory path to start the search from. Defaults to current working directory."
                        },
                        "include": {
                            "type": "string",
                            "description": "Glob pattern to filter which files to search (e.g., '*.rs', '*.{js,ts}'). If not specified, searches all files."
                        },
                        "context_after": {
                            "type": "integer",
                            "description": "Number of lines to show after each match (like grep -A)",
                            "default": 0
                        },
                        "context_before": {
                            "type": "integer",
                            "description": "Number of lines to show before each match (like grep -B)",
                            "default": 0
                        }
                    },
                    "required": ["pattern"]
                }
            }
        })
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self {
            tool_name: "grep".to_string(),
            description:
                "Search file contents for text patterns. Returns matching file paths and contexts."
                    .to_string(),
            max_grep_results: 100,
        }
    }
}

impl ToolSpec for GrepTool {
    type Args = GrepRequest;

    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Search
    }

    fn operation(&self) -> ToolOperation {
        ToolOperation::Explored
    }

    fn to_tool_definition(&self) -> serde_json::Value {
        self.to_tool_definition_json()
    }

    fn run(&self, args: Self::Args, _confirmed: bool) -> Result<ToolResult> {
        let result = self.run_grep(&args)?;
        let response_summary = result.response_summary.clone();
        let mut stdout = result
            .matches
            .iter()
            .take(50)
            .map(|m| format!("{}:{}:{}", m.path, m.line_number, m.line))
            .collect::<Vec<_>>()
            .join("\n");
        if result.matches.len() > 50 {
            stdout.push_str("\n...");
        }
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

/// Check if a file path matches a glob pattern
fn glob_match(pattern: &str, path: &Path) -> bool {
    // Simple glob matching
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Handle extensions pattern like "*.{js,ts}"
    if pattern.contains("{") {
        // Parse {ext1,ext2,...}
        let _base: String = pattern.chars().take_while(|c| *c != '*').collect();
        let exts_part: String = pattern.chars().skip_while(|c| *c != '{').collect();

        if exts_part.starts_with('{') && exts_part.ends_with('}') {
            let exts = exts_part[1..exts_part.len() - 1]
                .split(',')
                .map(|e| e.trim())
                .collect::<Vec<_>>();

            return exts
                .iter()
                .any(|ext| file_name.ends_with(&format!(".{}", ext)));
        }
    }

    // Handle simple pattern like "*.rs"
    if pattern.starts_with("*.") {
        let ext = &pattern[2..];
        return file_name.ends_with(ext);
    }

    // Fallback: check if pattern matches file name
    if pattern.contains('*') {
        // Convert glob to simple regex
        let regex_pattern = pattern.replace('*', ".*").replace('?', ".");
        if let Ok(re) = Regex::new(&regex_pattern) {
            return re.is_match(file_name);
        }
    }

    // Exact match
    file_name == pattern
}
