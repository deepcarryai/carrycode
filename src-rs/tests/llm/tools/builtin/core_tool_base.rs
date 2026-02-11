use crate::llm::tools::builtin::core_tool_base::parse_confirmed_and_args;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Args {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repairs_control_chars_inside_string() {
        let raw = "{\"confirmed\":true,\"content\":\"a\nb\"}";
        let (args, confirmed) = parse_confirmed_and_args::<Args>(raw).expect("should parse after repair");
        assert!(confirmed);
        assert_eq!(args.content, "a\nb");
    }

    #[test]
    fn parse_still_fails_for_truncated_json() {
        let raw = "{\"content\":\"a\nb\"";
        assert!(parse_confirmed_and_args::<Args>(raw).is_err());
    }
}