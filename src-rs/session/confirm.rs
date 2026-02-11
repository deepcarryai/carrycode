use std::sync::Arc;

use crate::llm::utils::file_tracker::PathSecurity;
use super::manager::SESSION_MANAGER;
use super::types::ConfirmationStatus;

use crate::llm::utils::tool_access::ToolAccessLevel;
use crate::policy::path_policy::PathPolicy;
use crate::policy::policy_text::{display_width, truncate_to_width, truncate_to_width_with_ellipsis};

pub fn get_confirmation_status(
    session_id: &str,
    tool_name: &str,
    file_path: &str,
) -> Option<ConfirmationStatus> {
    let map_arc_opt = {
        let manager = SESSION_MANAGER.lock().ok()?;
        let ctx = manager.get(session_id)?;
        Some(Arc::clone(&ctx.tool_confirm))
    };

    if let Some(map_arc) = map_arc_opt {
        if let Ok(map) = map_arc.lock() {
            let key = (tool_name.to_string(), file_path.to_string());
            map.get(&key).copied()
        } else {
            None
        }
    } else {
        None
    }
}

pub fn set_confirmation_status(
    session_id: &str,
    tool_name: &str,
    file_path: &str,
    status: ConfirmationStatus,
) {
    if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(session_id) {
            if let Ok(mut map) = ctx.tool_confirm.lock() {
                let key = (tool_name.to_string(), file_path.to_string());
                map.insert(key, status);
            }
        }
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

pub fn key_path_from_args(
    tool_name: &str,
    args_json: &str,
    tool_definition: Option<&serde_json::Value>,
    access_level: Option<ToolAccessLevel>,
) -> String {
    let value: serde_json::Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(_) => {
            let repaired = escape_control_chars_in_json_strings(args_json);
            match serde_json::from_str(&repaired) {
                Ok(v) => v,
                Err(_) => return "*".to_string(),
            }
        }
    };

    let tool_name_lower = tool_name.to_lowercase();
    
    // Helper for path normalization based on access level
    let to_abs = |p: &str| {
        if p == "*" || p.is_empty() {
            return "*".to_string();
        }
        
        // Use PathPolicy if possible to be consistent with tool execution
        if let Ok(policy) = PathPolicy::new_with_level(access_level.unwrap_or(ToolAccessLevel::Workspace)) {
            policy.resolve(p).map(|pb| pb.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string())
        } else {
            PathSecurity::to_absolute_path(p).unwrap_or_else(|_| p.to_string())
        }
    };

    let clean_cmd = |c: &str| {
        let first_line = c.lines().next().unwrap_or("").trim();
        if c.lines().count() > 1 {
            format!("{}...", truncate_to_width(first_line, 61))
        } else if display_width(first_line) > 64 {
            truncate_to_width_with_ellipsis(first_line, 64).into_owned()
        } else {
            first_line.to_string()
        }
    };

    // 1) Strategy-based key extraction
    // First, check if we have a definition to be more precise
    if let Some(def) = tool_definition {
        let props = def.get("function").and_then(|f| f.get("parameters")).and_then(|p| p.get("properties"));
        
        if let Some(p) = props {
            if p.get("file_path").is_some() {
                if let Some(fp) = value.get("file_path").and_then(|v| v.as_str()) {
                    return to_abs(fp);
                }
            } else if p.get("command").is_some() {
                if let Some(cmd) = value.get("command").and_then(|v| v.as_str()) {
                    return clean_cmd(cmd);
                }
            } else if p.get("url").is_some() {
                if let Some(url) = value.get("url").and_then(|v| v.as_str()) {
                    return truncate_to_width_with_ellipsis(url, 64).into_owned();
                }
            } else if p.get("path").is_some() && p.get("pattern").is_some() {
                // Special case for glob/grep: combine path and pattern
                let path = value.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let pattern = value.get("pattern").and_then(|v| v.as_str()).unwrap_or("*");
                return format!("{} :: {}", to_abs(path), pattern);
            } else if p.get("path").is_some() {
                if let Some(path) = value.get("path").and_then(|v| v.as_str()) {
                    return to_abs(path);
                }
            }
        }
    }

    // 2) Fallback to name-based logic if schema inference failed
    let (final_key, final_use_path, final_use_cmd) = {
        if tool_name_lower.contains("bash") || tool_name_lower.contains("execute") {
            (value.get("command").or_else(|| value.get("cmd")).and_then(|v| v.as_str()), false, true)
        } else if tool_name_lower.contains("write") || tool_name_lower.contains("edit") || tool_name_lower.contains("view") || tool_name_lower.contains("diagnostics") || tool_name_lower.contains("read") {
            (value.get("file_path").or_else(|| value.get("filepath")).or_else(|| value.get("path")).and_then(|v| v.as_str()), true, false)
        } else if tool_name_lower.contains("ls") || tool_name_lower.contains("grep") || tool_name_lower.contains("glob") || tool_name_lower.contains("tree") {
            (value.get("path").or_else(|| value.get("directory")).or_else(|| value.get("pattern")).and_then(|v| v.as_str()), true, false)
        } else if tool_name_lower.contains("fetch") || tool_name_lower.contains("curl") || tool_name_lower.contains("http") {
            (value.get("url").and_then(|v| v.as_str()), false, false)
        } else {
            (value.get("file_path").or_else(|| value.get("path")).or_else(|| value.get("command")).or_else(|| value.get("url")).and_then(|v| v.as_str()), false, false)
        }
    };

    // 3) Processing based on flags
    match final_key {
        Some(k) if k.is_empty() => "*".to_string(),
        Some(k) => {
            if final_use_cmd {
                clean_cmd(k)
            } else if final_use_path {
                to_abs(k)
            } else {
                truncate_to_width_with_ellipsis(k, 64).into_owned()
            }
        },
        None => "*".to_string(),
    }
}

