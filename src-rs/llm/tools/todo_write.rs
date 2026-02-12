use crate::llm::config::AppConfig;
use crate::llm::tools::tool_trait::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Todo item status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]  
    Completed,
}

/// Todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    #[serde(rename = "activeForm")]
    pub active_form: String,
}

/// Todo write tool for managing task lists
#[derive(Clone)]
pub struct TodoWriteTool {
    pub tool_name: String,
    pub description: String,
}

use crate::llm::utils::serde_util::deserialize_vec_or_str_lax;

/// Todo write request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoWriteRequest {
    #[serde(deserialize_with = "deserialize_vec_or_str_lax")]
    pub todos: Vec<TodoItem>,
}

/// Result of todo write operation
impl TodoWriteTool {
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.tool_todo_write.tool_name,
                description: config.tool_todo_write.description,
            },
            Err(e) => {
                log::warn!("Failed to load config.toml: {}, using defaults", e);
                Self::default()
            }
        }
    }

    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.tool_todo_write.tool_name.clone(),
            description: config.tool_todo_write.description.clone(),
        }
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self {
            tool_name: "todo_write".to_string(),
            description: "Manage task lists and track progress".to_string(),
        }
    }
}

impl ToolSpec for TodoWriteTool {
    type Args = TodoWriteRequest;

    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Todo
    }

    fn operation(&self) -> ToolOperation {
        ToolOperation::Todo
    }

    fn to_tool_definition(&self) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": self.tool_name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "description": "List of todo items",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": {
                                        "type": "string",
                                        "description": "Todo item description (imperative form)"
                                    },
                                    "status": {
                                        "type": "string",
                                        "enum": ["pending", "in_progress", "completed"],
                                        "description": "Status of the todo item"
                                    },
                                    "activeForm": {
                                        "type": "string",
                                        "description": "Present continuous form of the todo (e.g., 'Running tests')"
                                    }
                                },
                                "required": ["content", "status", "activeForm"]
                            }
                        }
                    },
                    "required": ["todos"]
                }
            }
        })
    }

    fn run(&self, args: Self::Args, _confirmed: bool) -> Result<ToolResult> {
        let count = args.todos.len();
        Ok(ToolResult::ok(
            self.tool_name.clone(),
            self.kind(),
            self.operation(),
            "",
            json!({ "todos": args.todos }),
        )
        .with_summary(format!("{} items", count)))
    }
}
