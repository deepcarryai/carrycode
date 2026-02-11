use crate::llm::tools::builtin::core_edit::CoreEditTool;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_shows_two_lines_of_context() {
        let old = (1..=10).map(|n| format!("{n}\n")).collect::<String>();
        let new = (1..=10)
            .map(|n| {
                if n == 5 {
                    "5x\n".to_string()
                } else {
                    format!("{n}\n")
                }
            })
            .collect::<String>();

        let (out, adds, rems): (String, usize, usize) = CoreEditTool::calculate_diff(&old, &new, "test.txt");
        assert_eq!(adds, 1);
        assert_eq!(rems, 1);

        assert!(out.contains("diff --git a/test.txt b/test.txt\n"));
        assert!(out.contains("--- a/test.txt\n"));
        assert!(out.contains("+++ b/test.txt\n"));
        assert!(out.contains("@@ -3,5 +3,5 @@\n"));
        assert!(out.contains(" 3\n"));
        assert!(out.contains(" 4\n"));
        assert!(out.contains("-5\n"));
        assert!(out.contains("+5x\n"));
        assert!(out.contains(" 6\n"));
        assert!(out.contains(" 7\n"));

        assert!(!out.contains(" 1\n"));
        assert!(!out.contains("10\n"));
    }
}