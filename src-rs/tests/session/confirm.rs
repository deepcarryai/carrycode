use crate::session::confirm::*;
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_path_from_args_schema_driven() {
        let def = json!({
            "function": {
                "name": "core_write",
                "parameters": {
                    "properties": {
                        "file_path": { "type": "string" },
                        "content": { "type": "string" }
                    }
                }
            }
        });
        let args = json!({ "file_path": "test.txt", "content": "hello" }).to_string();
        let key = key_path_from_args("any_name", &args, Some(&def), None);
        assert!(key.contains("test.txt"));
    }

    #[test]
    fn test_key_path_from_args_repairs_unescaped_newlines() {
        let def = json!({
            "function": {
                "name": "core_write",
                "parameters": {
                    "properties": {
                        "file_path": { "type": "string" },
                        "content": { "type": "string" }
                    }
                }
            }
        });
        let args = "{\"file_path\":\"test.txt\",\"content\":\"a\nb\"}";
        let key = key_path_from_args("any_name", args, Some(&def), None);
        assert!(key.contains("test.txt"));
    }

    #[test]
    fn test_key_path_from_args_glob_combined() {
        let def = json!({
            "function": {
                "name": "core_glob",
                "parameters": {
                    "properties": {
                        "path": { "type": "string" },
                        "pattern": { "type": "string" }
                    }
                }
            }
        });
        let args = json!({ "path": "src", "pattern": "*.rs" }).to_string();
        let key = key_path_from_args("any_name", &args, Some(&def), None);
        assert!(key.contains("src"));
        assert!(key.contains(":: *.rs"));
    }

    #[test]
    fn test_key_path_from_args_bash_command() {
        let def = json!({
            "function": {
                "name": "core_bash",
                "parameters": {
                    "properties": {
                        "command": { "type": "string" }
                    }
                }
            }
        });
        let args = json!({ "command": "ls -la\necho hello" }).to_string();
        let key = key_path_from_args("any_name", &args, Some(&def), None);
        assert_eq!(key, "ls -la...");
    }

    #[test]
    fn test_key_path_from_args_fallback() {
        let args = json!({ "file_path": "fallback.txt" }).to_string();
        let key = key_path_from_args("core_write", &args, None, None);
        assert!(key.contains("fallback.txt"));
    }
}