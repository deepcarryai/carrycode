use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::collections::HashMap;

use crate::lsp::config::LspConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChunkingConfig {
    #[serde(default = "default_chunk_limit_chars")]
    pub default_chunk_limit_chars: usize,
    #[serde(default)]
    pub model_max_tokens: HashMap<String, u32>,
}

fn default_chunk_limit_chars() -> usize {
    20480
}

impl Default for ToolChunkingConfig {
    fn default() -> Self {
        Self {
            default_chunk_limit_chars: default_chunk_limit_chars(),
            model_max_tokens: HashMap::new(),
        }
    }
}

impl ToolChunkingConfig {
    pub fn limit_chars_for_model(&self, model_key: Option<&str>) -> usize {
        let Some(key) = model_key else {
            return self.default_chunk_limit_chars;
        };

        if let Some(v) = self.model_max_tokens.get(key) {
            return (((*v as f64) * 2.0).floor() as usize).max(1);
        }

        let model_only = key.split_once(':').map(|(_, m)| m).unwrap_or(key);
        if let Some(v) = self.model_max_tokens.get(model_only) {
            return (((*v as f64) * 2.0).floor() as usize).max(1);
        }

        self.default_chunk_limit_chars
    }
}

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
    #[serde(default)]
    pub provider_brand: Option<String>,
    #[serde(rename = "provider_id", alias = "provider_name")]
    pub provider_id: String,
    pub model_name: String,
    pub base_url: String,
    pub api_key: String,
}

impl From<UserProviderConfig> for ProviderConfig {
    fn from(c: UserProviderConfig) -> Self {
        let brand = c.provider_brand.unwrap_or_else(|| c.provider_id.clone());
        ProviderConfig {
            name: c.provider_id,
            brand: Some(brand),
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

/// Explicit MCP Server configuration (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpServerConfigExplicit {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    #[serde(rename = "streamableHttp")]
    StreamableHttp {
        mcp_url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    #[serde(rename = "sseLegacy")]
    SseLegacy {
        mcp_url: String,
        #[serde(default)]
        post_url: Option<String>,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    #[serde(rename = "sse")]
    Sse {
        mcp_url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

impl McpServerConfigExplicit {
    pub fn to_public(&self) -> PublicMcpServerConfig {
        match self {
            Self::Stdio { command, args, .. } => PublicMcpServerConfig::Stdio {
                command: command.clone(),
                args: args.clone(),
            },
            Self::StreamableHttp { mcp_url, .. } => PublicMcpServerConfig::StreamableHttp {
                mcp_url: mcp_url.clone(),
            },
            Self::SseLegacy { mcp_url, post_url, .. } => PublicMcpServerConfig::SseLegacy {
                mcp_url: mcp_url.clone(),
                post_url: post_url.clone(),
            },
            Self::Sse { mcp_url, .. } => PublicMcpServerConfig::Sse {
                mcp_url: mcp_url.clone(),
            },
        }
    }

    pub fn create_client(&self) -> anyhow::Result<crate::llm::tools::mcp::mcp_tool_client::McpClient> {
        use crate::llm::tools::mcp::mcp_tool_client::McpClient;
        match self {
            Self::Stdio { command, args, env } => {
                McpClient::new_stdio(command.clone(), args.clone(), env.clone())
            }
            Self::StreamableHttp { mcp_url, headers } => {
                McpClient::new_streamable_http(mcp_url.clone(), headers.clone())
            }
            Self::SseLegacy { mcp_url, post_url, headers } => {
                McpClient::new_legacy_sse(mcp_url.clone(), post_url.clone(), headers.clone())
            }
            Self::Sse { mcp_url, headers } => {
                McpClient::new_legacy_sse(mcp_url.clone(), None, headers.clone())
            }
        }
    }
}

/// MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    Explicit(McpServerConfigExplicit),
    // Standard/Untagged support
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    Sse {
        mcp_url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

impl McpServerConfig {
    pub fn to_public(&self) -> PublicMcpServerConfig {
        match self {
            Self::Explicit(e) => e.to_public(),
            Self::Stdio { command, args, .. } => PublicMcpServerConfig::Stdio {
                command: command.clone(),
                args: args.clone(),
            },
            Self::Sse { mcp_url, .. } => PublicMcpServerConfig::Sse {
                mcp_url: mcp_url.clone(),
            },
        }
    }

    pub fn create_client(&self) -> anyhow::Result<crate::llm::tools::mcp::mcp_tool_client::McpClient> {
        use crate::llm::tools::mcp::mcp_tool_client::McpClient;
        match self {
            Self::Explicit(e) => e.create_client(),
            Self::Stdio { command, args, env } => {
                McpClient::new_stdio(command.clone(), args.clone(), env.clone())
            }
            Self::Sse { mcp_url, headers } => {
                McpClient::new_legacy_sse(mcp_url.clone(), None, headers.clone())
            }
        }
    }
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider name (e.g., "zhipuai", "openai", "vllm")
    pub name: String,

    #[serde(default)]
    pub brand: Option<String>,

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
    /// Provider id (e.g., "zhipuai", "openai", "vllm")
    #[serde(default = "default_provider_id", rename = "provider_id", alias = "provider_name")]
    pub provider_id: String,

    /// Base URL for the LLM API
    pub base_url: String,

    /// API key for authentication
    pub api_key: String,

    /// Model name to use
    pub model_name: String,
}

fn default_provider_id() -> String {
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
    "core_ls".to_string()
}

fn default_max_ls_files() -> usize {
    100
}

fn default_description() -> String {
    "[CORE SYSTEM] List directory contents. Shows files and directories in specified path.".to_string()
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
    "core_grep".to_string()
}

fn default_max_results() -> usize {
    100
}

fn default_grep_desc() -> String {
    "[CORE SYSTEM] Search file contents for text patterns. Returns matching file paths and contexts.".to_string()
}

/// Tool Diagnostics configuration from config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDiagnosticsConfig {
    /// Tool name identifier
    #[serde(default = "default_diagnostics_name")]
    pub tool_name: String,

    /// Description of what this tool does
    #[serde(default = "default_diagnostics_desc")]
    pub description: String,
}

fn default_diagnostics_name() -> String {
    "core_diagnostics".to_string()
}

fn default_diagnostics_desc() -> String {
    "[CORE SYSTEM] Get diagnostics for a file and/or project.".to_string()
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
    "core_fetch".to_string()
}

fn default_fetch_desc() -> String {
    "[CORE SYSTEM] Fetches content from a URL and returns it in the specified format.".to_string()
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
    "core_glob".to_string()
}

fn default_max_glob_results() -> usize {
    100
}

fn default_glob_desc() -> String {
    "[CORE SYSTEM] Fast file pattern matching tool that finds files by name and pattern.".to_string()
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
    "core_bash".to_string()
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
    "[CORE SYSTEM] Executes a given bash command in a persistent shell session.".to_string()
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
    "core_view".to_string()
}

fn default_max_file_size() -> usize {
    256000
}

fn default_view_desc() -> String {
    "[CORE SYSTEM] File viewing tool that reads and displays the contents of files with line numbers.".to_string()
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
    "core_write".to_string()
}

fn default_write_desc() -> String {
    "[CORE SYSTEM] File writing tool that creates or updates files in the filesystem.".to_string()
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
    "core_edit".to_string()
}

fn default_edit_desc() -> String {
    "[CORE SYSTEM] Edits files by replacing text, creating new files, or deleting content.".to_string()
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
    "core_todo_write".to_string()
}

fn default_todo_write_desc() -> String {
    "[CORE SYSTEM] Manage task lists and track progress".to_string()
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderPreset {
    #[serde(rename = "provider_id", alias = "provider_name")]
    pub provider_id: String,
    #[serde(rename = "provider_brand")]
    pub provider_brand: String,
    #[serde(rename = "base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model_name: String,
    #[serde(default)]
    pub provider_desc: String,
}

fn default_banner() -> Vec<String> {
    vec!["CARRY".to_string(), "CODE".to_string()]
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_welcome_wizard_done: Option<bool>,
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

    #[serde(default)]
    pub provider_presets: Vec<ProviderPreset>,

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
    #[serde(rename = "core_ls", alias = "tool_ls")]
    pub core_ls: ToolLsConfig,

    /// Diagnostics tool configuration
    #[serde(rename = "core_diagnostics", alias = "tool_diagnostics")]
    pub core_diagnostics: ToolDiagnosticsConfig,

    /// Grep tool configuration
    #[serde(rename = "core_grep", alias = "tool_grep")]
    pub core_grep: ToolGrepConfig,

    /// Fetch tool configuration
    #[serde(rename = "core_fetch", alias = "tool_fetch")]
    pub core_fetch: ToolFetchConfig,

    /// Glob tool configuration
    #[serde(rename = "core_glob", alias = "tool_glob")]
    pub core_glob: ToolGlobConfig,

    /// Bash tool configuration
    #[serde(rename = "core_bash", alias = "tool_bash")]
    pub core_bash: ToolBashConfig,

    /// View tool configuration
    #[serde(rename = "core_view", alias = "tool_view")]
    pub core_view: ToolViewConfig,

    /// Write tool configuration
    #[serde(rename = "core_write", alias = "tool_write")]
    pub core_write: ToolWriteConfig,

    /// Edit tool configuration
    #[serde(rename = "core_edit", alias = "tool_edit")]
    pub core_edit: ToolEditConfig,

    /// TodoWrite tool configuration
    #[serde(rename = "core_todo_write", alias = "tool_todo_write")]
    pub core_todo_write: ToolTodoWriteConfig,

    /// LSP configuration
    #[serde(default)]
    pub lsp: LspConfig,

    /// MCP servers configuration
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    #[serde(default)]
    pub tool_chunking: ToolChunkingConfig,
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
                    match serde_json::from_str::<RuntimeConfig>(&content) {
                        Ok(runtime_config) => {
                            if let Some(theme) = &runtime_config.theme {
                                config.theme = Some(theme.clone());
                            }
                            config.runtime = runtime_config;
                        }
                        Err(e) => {
                            log::warn!("Failed to parse runtime config at {}: {}", runtime_path.display(), e);
                        }
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

    pub(crate) fn apply_patch<P: AsRef<Path>>(config: &mut AppConfig, path: P) {
        let path = path.as_ref();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                // Try parsing as UserOverrideConfig to restrict fields
                match serde_json::from_str::<UserOverrideConfig>(&content) {
                    Ok(patch) => {
                        if let Some(providers) = patch.providers {
                            // Override providers
                            let mut ordered_keys: Vec<String> = Vec::new();
                            let mut by_key: HashMap<String, ProviderConfig> = HashMap::new();

                            for p in providers {
                                let c: ProviderConfig = p.into();
                                let model = c.models.first().cloned().unwrap_or_default();
                                if c.name.trim().is_empty() || model.trim().is_empty() {
                                    continue;
                                }
                                let key = format!("{}:{}", c.name, model);
                                if !by_key.contains_key(&key) {
                                    ordered_keys.push(key.clone());
                                }
                                by_key.insert(key, c);
                            }

                            let mut merged: Vec<ProviderConfig> = Vec::new();
                            for k in ordered_keys {
                                if let Some(v) = by_key.remove(&k) {
                                    merged.push(v);
                                }
                            }
                            merged.extend(by_key.into_values());
                            config.providers = merged;
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

    pub fn to_public(&self) -> PublicAppConfig {
        PublicAppConfig {
            runtime: self.runtime.clone(),
            theme: self.theme.clone(),
            welcome: self.welcome.clone(),
            provider_presets: self.provider_presets.clone(),
            providers: self.providers.iter().map(|p| PublicProviderConfig {
                name: p.name.clone(),
                brand: p.brand.clone(),
                base_url: p.base_url.clone(),
                models: p.models.clone(),
            }).collect(),
            default_model: self.default_model.clone(),
            prompt_plan: self.prompt_plan.clone(),
            prompt_build: self.prompt_build.clone(),
            core_ls: self.core_ls.clone(),
            core_diagnostics: self.core_diagnostics.clone(),
            core_grep: self.core_grep.clone(),
            core_fetch: self.core_fetch.clone(),
            core_glob: self.core_glob.clone(),
            core_bash: self.core_bash.clone(),
            core_view: self.core_view.clone(),
            core_write: self.core_write.clone(),
            core_edit: self.core_edit.clone(),
            core_todo_write: self.core_todo_write.clone(),
            lsp: self.lsp.clone(),
            mcp_servers: self.mcp_servers.iter().map(|(k, v)| (k.clone(), v.to_public())).collect(),
            tool_chunking: self.tool_chunking.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicAppConfig {
    pub runtime: RuntimeConfig,
    pub theme: Option<String>,
    pub welcome: Option<WelcomeConfig>,
    pub provider_presets: Vec<ProviderPreset>,
    pub providers: Vec<PublicProviderConfig>,
    pub default_model: Option<String>,
    pub prompt_plan: Option<PromptPlanConfig>,
    pub prompt_build: Option<PromptPlanConfig>,
    pub core_ls: ToolLsConfig,
    pub core_diagnostics: ToolDiagnosticsConfig,
    pub core_grep: ToolGrepConfig,
    pub core_fetch: ToolFetchConfig,
    pub core_glob: ToolGlobConfig,
    pub core_bash: ToolBashConfig,
    pub core_view: ToolViewConfig,
    pub core_write: ToolWriteConfig,
    pub core_edit: ToolEditConfig,
    pub core_todo_write: ToolTodoWriteConfig,
    pub lsp: LspConfig,
    pub mcp_servers: HashMap<String, PublicMcpServerConfig>,
    pub tool_chunking: ToolChunkingConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicProviderConfig {
    pub name: String,
    pub brand: Option<String>,
    pub base_url: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PublicMcpServerConfig {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        args: Vec<String>,
    },
    #[serde(rename = "streamableHttp")]
    StreamableHttp {
        mcp_url: String,
    },
    #[serde(rename = "sseLegacy")]
    SseLegacy {
        mcp_url: String,
        post_url: Option<String>,
    },
    #[serde(rename = "sse")]
    Sse {
        mcp_url: String,
    },
}


pub(crate) fn resolve_default_model(
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

