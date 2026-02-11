/// Truncates a string to at most `max_bytes` while ensuring it's a valid UTF-8 sequence.
/// Adds an ellipsis if truncated.
pub fn truncate_utf8_with_ellipsis(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }

    let mut end = 0usize;
    for (i, ch) in s.char_indices() {
        let next = i + ch.len_utf8();
        if next <= max_bytes {
            end = next;
        } else {
            break;
        }
    }

    if end == 0 && !s.is_empty() {
        let first_end = s.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
        end = std::cmp::min(first_end, s.len());
    }

    format!("{}...", &s[..end])
}

/// Truncates a string by keeping the start and end, and showing a truncation notice in the middle.
/// Ensures all slices are valid UTF-8.
pub fn truncate_middle(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }

    let half_length = max_bytes / 2;
    
    // Find safe UTF-8 boundary for the start part
    let mut start_end = 0;
    for (i, ch) in content.char_indices() {
        if i + ch.len_utf8() <= half_length {
            start_end = i + ch.len_utf8();
        } else {
            break;
        }
    }
    
    let start = &content[..start_end];
    
    // For the end part, we want at most half_length bytes from the end, starting at a character boundary
    let target_end_start = content.len().saturating_sub(half_length);
    let mut end_start = content.len();
    for (i, _) in content.char_indices() {
        if i >= target_end_start {
            end_start = i;
            break;
        }
    }

    let end = &content[end_start..];
    let truncated_part = &content[start_end..end_start];
    let truncated_lines = truncated_part.lines().count();

    format!(
        "{}\n\n... [{} lines truncated] ...\n\n{}",
        start, truncated_lines, end
    )
}
