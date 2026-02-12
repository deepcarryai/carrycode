use crate::llm::config::AppConfig;
use crate::llm::tools::tool_trait::{ToolKind, ToolOperation, ToolResult, ToolSpec};
use crate::llm::utils::path_policy::PathPolicy;
use crate::llm::utils::tool_access::is_full_access;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{fs, thread};

const MAX_OUTPUT_LENGTH: usize = 30000;

// Persistent shell implementation
#[derive(Debug)]
struct PersistentShell {
    child: Arc<Mutex<Option<Child>>>,
}

#[derive(Debug)]
struct CommandResult {
    stdout: String,
    stderr: String,
    exit_code: i32,
    interrupted: bool,
    start_time: i64,
    end_time: i64,
}

impl PersistentShell {
    fn new(cwd: &str) -> Result<Self> {
        let child = Command::new("bash")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("GIT_EDITOR", "true")
            .spawn()
            .context("Failed to spawn bash process")?;

        Ok(Self {
            child: Arc::new(Mutex::new(Some(child))),
        })
    }

    fn exec(&self, command: &str, timeout_ms: u64) -> Result<CommandResult> {
        let start_time = Instant::now();
        let start_time_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let stdout_file = temp_dir.join(format!("carrycode-stdout-{}", timestamp));
        let stderr_file = temp_dir.join(format!("carrycode-stderr-{}", timestamp));
        let status_file = temp_dir.join(format!("carrycode-status-{}", timestamp));

        let full_command = format!(
            "({}) > {} 2> {}; echo $? > {}\n",
            command,
            shell_quote(&stdout_file.to_string_lossy()),
            shell_quote(&stderr_file.to_string_lossy()),
            shell_quote(&status_file.to_string_lossy())
        );

        let child_clone = Arc::clone(&self.child);
        let mut child_guard = child_clone.lock().unwrap();

        if let Some(ref mut child) = *child_guard {
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(full_command.as_bytes())?;
                stdin.flush()?;
            }
        }
        drop(child_guard);

        let timeout = Duration::from_millis(timeout_ms);
        let mut interrupted = false;

        loop {
            if status_file.exists()
                && fs::metadata(&status_file)
                    .ok()
                    .is_some_and(|m| m.len() > 0)
            {
                break;
            }

            if start_time.elapsed() >= timeout {
                interrupted = true;
                self.kill_children()?;
                break;
            }

            thread::sleep(Duration::from_millis(10));
        }

        let stdout = fs::read_to_string(&stdout_file).unwrap_or_default();
        let stderr = fs::read_to_string(&stderr_file).unwrap_or_default();
        let exit_code_str = fs::read_to_string(&status_file).unwrap_or_default();

        let exit_code = if !exit_code_str.is_empty() {
            exit_code_str.trim().parse().unwrap_or(0)
        } else if interrupted {
            143
        } else {
            0
        };

        let _ = fs::remove_file(&stdout_file);
        let _ = fs::remove_file(&stderr_file);
        let _ = fs::remove_file(&status_file);

        let end_time_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        Ok(CommandResult {
            stdout,
            stderr,
            exit_code,
            interrupted,
            start_time: start_time_ms,
            end_time: end_time_ms,
        })
    }

    fn kill_children(&self) -> Result<()> {
        let child_guard = self.child.lock().unwrap();
        if let Some(ref child) = *child_guard {
            let pid = child.id();
            let _ = Command::new("pkill")
                .arg("-P")
                .arg(pid.to_string())
                .output();
        }
        Ok(())
    }

    fn close(&self) -> Result<()> {
        let mut child_guard = self.child.lock().unwrap();
        if let Some(mut child) = child_guard.take() {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(b"exit\n");
            }
            let _ = child.kill();
        }
        Ok(())
    }
}

impl Drop for PersistentShell {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

lazy_static::lazy_static! {
    static ref GLOBAL_SHELL: Mutex<Option<Arc<PersistentShell>>> = Mutex::new(None);
}

fn get_persistent_shell(cwd: &str) -> Result<Arc<PersistentShell>> {
    let mut shell_guard = GLOBAL_SHELL.lock().unwrap();

    if shell_guard.is_none() {
        let shell = PersistentShell::new(cwd)?;
        *shell_guard = Some(Arc::new(shell));
    }

    Ok(Arc::clone(shell_guard.as_ref().unwrap()))
}

// Bash tool implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashTool {
    pub tool_name: String,
    pub description: String,
    pub banned_commands: Vec<String>,
    pub safe_read_only_commands: Vec<String>,
}

use crate::llm::utils::serde_util::deserialize_u64_opt_lax;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashRequest {
    pub command: String,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_u64_opt_lax")]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BashResult {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub requires_confirmation: bool,
    pub executed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupted: Option<bool>,
    /// Summary of the result
    pub response_summary: String,
}

impl BashTool {
    pub fn new() -> Self {
        match AppConfig::load() {
            Ok(config) => Self {
                tool_name: config.tool_bash.tool_name,
                description: config.tool_bash.description,
                banned_commands: config.tool_bash.banned_commands,
                safe_read_only_commands: config.tool_bash.safe_read_only_commands,
            },
            Err(_) => Self::default(),
        }
    }

    #[allow(dead_code)]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            tool_name: config.tool_bash.tool_name.clone(),
            description: config.tool_bash.description.clone(),
            banned_commands: config.tool_bash.banned_commands.clone(),
            safe_read_only_commands: config.tool_bash.safe_read_only_commands.clone(),
        }
    }

    pub fn get_primary_command(&self, command: &str) -> String {
        command
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    }

    pub fn is_banned(&self, command: &str) -> bool {
        if is_full_access() {
            return false;
        }
        let primary = self.get_primary_command(command);
        self.banned_commands
            .iter()
            .any(|banned| primary == *banned || primary.starts_with(&format!("{} ", banned)))
    }

    pub fn is_safe_read_only(&self, command: &str) -> bool {
        let cmd = command.trim();
        self.safe_read_only_commands.iter().any(|safe| {
            if cmd == *safe {
                return true;
            }
            if cmd.starts_with(safe) {
                let remaining = &cmd[safe.len()..];
                return remaining.is_empty()
                    || remaining.starts_with(' ')
                    || remaining.starts_with('\t');
            }
            false
        })
    }

    pub fn run_bash(&self, request: &BashRequest) -> Result<BashResult> {
        if self.is_banned(&request.command) {
            let primary = self.get_primary_command(&request.command);
            return Ok(BashResult {
                command: request.command.clone(),
                exit_code: None,
                stdout: format!("Command '{}' is not allowed", primary),
                stderr: String::new(),
                requires_confirmation: false,
                executed: false,
                start_time: None,
                end_time: None,
                interrupted: None,
                response_summary: format!("Command '{}' is not allowed", primary),
            });
        }

        if self.is_safe_read_only(&request.command) {
            return self.execute_command(request, true);
        }

        // Check if confirmation is provided
        if request.confirmed {
            return self.execute_command(request, true);
        }

        let primary = self.get_primary_command(&request.command);
        Ok(BashResult {
            command: request.command.clone(),
            exit_code: None,
            stdout: format!("Command '{}' requires confirmation. Please respond with 'yes' to execute or 'no' to cancel.", primary),
            stderr: String::new(),
            requires_confirmation: true,
            executed: false,
            start_time: None,
            end_time: None,
            interrupted: None,
            response_summary: "Requires confirmation".to_string(),
        })
    }

    fn execute_command(&self, request: &BashRequest, _confirmed: bool) -> Result<BashResult> {
        let command_str = request.command.trim();
        let timeout = request.timeout.unwrap_or(1800000).min(600000);

        let workdir = if let Some(wd) = request.workdir.as_ref() {
            let policy = PathPolicy::new()?;
            policy.resolve(wd)?.to_string_lossy().to_string()
        } else {
            std::env::current_dir().unwrap().to_string_lossy().to_string()
        };

        let shell = get_persistent_shell(&workdir)?;
        let result = shell.exec(command_str, timeout)?;

        let stdout = truncate_output(&result.stdout);
        let stderr = truncate_output(&result.stderr);

        let mut error_message = stderr.clone();
        if result.interrupted {
            if !error_message.is_empty() {
                error_message.push('\n');
            }
            error_message.push_str("Command was aborted before completion");
        } else if result.exit_code != 0 {
            if !error_message.is_empty() {
                error_message.push('\n');
            }
            error_message.push_str(&format!("Exit code {}", result.exit_code));
        }

        let has_both_outputs = !stdout.is_empty() && !stderr.is_empty();
        let mut final_stdout = stdout;
        if has_both_outputs {
            final_stdout.push('\n');
        }
        if !error_message.is_empty() {
            final_stdout.push('\n');
            final_stdout.push_str(&error_message);
        }

        // Calculate summary (first 3 lines)
        let response_summary = final_stdout.lines().take(3).collect::<Vec<_>>().join("\n");

        Ok(BashResult {
            command: request.command.clone(),
            exit_code: Some(result.exit_code),
            stdout: if final_stdout.is_empty() {
                "no output".to_string()
            } else {
                final_stdout
            },
            stderr: String::new(),
            requires_confirmation: false,
            executed: true,
            start_time: Some(result.start_time),
            end_time: Some(result.end_time),
            interrupted: Some(result.interrupted),
            response_summary: if response_summary.is_empty() {
                "no output".to_string()
            } else {
                response_summary
            },
        })
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
                        "command": {
                            "type": "string",
                            "description": "The bash command to execute"
                        },
                        "workdir": {
                            "type": "string",
                            "description": "Optional working directory for command. Defaults to current working directory."
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Optional timeout in milliseconds (max 600000ms / 10 minutes). Defaults to 1800000ms (30 minutes).",
                            "maximum": 600000
                        }
                    },
                    "required": ["command"]
                }
            }
        })
    }
}

pub fn truncate_output(content: &str) -> String {
    if content.len() <= MAX_OUTPUT_LENGTH {
        return content.to_string();
    }

    let half_length = MAX_OUTPUT_LENGTH / 2;
    let start = &content[..half_length];
    let end = &content[content.len() - half_length..];

    let truncated_part = &content[half_length..content.len() - half_length];
    let truncated_lines = truncated_part.lines().count();

    format!(
        "{}\n\n... [{} lines truncated] ...\n\n{}",
        start, truncated_lines, end
    )
}

impl Default for BashTool {
    fn default() -> Self {
        Self {
            tool_name: "bash".to_string(),
            description: "Executes a given bash command in a persistent shell session.".to_string(),
            banned_commands: vec![
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
            ],
            safe_read_only_commands: vec![
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
            ],
        }
    }
}

impl ToolSpec for BashTool {
    type Args = BashRequest;

    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Execute
    }

    fn operation(&self) -> ToolOperation {
        ToolOperation::Bash
    }

    fn to_tool_definition(&self) -> serde_json::Value {
        self.to_tool_definition_json()
    }

    fn run(&self, mut args: Self::Args, confirmed: bool) -> Result<ToolResult> {
        args.confirmed = confirmed;
        let result = self.run_bash(&args)?;
        let mut tr = ToolResult::ok(
            self.tool_name.clone(),
            self.kind(),
            self.operation(),
            result.stdout.clone(),
            serde_json::to_value(&result)?,
        )
        .with_summary(result.response_summary.clone());
        tr.stderr = result.stderr.clone();
        tr.requires_confirmation = result.requires_confirmation;
        tr.executed = result.executed;
        tr.success = if !result.executed {
            true
        } else {
            result.exit_code.unwrap_or(0) == 0
        };
        Ok(tr)
    }
}
