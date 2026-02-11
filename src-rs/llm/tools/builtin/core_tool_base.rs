use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool side-effect / capability classification (aligned with Gemini kinds)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolKind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    Todo,
    Other,
}

/// High level operation category (for display/policy)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolOperation {
    Bash,
    Explored,
    Edited,
    Todo,
    Other,
}

pub const TOOL_RESULT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub version: u32,
    pub tool_name: String,
    pub kind: ToolKind,
    pub operation: ToolOperation,
    pub key_path: String,
    pub success: bool,
    pub requires_confirmation: bool,
    pub executed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_summary: Option<String>,
    pub stdout: String,
    pub stderr: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolOperationEnd {
    BashEnd,
    ExploredEnd,
    EditedEnd,
    TodoEnd,
    OtherEnd,
}

impl From<ToolOperation> for ToolOperationEnd {
    fn from(op: ToolOperation) -> Self {
        match op {
            ToolOperation::Bash => ToolOperationEnd::BashEnd,
            ToolOperation::Explored => ToolOperationEnd::ExploredEnd,
            ToolOperation::Edited => ToolOperationEnd::EditedEnd,
            ToolOperation::Todo => ToolOperationEnd::TodoEnd,
            ToolOperation::Other => ToolOperationEnd::OtherEnd,
        }
    }
}

impl ToolOperationEnd {
    pub fn marker(self) -> &'static str {
        match self {
            ToolOperationEnd::BashEnd => "__BASH_END__",
            ToolOperationEnd::ExploredEnd => "__EXPLORED_END__",
            ToolOperationEnd::EditedEnd => "__EDITED_END__",
            ToolOperationEnd::TodoEnd => "__TODO_END__",
            ToolOperationEnd::OtherEnd => "__OTHER_END__",
        }
    }
}

impl ToolResult {
    pub fn ok(
        tool_name: impl Into<String>,
        kind: ToolKind,
        operation: ToolOperation,
        stdout: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            version: TOOL_RESULT_VERSION,
            tool_name: tool_name.into(),
            kind,
            operation,
            key_path: String::new(),
            success: true,
            requires_confirmation: false,
            executed: true,
            response_summary: None,
            stdout: stdout.into(),
            stderr: String::new(),
            data,
        }
    }

    pub fn err(
        tool_name: impl Into<String>,
        kind: ToolKind,
        operation: ToolOperation,
        stderr: impl Into<String>,
        data: Value,
    ) -> Self {
        let err_msg = stderr.into();
        Self {
            version: TOOL_RESULT_VERSION,
            tool_name: tool_name.into(),
            kind,
            operation,
            key_path: String::new(),
            success: false,
            requires_confirmation: false,
            executed: true,
            response_summary: Some(err_msg.clone()),
            stdout: String::new(),
            stderr: err_msg,
            data,
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.response_summary = Some(summary.into());
        self
    }
}

fn escape_control_chars_in_json_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    for c in input.chars() {
        if in_string {
            if escape {
                out.push(c);
                escape = false;
                continue;
            }
            if c == '\\' {
                out.push(c);
                escape = true;
                continue;
            }
            if c == '"' {
                out.push(c);
                in_string = false;
                continue;
            }
            match c {
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => {
                    if c.is_control() {
                        use std::fmt::Write;
                        let _ = write!(out, "\\u{:04x}", c as u32);
                    } else {
                        out.push(c);
                    }
                }
            }
            continue;
        }

        out.push(c);
        if c == '"' {
            in_string = true;
        }
    }
    out
}

pub fn parse_confirmed_and_args<T: DeserializeOwned>(arguments: &str) -> Result<(T, bool)> {
    let mut v: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(first_err) => {
            let repaired = escape_control_chars_in_json_strings(arguments);
            match serde_json::from_str(&repaired) {
                Ok(v) => v,
                Err(second_err) => {
                    return Err(anyhow::anyhow!(
                        "Failed to parse tool arguments. first_error={}; after_repair_error={}",
                        first_err,
                        second_err
                    ));
                }
            }
        }
    };
    let confirmed = v
        .get("confirmed")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    if let Value::Object(map) = &mut v {
        map.remove("confirmed");
    }
    let args: T = serde_json::from_value(v).context("Failed to deserialize tool arguments")?;
    Ok((args, confirmed))
}

/// Standard output structure for all tools
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// The original command/request
    pub command: String,
    /// Standard output or main result
    pub stdout: String,
    /// Standard error or error details
    pub stderr: String,
    /// Whether the tool requires user confirmation before execution
    pub requires_confirmation: bool,
    /// Whether the tool was actually executed
    pub executed: bool,
    /// Optional concise summary of the response (e.g., "10 lines", "5 files", etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_summary: Option<String>,
}

impl ToolOutput {
    /// Create a new successful output
    #[allow(dead_code)]
    pub fn success(command: impl Into<String>, stdout: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            stdout: stdout.into(),
            stderr: String::new(),
            requires_confirmation: false,
            executed: true,
            response_summary: None,
        }
    }

    /// Create a new error output
    pub fn error(command: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            stdout: String::new(),
            stderr: stderr.into(),
            requires_confirmation: false,
            executed: true, // Attempted execution
            response_summary: None,
        }
    }
}

/// Trait that all agent tools must implement
pub trait Tool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get tool kind (for display, policy, and prompt hints)
    fn kind(&self) -> ToolKind;

    /// Get high level operation category
    fn operation(&self) -> ToolOperation;

    fn operation_end(&self) -> ToolOperationEnd {
        ToolOperationEnd::from(self.operation())
    }

    /// Get the tool definition in OpenAI function calling format
    fn to_tool_definition(&self) -> Value;

    /// Execute the tool with given arguments (JSON string)
    ///
    /// # Arguments
    /// * `arguments` - JSON string containing the tool arguments.
    ///                 If the tool supports confirmation, the JSON may contain
    ///                 a `confirmed` boolean field.
    ///
    /// # Returns
    /// * Result with the execution result as a JSON string (schema: ToolOutput)
    fn execute(&self, arguments: &str) -> Result<String>;

    /// Create a clone of the tool (boxed)
    fn clone_box(&self) -> Box<dyn Tool>;
}

impl Clone for Box<dyn Tool> {
    fn clone(&self) -> Box<dyn Tool> {
        self.clone_box()
    }
}

pub trait ToolSpec: Send + Sync + Clone + 'static {
    type Args: DeserializeOwned;

    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn kind(&self) -> ToolKind;
    fn operation(&self) -> ToolOperation;
    fn to_tool_definition(&self) -> Value;
    fn run(&self, args: Self::Args, confirmed: bool) -> Result<ToolResult>;
}

#[derive(Debug, Clone)]
pub struct ToolAdapter<T: ToolSpec>(pub T);

impl<T: ToolSpec> Tool for ToolAdapter<T> {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn description(&self) -> &str {
        self.0.description()
    }

    fn kind(&self) -> ToolKind {
        self.0.kind()
    }

    fn operation(&self) -> ToolOperation {
        self.0.operation()
    }

    fn to_tool_definition(&self) -> Value {
        self.0.to_tool_definition()
    }

    fn execute(&self, arguments: &str) -> Result<String> {
        let (args, confirmed) = match parse_confirmed_and_args::<T::Args>(arguments) {
            Ok(x) => x,
            Err(e) => {
                let tr = ToolResult::err(
                    self.name(),
                    self.kind(),
                    self.operation(),
                    e.to_string(),
                    serde_json::json!({ "arguments": arguments }),
                );
                return serde_json::to_string(&tr).context("Failed to serialize ToolResult");
            }
        };

        let mut tr = match self.0.run(args, confirmed) {
            Ok(x) => x,
            Err(e) => ToolResult::err(
                self.name(),
                self.kind(),
                self.operation(),
                e.to_string(),
                serde_json::json!({}),
            ),
        };

        tr.version = TOOL_RESULT_VERSION;
        tr.tool_name = self.name().to_string();
        tr.kind = self.kind();
        tr.operation = self.operation();
        if tr.response_summary.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            tr.response_summary = Some("no output".to_string());
        }

        serde_json::to_string(&tr).context("Failed to serialize ToolResult")
    }

    fn clone_box(&self) -> Box<dyn Tool> {
        Box::new(self.clone())
    }
}

