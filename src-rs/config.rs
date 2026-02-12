use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::collections::HashMap;

use crate::lsp::config::LspConfig;

/// User override configuration (restricted fields)
#[derive(Deserialize)]
pub struct UserOverrideConfig {
    // pub theme: Option<String>,
    pub providers: Option<Vec<UserProviderConfig>>,
    #[serde(alias = "mcpServers")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,
}

/// User provider configuration (matching user schema)
#[derive(Deserialize)]
pub struct UserProviderConfig {
    #[serde(alias = "provider_id")]
    pub provider_name: String,
    pub model_name: String,
    pub base_url: String,
    pub api_key: String,
}

impl From<UserProviderConfig> for ProviderConfig {
    fn from(c: UserProviderConfig) -> Self {
        ProviderConfig {
            name: c.provider_name,
            base_url: c.base_url,
            api_key: c.api_key,
            models: vec![c.model_name],
        }
    }
}

/// Prompt plan configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptPlanConfig {
    #[serde(default = "default_prompt_plan_enabled")]
    pub enabled: bool,
    #[serde(default = "default_prompt_plan_name")]
    pub prompt_name: String,
    #[serde(default)]
    pub prompt_template: String,
}

fn default_prompt_plan_enabled() -> bool {
    false
}

fn default_prompt_plan_name() -> String {
    "plan".to_string()
}

/// MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(flatten)]
        _extra: HashMap<String, serde_json::Value>,
    },
    Http {
        #[serde(alias = "mcp_url")]
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(flatten)]
        _extra: HashMap<String, serde_json::Value>,
    },
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider name (e.g., "zhipuai", "openai", "vllm")
    pub name: String,

    /// Base URL for the LLM API
    pub base_url: String,

    /// API key for authentication
    pub api_key: String,

    /// List of supported models
    #[serde(default)]
    pub models: Vec<String>,
}

/// LLM Provider configuration from Config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    /// Provider name (e.g., "zhipuai", "openai", "vllm")
    #[serde(default = "default_provider_name")]
    pub provider_name: String,

    /// Base URL for the LLM API
    pub base_url: String,

    /// API key for authentication
    pub api_key: String,

    /// Model name to use
    pub model_name: String,
}

fn default_provider_name() -> String {
    "openai".to_string()
}

/// Tool LS configuration from Config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLsConfig {
    /// Tool name identifier
    #[serde(default = "default_tool_name")]
    pub tool_name: String,

    /// Maximum number of files to return
    #[serde(default = "default_max_ls_files")]
    pub max_ls_files: usize,

    /// Default ignore patterns (glob patterns)
    #[serde(default = "default_ignore_patterns")]
    pub default_ignore: Vec<String>,

    /// Description of what this tool does
    #[serde(default = "default_description")]
    pub description: String,
}

fn default_tool_name() -> String {
    "ls".to_string()
}

fn default_max_ls_files() -> usize {
    100
}

fn default_description() -> String {
    "List directory contents. Shows files and directories in specified path.".to_string()
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
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
    ]
}

/// Tool Grep configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGrepConfig {
    /// Tool name identifier
    #[serde(default = "default_grep_name")]
    pub tool_name: String,

    /// Maximum number of results to return
    #[serde(default = "default_max_results")]
    pub max_grep_results: usize,

    /// Description of what this tool does
    #[serde(default = "default_grep_desc")]
    pub description: String,
}

fn default_grep_name() -> String {
    "grep".to_string()
}

fn default_max_results() -> usize {
    100
}

fn default_grep_desc() -> String {
    "Search file contents for text patterns. Returns matching file paths and contexts.".to_string()
}

/// Tool Fetch configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFetchConfig {
    /// Tool name identifier
    #[serde(default = "default_fetch_name")]
    pub tool_name: String,

    /// Description of what this tool does
    #[serde(default = "default_fetch_desc")]
    pub description: String,
}

fn default_fetch_name() -> String {
    "fetch".to_string()
}

fn default_fetch_desc() -> String {
    "Fetches content from a URL and returns it in the specified format.".to_string()
}

/// Tool Glob configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGlobConfig {
    /// Tool name identifier
    #[serde(default = "default_glob_name")]
    pub tool_name: String,

    /// Maximum number of results to return
    #[serde(default = "default_max_glob_results")]
    pub max_glob_results: usize,

    /// Description of what this tool does
    #[serde(default = "default_glob_desc")]
    pub description: String,
}

fn default_glob_name() -> String {
    "glob".to_string()
}

fn default_max_glob_results() -> usize {
    100
}

fn default_glob_desc() -> String {
    "Fast file pattern matching tool that finds files by name and pattern.".to_string()
}

/// Tool Bash configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolBashConfig {
    /// Tool name identifier
    #[serde(default = "default_bash_name")]
    pub tool_name: String,

    /// List of banned commands that cannot be executed
    #[serde(default = "default_banned_commands")]
    pub banned_commands: Vec<String>,

    /// List of safe read-only commands that can be executed without confirmation
    #[serde(default = "default_safe_read_only_commands")]
    pub safe_read_only_commands: Vec<String>,

    /// Description of what this tool does
    #[serde(default = "default_bash_desc")]
    pub description: String,
}

fn default_bash_name() -> String {
    "bash".to_string()
}

fn default_banned_commands() -> Vec<String> {
    vec![
        "alias".to_string(),
        "curl".to_string(),
        "curlie".to_string(),
        "wget".to_string(),
        "axel".to_string(),
        "aria2c".to_string(),
        "nc".to_string(),
        "telnet".to_string(),
        "lynx".to_string(),
        "w3m".to_string(),
        "links".to_string(),
        "httpie".to_string(),
        "xh".to_string(),
        "http-prompt".to_string(),
        "chrome".to_string(),
        "firefox".to_string(),
        "safari".to_string(),
    ]
}

fn default_safe_read_only_commands() -> Vec<String> {
    vec![
        "ls".to_string(),
        "echo".to_string(),
        "pwd".to_string(),
        "date".to_string(),
        "cal".to_string(),
        "uptime".to_string(),
        "whoami".to_string(),
        "id".to_string(),
        "groups".to_string(),
        "env".to_string(),
        "printenv".to_string(),
        "set".to_string(),
        "unset".to_string(),
        "which".to_string(),
        "type".to_string(),
        "whereis".to_string(),
        "whatis".to_string(),
        "uname".to_string(),
        "hostname".to_string(),
        "df".to_string(),
        "du".to_string(),
        "free".to_string(),
        "top".to_string(),
        "ps".to_string(),
        "kill".to_string(),
        "killall".to_string(),
        "nice".to_string(),
        "nohup".to_string(),
        "time".to_string(),
        "timeout".to_string(),
        "git status".to_string(),
        "git log".to_string(),
        "git diff".to_string(),
        "git show".to_string(),
        "git branch".to_string(),
        "git tag".to_string(),
        "git remote".to_string(),
        "git ls-files".to_string(),
        "git ls-remote".to_string(),
        "git rev-parse".to_string(),
        "git config --get".to_string(),
        "git config --list".to_string(),
        "git describe".to_string(),
        "git blame".to_string(),
        "git grep".to_string(),
        "git shortlog".to_string(),
    ]
}

fn default_bash_desc() -> String {
    "Executes a given bash command in a persistent shell session.".to_string()
}

/// Tool View configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolViewConfig {
    /// Tool name identifier
    #[serde(default = "default_view_name")]
    pub tool_name: String,

    /// Maximum file size to read (in bytes)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: usize,

    /// Description of what this tool does
    #[serde(default = "default_view_desc")]
    pub description: String,
}

fn default_view_name() -> String {
    "view".to_string()
}

fn default_max_file_size() -> usize {
    256000
}

fn default_view_desc() -> String {
    "File viewing tool that reads and displays the contents of files with line numbers.".to_string()
}

/// Tool Write configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolWriteConfig {
    /// Tool name identifier
    #[serde(default = "default_write_name")]
    pub tool_name: String,

    /// Description of what this tool does
    #[serde(default = "default_write_desc")]
    pub description: String,
}

fn default_write_name() -> String {
    "write".to_string()
}

fn default_write_desc() -> String {
    "File writing tool that creates or updates files in the filesystem.".to_string()
}

/// Tool Edit configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEditConfig {
    /// Tool name identifier
    #[serde(default = "default_edit_name")]
    pub tool_name: String,

    /// Description of what this tool does
    #[serde(default = "default_edit_desc")]
    pub description: String,
}

fn default_edit_name() -> String {
    "edit".to_string()
}

fn default_edit_desc() -> String {
    "Edits files by replacing text, creating new files, or deleting content.".to_string()
}

/// Tool TodoWrite configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTodoWriteConfig {
    #[serde(default = "default_todo_write_name")]
    pub tool_name: String,
    #[serde(default = "default_todo_write_desc")]
    pub description: String,
}

fn default_todo_write_name() -> String {
    "todo_write".to_string()
}

fn default_todo_write_desc() -> String {
    "Manage task lists and track progress".to_string()
}

/// Welcome configuration from Config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeConfig {
    #[serde(default = "default_banner")]
    pub banner: Vec<String>,
    #[serde(default)]
    pub tips: Vec<String>,
    pub theme: Option<String>,
}

fn default_banner() -> Vec<String> {
    vec!["CARRY".to_string(), "CODE".to_string()]
}

/// Runtime session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSessionConfig {
    pub session_id: String,
    pub agent_mode: String, // "plan" | "build"
    pub approval_mode: String, // "read-only" | "agent" | "agent-full"
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    pub theme: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub sessions: Vec<RuntimeSessionConfig>,
}

/// Global application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Runtime configuration (Internal use)
    #[serde(skip)]
    pub runtime: RuntimeConfig,

    /// UI Theme
    #[serde(default)]
    pub theme: Option<String>,

    /// Welcome configuration
    #[serde(default)]
    pub welcome: Option<WelcomeConfig>,

    /// LLM Provider configuration (Deprecated, use providers)
    #[serde(rename = "llm_provider")]
    pub llm_provider: Option<LlmProviderConfig>,

    /// List of providers
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,

    /// Default model to use (provider:model or just model)
    #[serde(default)]
    pub default_model: Option<String>,

    /// Prompt plan configuration
    #[serde(default)]
    pub prompt_plan: Option<PromptPlanConfig>,

    /// Prompt build configuration
    #[serde(default)]
    pub prompt_build: Option<PromptPlanConfig>,

    /// LS tool configuration
    #[serde(rename = "tool_ls")]
    pub tool_ls: ToolLsConfig,

    /// Grep tool configuration
    #[serde(rename = "tool_grep")]
    pub tool_grep: ToolGrepConfig,

    /// Fetch tool configuration
    #[serde(rename = "tool_fetch")]
    pub tool_fetch: ToolFetchConfig,

    /// Glob tool configuration
    #[serde(rename = "tool_glob", alias = "tool_grob")]
    pub tool_glob: ToolGlobConfig,

    /// Bash tool configuration
    #[serde(rename = "tool_bash")]
    pub tool_bash: ToolBashConfig,

    /// View tool configuration
    #[serde(rename = "tool_view")]
    pub tool_view: ToolViewConfig,

    /// Write tool configuration
    #[serde(rename = "tool_write")]
    pub tool_write: ToolWriteConfig,

    /// Edit tool configuration
    #[serde(rename = "tool_edit")]
    pub tool_edit: ToolEditConfig,

    /// TodoWrite tool configuration
    #[serde(rename = "tool_todo_write")]
    pub tool_todo_write: ToolTodoWriteConfig,

    /// LSP configuration
    #[serde(default)]
    pub lsp: LspConfig,

    /// MCP servers configuration
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl AppConfig {
    /// Load configuration with layered strategy:
    /// 1. Defaults (Embedded Config.toml)
    /// 2. User Config (~/.carry/carrycode.json) - Only theme/providers
    /// 3. Project Config (./.carry/carrycode.json) - Only theme/providers
    /// 4. Runtime Config (~/.carry/carrycode-runtime.json) - Runtime state
    pub fn load() -> Result<Self> {
        // 1. Load Base Config (Embedded)
        let default_str = include_str!("../Config.toml");
        let mut config: AppConfig = toml::from_str(default_str)
            .context("Failed to parse embedded Config.toml")?;

        // 2. Apply User Config Patch
        if let Some(home) = dirs::home_dir() {
            let user_path = home.join(".carry").join("carrycode.json");
            Self::apply_patch(&mut config, user_path);
        }

        // 3. Apply Project Config Patch
        let project_path = Path::new(".carry").join("carrycode.json");
        Self::apply_patch(&mut config, project_path);

        if config.theme.is_none() {
            config.theme = config.welcome.as_ref().and_then(|w| w.theme.clone());
        }

        // 4. Load Runtime Config
        let mut runtime_needs_save = false;
        let mut runtime_file_exists = false;
        if let Some(home) = dirs::home_dir() {
            let runtime_path = home.join(".carry").join("carrycode-runtime.json");
            if runtime_path.exists() {
                runtime_file_exists = true;
                 if let Ok(content) = fs::read_to_string(&runtime_path) {
                    if let Ok(runtime_config) = serde_json::from_str::<RuntimeConfig>(&content) {
                        if let Some(theme) = &runtime_config.theme {
                            config.theme = Some(theme.clone());
                        }
                        config.runtime = runtime_config;
                    }
                }
            } else {
                // If runtime config does not exist, initialize runtime.theme from current config.theme
                // This handles the case where Config.toml has a default, but runtime.json is missing or new.
                if let Some(theme) = &config.theme {
                    config.runtime.theme = Some(theme.clone());
                    runtime_needs_save = true;
                }
            }
        }
        
        // Ensure runtime theme is consistent if it was null in file but we have a theme from other sources
        if config.runtime.theme.is_none() && config.theme.is_some() {
             config.runtime.theme = config.theme.clone();
             runtime_needs_save = true;
        }

        let (resolved_default_model, should_save_default_model) = resolve_default_model(
            runtime_file_exists,
            config.runtime.default_model.clone(),
            &config.providers,
        );
        config.default_model = resolved_default_model.clone();
        config.runtime.default_model = resolved_default_model;
        if should_save_default_model {
            runtime_needs_save = true;
        }

        if runtime_needs_save {
            let _ = config.save_runtime();
        }

        Ok(config)
    }

    pub fn save_runtime(&self) -> Result<()> {
        if let Some(home) = dirs::home_dir() {
            let config_dir = home.join(".carry");
            if !config_dir.exists() {
                fs::create_dir_all(&config_dir)?;
            }
            let runtime_path = config_dir.join("carrycode-runtime.json");
            let content = serde_json::to_string_pretty(&self.runtime)?;
            fs::write(runtime_path, content)?;
        }
        Ok(())
    }

    fn apply_patch<P: AsRef<Path>>(config: &mut AppConfig, path: P) {
        let path = path.as_ref();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                // Try parsing as UserOverrideConfig to restrict fields
                match serde_json::from_str::<UserOverrideConfig>(&content) {
                    Ok(patch) => {
                        if let Some(providers) = patch.providers {
                            // Override providers
                            config.providers = providers.into_iter().map(|p| p.into()).collect();
                        }
                        if let Some(mcp_servers) = patch.mcp_servers {
                            // Merge MCP servers
                            for (name, server) in mcp_servers {
                                config.mcp_servers.insert(name, server);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse config patch at {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
}

fn resolve_default_model(
    runtime_file_exists: bool,
    runtime_default_model: Option<String>,
    providers: &[ProviderConfig],
) -> (Option<String>, bool) {
    let runtime_default_model = runtime_default_model.and_then(|v| {
        let v = v.trim().to_string();
        if v.is_empty() { None } else { Some(v) }
    });

    if runtime_file_exists && runtime_default_model.is_some() {
        return (runtime_default_model, false);
    }

    let Some(p) = providers.first() else {
        return (runtime_default_model, false);
    };
    let Some(m) = p.models.first() else {
        return (runtime_default_model, false);
    };

    (Some(format!("{}:{}", p.name, m)), true)
}

#[cfg(test)]
mod tests {
    use super::{resolve_default_model, ProviderConfig, RuntimeConfig};

    #[test]
    fn runtime_config_deserializes_without_default_model() {
        let json = r#"{"theme":"carrycode-dark","sessions":[]}"#;
        let cfg: RuntimeConfig = serde_json::from_str(json).expect("should parse old runtime schema");
        assert!(cfg.default_model.is_none());
    }

    #[test]
    fn runtime_config_deserializes_with_default_model() {
        let json = r#"{"theme":"carrycode-dark","default_model":"openai:gpt-4o-mini","sessions":[]}"#;
        let cfg: RuntimeConfig = serde_json::from_str(json).expect("should parse new runtime schema");
        assert_eq!(cfg.default_model.as_deref(), Some("openai:gpt-4o-mini"));
    }

    #[test]
    fn resolve_default_model_falls_back_when_runtime_missing() {
        let providers = vec![ProviderConfig {
            name: "openai".to_string(),
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];
        let (v, should_save) = resolve_default_model(false, None, &providers);
        assert_eq!(v.as_deref(), Some("openai:gpt-4o-mini"));
        assert!(should_save);
    }

    #[test]
    fn resolve_default_model_falls_back_when_runtime_default_model_empty() {
        let providers = vec![ProviderConfig {
            name: "openai".to_string(),
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];
        let (v, should_save) =
            resolve_default_model(true, Some("   ".to_string()), &providers);
        assert_eq!(v.as_deref(), Some("openai:gpt-4o-mini"));
        assert!(should_save);
    }

    #[test]
    fn resolve_default_model_uses_runtime_value_when_present() {
        let providers = vec![ProviderConfig {
            name: "openai".to_string(),
            base_url: "http://localhost:1234".to_string(),
            api_key: "k".to_string(),
            models: vec!["gpt-4o-mini".to_string()],
        }];
        let (v, should_save) =
            resolve_default_model(true, Some("openai:gpt-4o-mini".to_string()), &providers);
        assert_eq!(v.as_deref(), Some("openai:gpt-4o-mini"));
        assert!(!should_save);
    }
}
