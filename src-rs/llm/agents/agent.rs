use crate::llm::models::provider_handle::{
    AnyProviderClient, Message, ProviderClient, ProviderClientFactory,
};
use crate::llm::tools::tool_trait::{
    Tool, ToolKind, ToolOperation, ToolOutput, ToolResult, TOOL_RESULT_VERSION,
};
use crate::session::key_path_from_args;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_stream::StreamExt;

fn tool_result_from_execution(
    tool_name: &str,
    args: &str,
    kind: ToolKind,
    operation: ToolOperation,
    execution_result: &Result<String>,
) -> ToolResult {
    let key_path = key_path_from_args(tool_name, args);

    match execution_result {
        Ok(result) => {
            if let Ok(mut parsed) = serde_json::from_str::<ToolResult>(result) {
                if parsed.version == TOOL_RESULT_VERSION {
                    parsed.tool_name = tool_name.to_string();
                    parsed.kind = kind;
                    parsed.operation = operation;
                    parsed.key_path = key_path;
                    if parsed.response_summary.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true)
                    {
                        parsed.response_summary = Some("no output".to_string());
                    }
                    return parsed;
                }
            }

            if let Ok(out) = serde_json::from_str::<ToolOutput>(result) {
                let response_summary = out.response_summary.clone().or_else(|| {
                    if !out.stdout.is_empty() {
                        Some(format!("stdout {} chars", out.stdout.len()))
                    } else if !out.stderr.is_empty() {
                        Some(format!("stderr {} chars", out.stderr.len()))
                    } else {
                        Some("no output".to_string())
                    }
                });

                return ToolResult {
                    version: TOOL_RESULT_VERSION,
                    tool_name: tool_name.to_string(),
                    kind,
                    operation,
                    key_path,
                    success: out.stderr.is_empty(),
                    requires_confirmation: out.requires_confirmation,
                    executed: out.executed,
                    response_summary,
                    stdout: out.stdout,
                    stderr: out.stderr,
                    data: json!({ "command": out.command }),
                };
            }

            if let Ok(v) = serde_json::from_str::<Value>(result) {
                let pretty = serde_json::to_string_pretty(&v).unwrap_or_else(|_| result.clone());
                return ToolResult {
                    version: TOOL_RESULT_VERSION,
                    tool_name: tool_name.to_string(),
                    kind,
                    operation,
                    key_path,
                    success: true,
                    requires_confirmation: false,
                    executed: true,
                    response_summary: Some(format!("json {} chars", result.len())),
                    stdout: pretty,
                    stderr: String::new(),
                    data: v,
                };
            }

            ToolResult {
                version: TOOL_RESULT_VERSION,
                tool_name: tool_name.to_string(),
                kind,
                operation,
                key_path,
                success: true,
                requires_confirmation: false,
                executed: true,
                response_summary: Some(format!("{} chars", result.len())),
                stdout: result.clone(),
                stderr: String::new(),
                data: json!({ "raw": result }),
            }
        }
        Err(e) => ToolResult {
            version: TOOL_RESULT_VERSION,
            tool_name: tool_name.to_string(),
            kind,
            operation,
            key_path,
            success: false,
            requires_confirmation: false,
            executed: true,
            response_summary: Some("error".to_string()),
            stdout: String::new(),
            stderr: e.to_string(),
            data: json!({ "error": e.to_string() }),
        },
    }
}

#[derive(Clone, Debug)]
pub enum StreamStage {
    Thinking,
    Answering,
}

#[derive(Clone, Debug)]
pub enum StreamEvent {
    Text(String),
    StageStart(StreamStage),
    StageEnd(StreamStage),
    End,
}

pub type StreamCallback = Arc<dyn Fn(StreamEvent) + Send + Sync>;

/// Callback for tool execution
/// Takes (tool_ref, tool_name, arguments) and returns Result<String>
/// The callback receives the tool reference so it can execute it after confirmation
pub type ToolExecutorCallback = Arc<
    dyn Fn(
            &Box<dyn Tool>,
            &str,
            &str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>>
        + Send
        + Sync,
>;

use crate::config::ProviderConfig;

/// Main LLM Agent that orchestrates tool calls
pub struct Agent {
    provider_name: String,
    model_name: String,
    system_prompt: Option<String>,
    client: Arc<AnyProviderClient>,
    client_factory: ProviderClientFactory,
    /// Available provider configurations
    provider_configs: Vec<ProviderConfig>,
    /// Registered tools
    tools: Vec<Box<dyn Tool>>,
    /// Conversation history
    messages: Vec<Message>,
    /// Optional callback for streaming output
    stream_callback: Option<StreamCallback>,
    /// Optional callback for tool execution (for confirmation logic)
    tool_executor_callback: Option<ToolExecutorCallback>,
}

/// Agent execution result
#[derive(Debug)]
pub struct AgentResult {
    /// Final response content
    pub content: String,
    /// Whether tools were called
    pub tools_used: bool,
    /// Tool execution results
    #[allow(dead_code)]
    pub tool_results: Vec<ToolExecutionResult>,
}

/// Result of a tool execution
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub tool_name: String,
    pub success: bool,
    pub result: String,
}

impl Agent {
    /// Create a new Agent with registered tools
    ///
    /// # Arguments
    /// * `tools` - Vector of tools to register
    pub fn new(
        provider_name: String,
        model_name: String,
        system_prompt: Option<String>,
        provider_configs: Vec<ProviderConfig>,
        tools: Vec<Box<dyn Tool>>,
    ) -> Result<Self> {
        let mut client_factory = ProviderClientFactory::default();
        let client = client_factory.get_or_create(
            &provider_name,
            &model_name,
            &provider_configs,
            system_prompt.clone(),
        )?;

        Ok(Self {
            provider_name,
            model_name,
            system_prompt,
            client,
            client_factory,
            provider_configs,
            tools,
            messages: Vec::new(),
            stream_callback: None,
            tool_executor_callback: None,
        })
    }

    /// Set available provider configurations
    pub fn set_provider_configs(&mut self, configs: Vec<ProviderConfig>) {
        self.provider_configs = configs;
    }

    /// Get available models grouped by provider
    pub fn get_available_models(&self) -> Vec<(String, String)> {
        let mut models = Vec::new();
        for config in &self.provider_configs {
            for model in &config.models {
                models.push((config.name.clone(), model.clone()));
            }
        }
        models
    }

    /// Set current model and update provider if necessary
    pub fn set_model(&mut self, provider_name: &str, model_name: &str) -> Result<()> {
        let config = self
            .provider_configs
            .iter()
            .find(|c| c.name == provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_name))?;

        if !config.models.contains(&model_name.to_string()) {
            return Err(anyhow::anyhow!(
                "Model {} not found in provider {}",
                model_name,
                provider_name
            ));
        }

        self.provider_name = provider_name.to_string();
        self.model_name = model_name.to_string();
        self.client = self.client_factory.get_or_create(
            &self.provider_name,
            &self.model_name,
            &self.provider_configs,
            self.system_prompt.clone(),
        )?;

        Ok(())
    }

    pub fn set_system_prompt(&mut self, prompt: Option<String>) -> Result<()> {
        self.system_prompt = prompt;
        self.client = self.client_factory.get_or_create(
            &self.provider_name,
            &self.model_name,
            &self.provider_configs,
            self.system_prompt.clone(),
        )?;
        Ok(())
    }

    /// Get the current provider's base URL
    pub fn get_base_url(&self) -> String {
        self.provider_configs
            .iter()
            .find(|c| c.name == self.provider_name)
            .map(|c| c.base_url.clone())
            .unwrap_or_default()
    }

    pub fn get_provider_name(&self) -> String {
        self.provider_name.clone()
    }

    /// Get the current model name
    pub fn get_model_name(&self) -> String {
        self.model_name.clone()
    }

    /// Create a new Agent with no tools
    ///
    /// # Arguments
    /// * `provider` - LLM provider instance
    #[allow(dead_code)]
    pub fn without_tools(
        provider_name: String,
        model_name: String,
        system_prompt: Option<String>,
        provider_configs: Vec<ProviderConfig>,
    ) -> Result<Self> {
        Self::new(provider_name, model_name, system_prompt, provider_configs, Vec::new())
    }

    /// Set a callback for streaming content output
    ///
    /// # Arguments
    /// * `callback` - Function to call with each chunk of content
    pub fn set_stream_callback<F>(&mut self, callback: F)
    where
        F: Fn(StreamEvent) + Send + Sync + 'static,
    {
        self.stream_callback = Some(Arc::new(callback));
    }

    /// Set a callback for tool execution
    ///
    /// This callback will be called instead of the default execute_tool method.
    /// Useful for implementing confirmation logic or custom tool execution.
    ///
    /// # Arguments
    /// * `callback` - Async function that takes (tool_name, arguments) and returns Result<String>
    #[allow(dead_code)]
    pub fn set_tool_executor_callback(&mut self, callback: ToolExecutorCallback) {
        self.tool_executor_callback = Some(callback);
    }

    /// Clear the stream callback
    #[allow(dead_code)]
    pub fn clear_stream_callback(&mut self) {
        self.stream_callback = None;
    }

    /// Add a user message to conversation
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(Message {
            role: "user".to_string(),
            content,
        });
    }

    /// Add an assistant message to conversation
    pub fn add_assistant_message(&mut self, content: String) {
        self.messages.push(Message {
            role: "assistant".to_string(),
            content,
        });
    }

    pub fn export_messages(&self) -> Vec<Message> {
        self.messages.clone()
    }

    pub fn import_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Execute the agent with streaming LLM calls
    ///
    /// This method:
    /// 1. Calls LLM with streaming
    /// 2. Monitors for tool_call in the response
    /// 3. Executes tools when requested
    /// 4. Continues conversation with tool results
    pub async fn execute(&mut self) -> Result<AgentResult> {
        let mut tool_results = Vec::new();
        let mut final_content = String::new();
        let mut tools_used = false;

        // Prepare tool definitions
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|tool| tool.to_tool_definition())
            .collect();

        loop {
            log::info!("Calling LLM with {} messages", self.messages.len());

            // Get streaming response from LLM
            let mut stream = self
                .client
                .stream_chat(self.messages.clone(), Some(tools.clone()))
                .await
                .context("Failed to initiate LLM stream")?;

            let mut current_content = String::new();
            let mut tool_calls_map: HashMap<usize, (Option<String>, Option<String>, String)> =
                HashMap::new();
            let mut finish_reason: Option<String> = None;

            let mut thinking_sent = false;
            let mut thinking_ended = false;
            let mut answering_sent = false;

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result.context("Error reading stream chunk")?;

                log::debug!("Received chunk: {}", chunk);

                // Parse the chunk
                if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        if let Some(delta) = choice.get("delta") {
                            if let Some(reasoning) =
                                delta.get("reasoning_content").and_then(|c| c.as_str())
                            {
                                if !reasoning.trim().is_empty() {
                                    if !thinking_sent {
                                        if let Some(ref callback) = self.stream_callback {
                                            callback(StreamEvent::StageStart(StreamStage::Thinking));
                                        }
                                        thinking_sent = true;
                                    }
                                    if let Some(ref callback) = self.stream_callback {
                                        callback(StreamEvent::Text(reasoning.to_string()));
                                    }
                                }
                            }

                            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                if !content.is_empty() {
                                    if thinking_sent && !thinking_ended {
                                        if let Some(ref callback) = self.stream_callback {
                                            callback(StreamEvent::StageEnd(StreamStage::Thinking));
                                        }
                                        thinking_ended = true;
                                    }
                                    if !answering_sent {
                                        if let Some(ref callback) = self.stream_callback {
                                            callback(StreamEvent::StageStart(StreamStage::Answering));
                                        }
                                        answering_sent = true;
                                    }
                                    current_content.push_str(content);
                                    if let Some(ref callback) = self.stream_callback {
                                        callback(StreamEvent::Text(content.to_string()));
                                    }
                                }
                            }

                            // Check for tool calls - accumulate incrementally
                            if let Some(calls) = delta.get("tool_calls").and_then(|t| t.as_array())
                            {
                                for call in calls {
                                    let index =
                                        call.get("index").and_then(|i| i.as_u64()).unwrap_or(0)
                                            as usize;

                                    let entry =
                                        tool_calls_map.entry(index).or_insert((None, None, String::new()));

                                    if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
                                        if entry.0.is_none() {
                                            entry.0 = Some(id.to_string());
                                        }
                                    }

                                    if let Some(function) = call.get("function") {
                                        // Accumulate function name
                                        if let Some(name) =
                                            function.get("name").and_then(|n| n.as_str())
                                        {
                                            entry.1 = Some(name.to_string());
                                        }

                                        // Accumulate arguments
                                        if let Some(args) =
                                            function.get("arguments").and_then(|a| a.as_str())
                                        {
                                            entry.2.push_str(args);
                                        }
                                    }
                                }
                            }
                        }

                        // Check finish reason
                        if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
                            finish_reason = Some(reason.to_string());
                        }
                    }
                }
            }

            // Only log newline if using default stdout (not callback)
            if self.stream_callback.is_none() {
                println!();
            }

            // Prepare tool calls JSON
            let tool_calls_json_str = if !tool_calls_map.is_empty() {
                let mut calls: Vec<_> = tool_calls_map.iter().collect();
                calls.sort_by_key(|(k, _)| **k);
                let list: Vec<Value> = calls.into_iter().map(|(_, (id, name, args))| {
                    json!({
                        "id": id.clone().unwrap_or_default(),
                        "name": name.clone().unwrap_or_default(),
                        "arguments": args
                    })
                }).collect();
                Some(serde_json::to_string(&list).unwrap_or_default())
            } else {
                None
            };

            // Save assistant response (Content + ToolCalls)
            if !current_content.is_empty() || tool_calls_json_str.is_some() {
                let mut full_content = current_content.clone();
                if let Some(json_str) = &tool_calls_json_str {
                    if !full_content.is_empty() {
                        full_content.push_str("\n\n");
                    }
                    full_content.push_str(&format!("ToolCallsJSON:{}", json_str));
                }
                self.add_assistant_message(full_content);
                if !current_content.is_empty() {
                    final_content = current_content;
                }
            }

            // Check if we need to call tools
            if !tool_calls_map.is_empty() {
                log::info!("Tool calls detected: {}", tool_calls_map.len());
                tools_used = true;

                if let Some(ref callback) = self.stream_callback {
                    if thinking_sent && !thinking_ended {
                        callback(StreamEvent::StageEnd(StreamStage::Thinking));
                    }
                    if answering_sent {
                        callback(StreamEvent::StageEnd(StreamStage::Answering));
                    }
                }

                // Execute each tool call
                for (_index, (tool_call_id_opt, tool_name_opt, arguments_acc)) in tool_calls_map {
                    let tool_name = tool_name_opt.as_deref().unwrap_or("unknown");
                    let arguments = if arguments_acc.trim().is_empty() {
                        "{}"
                    } else {
                        arguments_acc.as_str()
                    };

                    log::info!("Executing tool: {} with args: {}", tool_name, arguments);

                    let tool_ref = self.find_tool(tool_name);
                    let kind = tool_ref.map(|t| t.kind()).unwrap_or(ToolKind::Other);
                    let op = tool_ref
                        .map(|t| t.operation())
                        .unwrap_or(ToolOperation::Other);

                    // Use tool_executor_callback if set, otherwise use default execute_tool
                    let execution_result = if let Some(ref callback) = self.tool_executor_callback {
                        log::debug!("Using tool executor callback");
                        // Find the tool first
                        if let Some(tool) = tool_ref {
                            callback(tool, tool_name, arguments).await
                        } else {
                            Err(anyhow::anyhow!("Tool not found: {}", tool_name))
                        }
                    } else {
                        log::debug!("Using default execute_tool");
                        self.execute_tool(tool_name, arguments).await
                    };

                    let tool_result =
                        tool_result_from_execution(tool_name, arguments, kind, op, &execution_result);
                    let tool_result_json = serde_json::to_string_pretty(&tool_result)
                        .unwrap_or_else(|_| "{\"error\":\"failed to serialize ToolResult\"}".to_string());

                    tool_results.push(ToolExecutionResult {
                        tool_name: tool_name.to_string(),
                        success: tool_result.success,
                        result: tool_result_json.clone(),
                    });

                    if !tool_result.success {
                        log::warn!(
                            "Tool '{}' execution failed. Error: {}. Arguments (first 500 chars): {:.500}", 
                            tool_name, 
                            tool_result.stderr, 
                            arguments
                        );
                    }

                    match &*self.client {
                        AnyProviderClient::Claude(_) => {
                            let tool_use_id = tool_call_id_opt.unwrap_or_default();
                            let result_value: Value = serde_json::from_str(&tool_result_json)
                                .unwrap_or_else(|_| json!({ "raw": tool_result_json }));
                            let payload = json!({
                                "tool_use_id": tool_use_id,
                                "result": result_value
                            });
                            self.add_user_message(format!("ToolResultJSON:{}", payload.to_string()));
                        }
                        _ => self.add_user_message(format!("ToolResult:\n{}", tool_result_json)),
                    }
                }

                // Continue loop to get LLM response to tool results
                continue;
            }

            if let Some(ref callback) = self.stream_callback {
                if thinking_sent && !thinking_ended {
                    callback(StreamEvent::StageEnd(StreamStage::Thinking));
                }
                if answering_sent {
                    callback(StreamEvent::StageEnd(StreamStage::Answering));
                }
            }

            // No more tool calls, we're done
            if let Some(reason) = finish_reason {
                log::info!("Stream finished with reason: {}", reason);
                if reason == "stop" || reason == "end" {
                    break;
                }
            }

            break;
        }

        if let Some(ref callback) = self.stream_callback {
            callback(StreamEvent::End);
        }

        Ok(AgentResult {
            content: final_content,
            tools_used,
            tool_results,
        })
    }

    /// Execute a specific tool
    ///
    /// This method finds and executes a tool by name with the given arguments.
    /// It's now public to allow tool execution callbacks to use it.
    pub async fn execute_tool(&self, tool_name: &str, arguments: &str) -> Result<String> {
        // Find the tool
        let tool_index = self
            .tools
            .iter()
            .position(|t| t.name() == tool_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tool_name))?;

        // Clone the arguments for the blocking task
        let arguments = arguments.to_string();

        // Execute the tool
        let tool = &self.tools[tool_index];
        tool.execute(&arguments)
    }

    /// Get conversation history
    #[allow(dead_code)]
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// Find a tool by name
    ///
    /// # Arguments
    /// * `tool_name` - The name of the tool to find
    ///
    /// # Returns
    /// * Option reference to the tool if found
    pub fn find_tool(&self, tool_name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == tool_name)
    }

    /// Clear conversation history
    pub fn clear_history(&mut self) {
        self.messages.clear();
    }
}
