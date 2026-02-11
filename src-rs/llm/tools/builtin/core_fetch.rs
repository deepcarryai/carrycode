use crate::llm::config::AppConfig;
use crate::llm::tools::builtin::core_tool_base::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Fetch tool for retrieving content from URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreFetchTool {
    pub tool_name: String,
    pub description: String,
}

use crate::llm::utils::serde_util::deserialize_u64_lax;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreFetchRequest {
    pub url: String,
    pub format: String,
    #[serde(default = "default_timeout", deserialize_with = "deserialize_u64_lax")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    30000 // 30 seconds in milliseconds
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchMetadata {
    pub url: String,
    pub format: String,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchResult {
    pub content: String,
    pub metadata: FetchMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub response_summary: String,
}

impl CoreFetchTool {
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.core_fetch.tool_name,
                description: config.core_fetch.description,
            },
            Err(_) => Self::default(),
        }
    }

    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.core_fetch.tool_name.clone(),
            description: config.core_fetch.description.clone(),
        }
    }

    pub fn fetch_content(&self, request: &CoreFetchRequest) -> Result<FetchResult> {
        // Validate URL
        let parsed_url = url::Url::parse(&request.url).context("Invalid URL format")?;

        // Only allow HTTP and HTTPS
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Ok(FetchResult {
                content: String::new(),
                metadata: FetchMetadata {
                    url: request.url.clone(),
                    format: request.format.clone(),
                    size: 0,
                },
                status_code: None,
                error: Some("Only HTTP and HTTPS protocols are supported".to_string()),
                response_summary: "Error: unsupported protocol".to_string(),
            });
        }

        // Clone data needed for the thread
        let url = request.url.clone();
        let format = request.format.clone();
        let timeout = request.timeout;

        // Use std::thread to avoid tokio runtime issues
        let handle = std::thread::spawn(move || -> Result<FetchResult> {
            // Build HTTP client with timeout
            let timeout_duration = std::time::Duration::from_millis(timeout.min(120000));
            let client = reqwest::blocking::Client::builder()
                .timeout(timeout_duration)
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .context("Failed to create HTTP client")?;

            // Make the request
            let response = client.get(&url).send().context("Failed to send request")?;

            let status = response.status();
            let status_code = status.as_u16();

            if !status.is_success() {
                return Ok(FetchResult {
                    content: String::new(),
                    metadata: FetchMetadata {
                        url: url.clone(),
                        format: format.clone(),
                        size: 0,
                    },
                    status_code: Some(status_code),
                    error: Some(format!("HTTP error: {}", status)),
                    response_summary: format!("Error: HTTP {}", status_code),
                });
            }

            // Get content type before consuming response
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "text/plain".to_string());

            // Read response body
            let body = response.text().context("Failed to read response body")?;
            let body_size = body.len();

            // Check size limit (5MB)
            if body_size > 5 * 1024 * 1024 {
                return Ok(FetchResult {
                    content: String::new(),
                    metadata: FetchMetadata {
                        url: url.clone(),
                        format: format.clone(),
                        size: body_size,
                    },
                    status_code: Some(status_code),
                    error: Some("Response too large (max 5MB)".to_string()),
                    response_summary: "Error: response too large".to_string(),
                });
            }

            // Convert content based on requested format
            let content = match format.as_str() {
                "html" => body,
                "markdown" | "md" => {
                    if content_type.contains("html") {
                        html2md::parse_html(&body)
                    } else {
                        body
                    }
                }
                "text" => {
                    if content_type.contains("html") {
                        html2text::from_read(body.as_bytes(), 100)
                    } else {
                        body
                    }
                }
                _ => body,
            };

            // Calculate line count for summary
            let line_count = content.lines().count();

            Ok(FetchResult {
                content,
                metadata: FetchMetadata {
                    url: url.clone(),
                    format: format.clone(),
                    size: body_size,
                },
                status_code: Some(status_code),
                error: None,
                response_summary: format!("{} lines", line_count),
            })
        });

        // Wait for the thread to complete
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("Fetch thread panicked"))?
    }

    fn to_tool_definition_json(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.tool_name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to fetch content from (HTTP or HTTPS only)"
                        },
                        "format": {
                            "type": "string",
                            "enum": ["text", "markdown", "html"],
                            "description": "Output format: 'text' for plain text, 'markdown' for markdown, 'html' for raw HTML"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Request timeout in milliseconds (max 120000ms). Default: 30000ms",
                            "maximum": 120000
                        }
                    },
                    "required": ["url", "format"]
                }
            }
        })
    }
}

impl Default for CoreFetchTool {
    fn default() -> Self {
        Self {
            tool_name: "core_fetch".to_string(),
            description: "[CORE SYSTEM] Fetches content from a URL and returns it in the specified format.".to_string(),
        }
    }
}

impl ToolSpec for CoreFetchTool {
    type Args = CoreFetchRequest;

    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Fetch
    }

    fn operation(&self) -> ToolOperation {
        ToolOperation::Explored
    }

    fn to_tool_definition(&self) -> serde_json::Value {
        self.to_tool_definition_json()
    }

    fn run(&self, args: Self::Args, _confirmed: bool) -> Result<ToolResult> {
        let result = self.fetch_content(&args)?;
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
