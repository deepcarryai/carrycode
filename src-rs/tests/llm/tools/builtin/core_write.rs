use crate::llm::tools::builtin::core_write::{CoreWriteTool, CoreWriteRequest};
use crate::llm::tools::builtin::core_tool_base::ToolSpec;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_write_append_appends_content() {
        let tool = CoreWriteTool::default();
        let rel_path = "target/tmp_core_write_append_test.txt";
        let _ = std::fs::remove_file(rel_path);

        tool.run_write(&CoreWriteRequest {
            file_path: rel_path.to_string(),
            content: "hello".to_string(),
            append: false,
        })
        .unwrap();

        tool.run_write(&CoreWriteRequest {
            file_path: rel_path.to_string(),
            content: " world".to_string(),
            append: true,
        })
        .unwrap();

        let out = std::fs::read_to_string(rel_path).unwrap();
        assert_eq!(out, "hello world");
        let _ = std::fs::remove_file(rel_path);
    }

    #[test]
    fn core_write_counts_chars_not_bytes() {
        let tool = CoreWriteTool::default();
        let rel_path = "target/tmp_core_write_multibyte_test.txt";
        let _ = std::fs::remove_file(rel_path);

        let content = "你".repeat(1000);
        <CoreWriteTool as ToolSpec>::run(
            &tool,
            CoreWriteRequest {
                file_path: rel_path.to_string(),
                content,
                append: false,
            },
            false,
        )
        .unwrap();

        let out = std::fs::read_to_string(rel_path).unwrap();
        assert_eq!(out.chars().count(), 1000);
        let _ = std::fs::remove_file(rel_path);
    }

    #[test]
    fn core_write_rejects_oversized_content() {
        let tool = CoreWriteTool::default();
        let args = CoreWriteRequest {
            file_path: "target/tmp_core_write_limit_test.txt".to_string(),
            content: "a".repeat(100000),
            append: false,
        };
        let err = <CoreWriteTool as ToolSpec>::run(&tool, args, false).unwrap_err();
        assert!(err.to_string().contains("content too long"));
    }
}