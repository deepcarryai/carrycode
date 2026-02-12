use std::sync::Arc;

use crate::llm::utils::file_tracker::PathSecurity;
use super::manager::SESSION_MANAGER;
use super::types::ConfirmationStatus;

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

pub fn key_path_from_args(tool_name: &str, args_json: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(args_json) {
        Ok(value) => {
            let to_abs = |p: &str| {
                if p == "*" {
                    return "*".to_string();
                }
                PathSecurity::to_absolute_path(p).unwrap_or_else(|_| p.to_string())
            };

            match tool_name {
                // Tools with 'command'
                "bash" => {
                    value.get("command").and_then(|v| v.as_str()).unwrap_or("*").to_string()
                },
                // Tools with 'file_path'
                "edit" | "view" | "write" | "diagnostics" => {
                    to_abs(value.get("file_path").and_then(|v| v.as_str()).unwrap_or("*"))
                },
                // Tools with 'url'
                "fetch" => {
                    value.get("url").and_then(|v| v.as_str()).unwrap_or("*").to_string()
                },
                // Tools with 'path' (optional)
                "ls" | "grep" => {
                    to_abs(value.get("path").and_then(|v| v.as_str()).unwrap_or("*"))
                },
                "glob" => {
                    value.get("pattern").and_then(|v| v.as_str()).unwrap_or("*").to_string()
                },
                // Tools with no specific path or global scope
                "todo_write" => {
                    "*".to_string()
                },
                // Fallback for unknown tools
                _ => {
                    if let Some(p) = value.get("path").and_then(|v| v.as_str()) {
                        to_abs(p)
                    } else if let Some(p) = value.get("file_path").and_then(|v| v.as_str()) {
                        to_abs(p)
                    } else if let Some(c) = value.get("command").and_then(|v| v.as_str()) {
                        c.to_string()
                    } else {
                        "*".to_string()
                    }
                }
            }
        }
        Err(_) => "*".to_string(),
    }
}
