use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub file_extensions: Vec<String>,
    pub root_markers: Vec<String>,
}

fn default_timeout() -> u64 {
    180000
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_ms: 180000,
            servers: vec![
                ServerConfig {
                    name: "rust-analyzer".to_string(),
                    command: "rust-analyzer".to_string(),
                    args: vec![],
                    file_extensions: vec!["rs".to_string()],
                    root_markers: vec!["Cargo.toml".to_string()],
                },
                ServerConfig {
                    name: "pyright".to_string(),
                    command: "pyright-langserver".to_string(),
                    args: vec!["--stdio".to_string()],
                    file_extensions: vec!["py".to_string()],
                    root_markers: vec![
                        "pyproject.toml".to_string(),
                        "setup.py".to_string(),
                        "requirements.txt".to_string(),
                    ],
                },
            ],
        }
    }
}
