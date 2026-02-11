use crate::policy::policy_text::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_ascii_and_cjk() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("你好"), 4);
        assert_eq!(display_width("a你b"), 4);
    }

    #[test]
    fn width_combining_mark() {
        let s = format!("e\u{301}");
        assert_eq!(display_width(&s), 1);
    }

    #[test]
    fn split_and_truncate_by_width() {
        let (prefix, suffix) = split_at_width("a你b", 3);
        assert_eq!(prefix, "a你");
        assert_eq!(suffix, "b");

        assert_eq!(truncate_to_width("你", 1), "");
        assert_eq!(truncate_to_width("你", 2), "你");
    }

    #[test]
    fn truncate_with_ellipsis() {
        assert_eq!(truncate_to_width_with_ellipsis("abcdef", 6), "abcdef");
        assert_eq!(truncate_to_width_with_ellipsis("abcdef", 5), "ab...");
        assert_eq!(truncate_to_width_with_ellipsis("你好世界", 5), "你...");
    }

    #[test]
    fn pad_right() {
        let padded = pad_right_to_width("你", 4);
        assert_eq!(display_width(&padded), 4);
        assert!(padded.ends_with("  "));
    }

    #[test]
    fn width_stripped_ansi() {
        let red = "\u{1b}[31m你\u{1b}[0m";
        assert_eq!(display_width_stripped_ansi(red), 2);
        assert_eq!(truncate_to_width_stripped_ansi(red, 2), "你");
    }
}