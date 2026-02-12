use crate::llm::config::AppConfig;
use crate::llm::utils::path_policy::PathPolicy;
use crate::llm::tools::tool_trait::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Ls tool for displaying directory structure in tree format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsTool {
    /// Tool name identifier
    pub tool_name: String,
    /// Description of what this tool does
    pub description: String,
    /// Maximum number of files to return
    pub max_ls_files: usize,
    /// Default ignore patterns (glob patterns)
    pub default_ignore: Vec<String>,
}

/// Input arguments for the ls tool
#[derive(Debug, Serialize, Deserialize)]
pub struct LsArgs {
    /// Optional path to list. If None, lists current directory
    #[serde(default)]
    pub path: Option<String>,
    /// Optional array of glob patterns to ignore
    #[serde(default)]
    pub ignore: Option<Vec<String>>,
}

/// Result of executing ls command (matching OpenCode format)
#[derive(Debug, Serialize, Deserialize)]
pub struct LsResult {
    /// Tree-formatted content
    pub content: String,
    /// Metadata about the listing
    pub metadata: LsMetadata,
    /// Summary of the result
    pub response_summary: String,
}

/// Metadata for ls result
#[derive(Debug, Serialize, Deserialize)]
pub struct LsMetadata {
    /// Total number of items found
    pub count: usize,
    /// Whether the result was truncated
    pub truncated: bool,
}

/// Internal structure for building the tree
struct TreeNode {
    name: String,
    #[allow(dead_code)]
    path: PathBuf,
    is_dir: bool,
    children: Vec<TreeNode>,
}

impl LsTool {
    /// Create a new LsTool by loading configuration from config.toml
    ///
    /// If config.toml is not found or fails to parse, falls back to hardcoded defaults.
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.tool_ls.tool_name,
                description: config.tool_ls.description,
                max_ls_files: config.tool_ls.max_ls_files,
                default_ignore: config.tool_ls.default_ignore,
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

    /// Create a new LsTool from a specific AppConfig
    ///
    /// # Arguments
    /// * `config` - The application configuration
    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.tool_ls.tool_name.clone(),
            description: config.tool_ls.description.clone(),
            max_ls_files: config.tool_ls.max_ls_files,
            default_ignore: config.tool_ls.default_ignore.clone(),
        }
    }

    /// Create a new LsTool with custom configuration
    #[allow(dead_code)]
    pub fn with_config(
        tool_name: String,
        description: String,
        max_ls_files: usize,
        default_ignore: Vec<String>,
    ) -> Self {
        Self {
            tool_name,
            description,
            max_ls_files,
            default_ignore,
        }
    }

    /// Check if a path should be ignored based on patterns
    ///
    /// # Arguments
    /// * `path` - Path to check
    /// * `patterns` - Glob patterns to match against
    /// * `base_path` - Base path for relative pattern matching
    fn should_ignore(path: &Path, patterns: &[String], base_path: &Path) -> bool {
        // Get the file name
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return false,
        };

        // Auto-hide hidden files (starting with '.')
        if file_name.starts_with('.') {
            return true;
        }

        // Get relative path for pattern matching
        let rel_path = match path.strip_prefix(base_path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => path.to_string_lossy().to_string(),
        };

        // Check against glob patterns
        for pattern in patterns {
            // Use simple glob matching
            if Self::glob_match(pattern, &rel_path) || Self::glob_match(pattern, &file_name) {
                return true;
            }
        }

        false
    }

    /// Simple glob pattern matching
    ///
    /// Supports: *, **, ?, and basic patterns
    fn glob_match(pattern: &str, text: &str) -> bool {
        // Handle ** pattern (matches anything including /)
        if pattern.contains("**") {
            let parts: Vec<&str> = pattern.split("**").collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1].trim_start_matches('/');

                if !prefix.is_empty() && !text.starts_with(prefix.trim_end_matches('/')) {
                    return false;
                }
                if !suffix.is_empty() && !text.ends_with(suffix) {
                    return false;
                }
                return true;
            }
        }

        // Simple wildcard matching for * and ?
        let pattern_chars: Vec<char> = pattern.chars().collect();
        let text_chars: Vec<char> = text.chars().collect();

        Self::wildcard_match(&pattern_chars, &text_chars, 0, 0)
    }

    /// Recursive wildcard matching
    fn wildcard_match(pattern: &[char], text: &[char], p_idx: usize, t_idx: usize) -> bool {
        if p_idx >= pattern.len() {
            return t_idx >= text.len();
        }

        match pattern[p_idx] {
            '*' => {
                // Try matching zero or more characters
                for i in t_idx..=text.len() {
                    if Self::wildcard_match(pattern, text, p_idx + 1, i) {
                        return true;
                    }
                }
                false
            }
            '?' => {
                if t_idx >= text.len() {
                    false
                } else {
                    Self::wildcard_match(pattern, text, p_idx + 1, t_idx + 1)
                }
            }
            c => {
                if t_idx >= text.len() || text[t_idx] != c {
                    false
                } else {
                    Self::wildcard_match(pattern, text, p_idx + 1, t_idx + 1)
                }
            }
        }
    }

    /// Build a tree structure from directory contents
    ///
    /// # Arguments
    /// * `root_path` - Root directory to scan
    /// * `ignore_patterns` - Patterns to ignore
    fn build_tree(&self, root_path: &Path, ignore_patterns: &[String]) -> Result<TreeNode> {
        let root_name = root_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        let mut root = TreeNode {
            name: root_name,
            path: root_path.to_path_buf(),
            is_dir: true,
            children: Vec::new(),
        };

        // Collect all entries at depth 1 first
        let mut entries: Vec<(PathBuf, bool)> = Vec::new();

        if let Ok(dir_entries) = fs::read_dir(root_path) {
            for entry in dir_entries.flatten() {
                let path = entry.path();

                // Skip if should be ignored
                if Self::should_ignore(&path, ignore_patterns, root_path) {
                    continue;
                }

                let is_dir = path.is_dir();
                entries.push((path, is_dir));
            }
        }

        // Sort entries: directories first, then alphabetically
        entries.sort_by(|a, b| match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.file_name().cmp(&b.0.file_name()),
        });

        // Recursively build children
        for (path, is_dir) in entries {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if is_dir {
                // Recursively build subdirectory tree
                match self.build_tree(&path, ignore_patterns) {
                    Ok(child_node) => root.children.push(child_node),
                    Err(e) => {
                        log::warn!("Failed to read directory {}: {}", path.display(), e);
                        // Add as empty dir node
                        root.children.push(TreeNode {
                            name: name + "/",
                            path,
                            is_dir: true,
                            children: Vec::new(),
                        });
                    }
                }
            } else {
                root.children.push(TreeNode {
                    name,
                    path,
                    is_dir: false,
                    children: Vec::new(),
                });
            }
        }

        Ok(root)
    }

    /// Render tree to string with box-drawing characters
    ///
    /// # Arguments
    /// * `node` - Tree node to render
    /// * `prefix` - Current line prefix
    /// * `is_root` - Whether this is the root node
    /// * `output` - Output buffer
    /// * `count` - Current item count
    /// * `max_items` - Maximum items to render
    fn render_tree(
        node: &TreeNode,
        prefix: &str,
        is_root: bool,
        output: &mut String,
        count: &mut usize,
        max_items: usize,
    ) -> bool {
        if *count >= max_items {
            return true; // truncated
        }

        // Render current node
        if is_root {
            output.push_str(".\n");
            *count += 1;
        }

        // Render children
        if node.is_dir && !node.children.is_empty() {
            for (i, child) in node.children.iter().enumerate() {
                if *count >= max_items {
                    return true;
                }

                let is_last_child = i == node.children.len() - 1;
                let connector = if is_last_child {
                    "└── "
                } else {
                    "├── "
                };
                let name_display = if child.is_dir {
                    format!("{}/", child.name)
                } else {
                    child.name.clone()
                };

                // Print current child
                output.push_str(&format!("{}{}{}\n", prefix, connector, name_display));
                *count += 1;

                // Recursively render child's children
                if child.is_dir && !child.children.is_empty() {
                    let child_prefix = if is_last_child {
                        format!("{}    ", prefix)
                    } else {
                        format!("{}│   ", prefix)
                    };

                    let truncated =
                        Self::render_tree(child, &child_prefix, false, output, count, max_items);
                    if truncated {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Execute ls command at the given path
    ///
    /// # Arguments
    /// * `path` - Optional path to list. If None, lists current directory
    /// * `ignore` - Optional array of glob patterns to ignore
    ///
    /// # Returns
    /// * `Result<LsResult>` - The result containing tree structure and metadata
    fn run_ls(&self, path: Option<&str>, ignore: Option<Vec<String>>) -> Result<LsResult> {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        let policy = PathPolicy::new()?;
        let target_path = match path {
            Some(p) if !p.is_empty() => policy
                .resolve(p)
                .with_context(|| format!("Path is outside workspace: {}", p))?,
            _ => cwd,
        };

        if !target_path.exists() {
            anyhow::bail!("Path does not exist: {}", target_path.display());
        }

        if !target_path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", target_path.display());
        }

        // Merge default ignore patterns with user-provided ones
        let mut ignore_patterns = self.default_ignore.clone();
        if let Some(user_ignore) = ignore {
            ignore_patterns.extend(user_ignore);
        }

        // Build tree structure
        let tree = self.build_tree(&target_path, &ignore_patterns)?;

        // Render tree to string
        let mut output = String::new();
        let mut count = 0;
        let truncated =
            Self::render_tree(&tree, "", true, &mut output, &mut count, self.max_ls_files);

        if truncated {
            output.push_str(&format!(
                "\n... (truncated at {} items)\n",
                self.max_ls_files
            ));
        }

        Ok(LsResult {
            content: output,
            metadata: LsMetadata { count, truncated },
            response_summary: format!("{} files", count),
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
                        "path": {
                            "type": "string",
                            "description": "The directory path to list. Defaults to current directory if not specified."
                        },
                        "ignore": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of glob patterns to ignore (e.g., [\"*.log\", \"tmp/**\"]). These patterns are added to the default ignore list."
                        }
                    },
                    "required": []
                }
            }
        })
    }
}

impl Default for LsTool {
    fn default() -> Self {
        Self {
            tool_name: "ls".to_string(),
            description: "List directory contents in tree structure. Shows files and directories with automatic filtering of hidden files and common ignore directories.".to_string(),
            max_ls_files: 1000,
            default_ignore: vec![
                "node_modules/**".to_string(),
                "__pycache__/**".to_string(),
                ".git/**".to_string(),
                "*.pyc".to_string(),
                ".DS_Store".to_string(),
                "target/**".to_string(),
                "dist/**".to_string(),
                "build/**".to_string(),
                ".vscode/**".to_string(),
                ".idea/**".to_string(),
            ],
        }
    }
}

impl ToolSpec for LsTool {
    type Args = LsArgs;

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
        let result = self.run_ls(args.path.as_deref(), args.ignore)?;
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
