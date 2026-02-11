use crate::llm::config::AppConfig;
use crate::policy::path_policy::PathPolicy;
use crate::llm::tools::builtin::core_tool_base::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// Glob tool for finding files by pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreGlobTool {
    /// Tool name identifier
    pub tool_name: String,
    /// Description of what this tool does
    pub description: String,
    /// Maximum number of results to return
    pub max_glob_results: usize,
}

/// Glob request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreGlobRequest {
    /// Glob pattern to match against file paths
    pub pattern: String,
    /// Starting directory for search (defaults to current working directory)
    pub path: Option<String>,
}

/// A single file match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobMatch {
    /// File path (relative to search directory)
    pub path: String,
    /// Last modified time (seconds since epoch)
    pub modified: Option<u64>,
    /// File size in bytes
    pub size: Option<u64>,
}

/// Result of glob search
#[derive(Debug, Serialize, Deserialize)]
pub struct GlobResult {
    /// List of matching files
    pub matches: Vec<GlobMatch>,
    /// Whether results were truncated due to max limit
    pub truncated: bool,
    /// Total number of matches found
    pub total_count: usize,
    /// Search pattern used
    pub pattern: String,
    /// Summary of the result
    pub response_summary: String,
}

impl CoreGlobTool {
    /// Create a new GlobTool by loading configuration from config.toml
    ///
    /// If config.toml is not found or fails to parse, falls back to hardcoded defaults.
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.core_glob.tool_name,
                description: config.core_glob.description,
                max_glob_results: config.core_glob.max_glob_results,
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

    /// Create a new GlobTool from a specific AppConfig
    ///
    /// # Arguments
    /// * `config` - The application configuration
    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.core_glob.tool_name.clone(),
            description: config.core_glob.description.clone(),
            max_glob_results: config.core_glob.max_glob_results,
        }
    }

    /// Create a new GlobTool with custom configuration
    #[allow(dead_code)]
    pub fn with_config(tool_name: String, description: String, max_glob_results: usize) -> Self {
        Self {
            tool_name,
            description,
            max_glob_results,
        }
    }

    /// Execute glob search
    ///
    /// # Arguments
    /// * `request` - Glob search parameters
    ///
    /// # Returns
    /// * `Result<GlobResult>` - The result containing matches and metadata
    pub fn run_glob(&self, request: &CoreGlobRequest) -> Result<GlobResult> {
        // Try ripgrep first if available
        if let Ok(result) = self.try_ripgrep(request) {
            return Ok(result);
        }

        // Fallback to native implementation
        self.run_glob_native(request)
    }

    /// Try to use ripgrep for better performance
    fn try_ripgrep(&self, request: &CoreGlobRequest) -> Result<GlobResult> {
        let policy = PathPolicy::new()?;
        let base_path = request
            .path
            .as_deref()
            .map(|p| policy.resolve(p))
            .transpose()?
            .unwrap_or_else(|| policy.resolve(".").expect("workspace path should resolve"));

        let mut cmd = Command::new("rg");
        cmd.arg("--files")
            .arg("--glob")
            .arg(&request.pattern)
            .arg(&base_path);

        let output = cmd.output().context("Failed to execute ripgrep")?;

        // Exit code 1 means no matches, fall back to native
        if output.status.code() == Some(2)
            || (!output.status.success() && output.stdout.is_empty() && !output.stderr.is_empty())
        {
            anyhow::bail!("ripgrep error: {}", String::from_utf8_lossy(&output.stderr));
        }

        if output.stdout.is_empty() {
            anyhow::bail!("ripgrep found no matches, falling back to native");
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let mut file_list: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for line in content.lines() {
            let path = PathBuf::from(line);

            // Skip hidden files (files starting with .)
            if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }

            if path.is_file() {
                let mtime = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                file_list.push((path, mtime));
            }
        }

        // Sort by modification time (newest first)
        file_list.sort_by(|a, b| b.1.cmp(&a.1));

        let mut matches = Vec::new();
        let total_count = file_list.len();

        for (path, _) in file_list.into_iter().take(self.max_glob_results) {
            let metadata = fs::metadata(&path).ok();
            let modified = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let size = metadata.as_ref().map(|m| m.len());

            let relative_path = path
                .strip_prefix(&base_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            matches.push(GlobMatch {
                path: relative_path,
                modified,
                size,
            });
        }

        Ok(GlobResult {
            matches,
            truncated: total_count > self.max_glob_results,
            total_count,
            pattern: request.pattern.clone(),
            response_summary: format!("{} lines", total_count),
        })
    }

    /// Native glob implementation (fallback)
    fn run_glob_native(&self, request: &CoreGlobRequest) -> Result<GlobResult> {
        let policy = PathPolicy::new()?;
        let base_path = request
            .path
            .as_deref()
            .map(|p| policy.resolve(p))
            .transpose()?
            .unwrap_or_else(|| policy.resolve(".").expect("workspace path should resolve"));

        let mut matches: Vec<GlobMatch> = Vec::new();
        let mut total_count = 0;

        // Parse the glob pattern to determine search strategy
        let pattern_parts: Vec<&str> = request.pattern.split('/').collect();
        let has_recursive = pattern_parts.contains(&"**");

        // Build the walker
        let walker = WalkDir::new(&base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                // Don't skip the root directory itself
                if entry.depth() == 0 {
                    return true;
                }
                // Skip hidden files/directories
                if entry
                    .file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
                {
                    return false;
                }
                // Skip common system directories
                if entry.file_name() == "__pycache__"
                    || entry.file_name() == "node_modules"
                    || entry.file_name() == "target"
                    || entry.file_name() == ".git"
                {
                    return false;
                }
                true
            })
            .filter_map(|e| e.ok());

        for entry in walker {
            let path = entry.path();

            // Skip directories unless pattern explicitly matches them
            if path.is_dir() {
                continue;
            }

            // Check if path matches pattern
            if !glob_pattern_match(&request.pattern, path, &base_path, has_recursive) {
                continue;
            }

            total_count += 1;

            // Get file metadata
            let metadata = entry.metadata().ok();
            let modified = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let size = metadata.as_ref().map(|m| m.len());

            // Get relative path
            let relative_path = path
                .strip_prefix(&base_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            matches.push(GlobMatch {
                path: relative_path,
                modified,
                size,
            });

            // Stop if we hit max results
            if matches.len() >= self.max_glob_results {
                break;
            }
        }

        // Sort by modification time (newest first)
        matches.sort_by(|a, b| b.modified.cmp(&a.modified));

        let truncated = total_count > self.max_glob_results;

        Ok(GlobResult {
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
                            "description": "Glob pattern to match files (e.g., '*.rs', '**/*.js', 'src/**/*.{ts,tsx}')"
                        },
                        "path": {
                            "type": "string",
                            "description": "The directory path to start the search from. Defaults to current working directory."
                        }
                    },
                    "required": ["pattern"]
                }
            }
        })
    }
}

impl Default for CoreGlobTool {
    fn default() -> Self {
        Self {
            tool_name: "core_glob".to_string(),
            description: "[CORE SYSTEM] Fast file pattern matching tool that finds files by name and pattern."
                .to_string(),
            max_glob_results: 1000,
        }
    }
}

impl ToolSpec for CoreGlobTool {
    type Args = CoreGlobRequest;

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
        let result = self.run_glob(&args)?;
        let response_summary = result.response_summary.clone();
        let mut stdout = result
            .matches
            .iter()
            .take(200)
            .map(|m| m.path.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if result.matches.len() > 200 {
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
fn glob_pattern_match(pattern: &str, path: &Path, base_path: &Path, has_recursive: bool) -> bool {
    let relative_path = path
        .strip_prefix(base_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    // Handle different glob patterns
    if has_recursive {
        // Pattern contains **, need to match path segments more carefully
        return glob_recursive_match(pattern, &relative_path);
    }

    // Handle simple file name patterns
    if !pattern.contains('/') {
        // Pattern is just a file name pattern
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        return glob_simple_match(pattern, file_name);
    }

    // Handle path patterns
    glob_simple_match(pattern, &relative_path)
}

/// Simple glob matching for file names or simple patterns
fn glob_simple_match(pattern: &str, text: &str) -> bool {
    // Handle extensions pattern like "*.{js,ts}"
    if pattern.contains('{') && pattern.contains('}') {
        let base: String = pattern.chars().take_while(|c| *c != '*').collect();
        let exts_part: String = pattern.chars().skip_while(|c| *c != '{').collect();

        if exts_part.starts_with('{') && exts_part.ends_with('}') {
            let exts = exts_part[1..exts_part.len() - 1]
                .split(',')
                .map(|e| e.trim())
                .collect::<Vec<_>>();

            return exts
                .iter()
                .any(|ext| text.ends_with(&format!("{}{}", base.replace('*', ""), ext)));
        }
    }

    // Convert glob pattern to regex
    let regex_pattern = convert_glob_to_regex(pattern);
    if let Ok(re) = Regex::new(&regex_pattern) {
        return re.is_match(text);
    }

    // Fallback: exact match
    text == pattern
}

/// Recursive glob matching for patterns with **
fn glob_recursive_match(pattern: &str, path: &str) -> bool {
    // Split pattern by **
    let parts: Vec<&str> = pattern.split("**/").collect();

    if parts.len() == 1 {
        // No ** in pattern, use simple match
        return glob_simple_match(pattern, path);
    }

    // For patterns like "**/*.rs" or "src/**/*.ts"
    let last_part = parts.last().unwrap();
    let first_part = parts.first().unwrap();

    // Check if path starts with first part (if not empty or *)
    if !first_part.is_empty() && *first_part != "*" && !path.starts_with(first_part) {
        return false;
    }

    // Check if any suffix of the path matches the last part
    let path_parts: Vec<&str> = path.split('/').collect();
    for i in 0..path_parts.len() {
        let suffix = path_parts[i..].join("/");
        if glob_simple_match(last_part, &suffix) {
            return true;
        }
    }

    false
}

/// Convert glob pattern to regex pattern
fn convert_glob_to_regex(pattern: &str) -> String {
    let mut regex = String::new();
    regex.push('^');

    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                // Check for **
                if chars.peek() == Some(&'*') {
                    chars.next();
                    regex.push_str(".*");
                } else {
                    regex.push_str("[^/]*");
                }
            }
            '?' => regex.push_str("[^/]"),
            '[' => {
                regex.push('[');
                // Copy everything until ]
                for ch in chars.by_ref() {
                    regex.push(ch);
                    if ch == ']' {
                        break;
                    }
                }
            }
            '.' | '+' | '^' | '$' | '(' | ')' | '|' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }

    regex.push('$');
    regex
}
