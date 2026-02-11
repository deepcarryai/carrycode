use crate::llm::models::openai::{extract_sse_frame_from_buffer, sse_data_from_frame};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_data_from_frame_supports_data_without_space() {
        let frame = "data:{\"x\":1}\n";
        let data = sse_data_from_frame(frame).expect("should extract data");
        assert_eq!(data, "{\"x\":1}");
    }

    #[test]
    fn sse_data_from_frame_joins_multiple_data_lines() {
        let frame = "event: message\ndata: a\ndata: b\n";
        let data = sse_data_from_frame(frame).expect("should extract data");
        assert_eq!(data, "a\nb");
    }

    #[test]
    fn extract_sse_frame_from_buffer_handles_crlf_delimiter() {
        let mut buffer = b"data: 1\r\n\r\ndata: 2\r\n\r\n".to_vec();

        let frame1 = extract_sse_frame_from_buffer(&mut buffer).expect("frame1");
        let data1 = sse_data_from_frame(&String::from_utf8_lossy(&frame1)).expect("data1");
        assert_eq!(data1, "1");

        let frame2 = extract_sse_frame_from_buffer(&mut buffer).expect("frame2");
        let data2 = sse_data_from_frame(&String::from_utf8_lossy(&frame2)).expect("data2");
        assert_eq!(data2, "2");
    }

    #[test]
    fn extract_sse_frame_from_buffer_handles_lf_delimiter() {
        let mut buffer = b"data: 1\n\ndata: 2\n\n".to_vec();

        let frame1 = extract_sse_frame_from_buffer(&mut buffer).expect("frame1");
        let data1 = sse_data_from_frame(&String::from_utf8_lossy(&frame1)).expect("data1");
        assert_eq!(data1, "1");

        let frame2 = extract_sse_frame_from_buffer(&mut buffer).expect("frame2");
        let data2 = sse_data_from_frame(&String::from_utf8_lossy(&frame2)).expect("data2");
        assert_eq!(data2, "2");
    }
}