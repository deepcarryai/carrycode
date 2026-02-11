use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub fn display_width_ascii_fast(text: &str) -> Option<usize> {
    if text
        .as_bytes()
        .iter()
        .all(|byte| (0x20..=0x7E).contains(byte))
    {
        return Some(text.len());
    }
    None
}

pub fn display_width(text: &str) -> usize {
    if let Some(width) = display_width_ascii_fast(text) {
        return width;
    }
    UnicodeWidthStr::width_cjk(text)
}

pub fn split_at_width(text: &str, width: usize) -> (Cow<'_, str>, Cow<'_, str>) {
    if width == 0 || text.is_empty() {
        return (Cow::Borrowed(""), Cow::Borrowed(text));
    }

    let mut current_width = 0usize;
    let mut end_byte_index = 0usize;

    for grapheme in text.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width_cjk(grapheme);

        if current_width + grapheme_width > width {
            break;
        }

        current_width += grapheme_width;
        end_byte_index += grapheme.len();

        if current_width == width {
            break;
        }
    }

    (Cow::Borrowed(&text[..end_byte_index]), Cow::Borrowed(&text[end_byte_index..]))
}

pub fn truncate_to_width(text: &str, max_width: usize) -> Cow<'_, str> {
    let (prefix, _) = split_at_width(text, max_width);
    prefix
}

pub fn truncate_to_width_with_ellipsis(text: &str, max_width: usize) -> Cow<'_, str> {
    if max_width == 0 {
        return Cow::Borrowed("");
    }

    if display_width(text) <= max_width {
        return Cow::Borrowed(text);
    }

    if max_width <= 3 {
        return truncate_to_width(text, max_width);
    }

    let prefix = truncate_to_width(text, max_width - 3).into_owned();
    Cow::Owned(format!("{}...", prefix))
}

pub fn pad_right_to_width(text: &str, width: usize) -> String {
    let current_width = display_width(text);
    if current_width >= width {
        return text.to_string();
    }

    let mut padded = String::with_capacity(text.len() + (width - current_width));
    padded.push_str(text);
    padded.extend(std::iter::repeat(' ').take(width - current_width));
    padded
}

pub fn display_width_stripped_ansi(text: &str) -> usize {
    let re = regex::Regex::new(r"[\u001b\u009b]\[[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-ORZcf-nqry=><]").unwrap();
    let stripped = re.replace_all(text, "");
    display_width(&stripped)
}

pub fn truncate_to_width_stripped_ansi(text: &str, max_width: usize) -> Cow<'_, str> {
    let re = regex::Regex::new(r"[\u001b\u009b]\[[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-ORZcf-nqry=><]").unwrap();
    let stripped = re.replace_all(text, "");
    Cow::Owned(truncate_to_width(&stripped, max_width).into_owned())
}

