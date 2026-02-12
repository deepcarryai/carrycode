use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

use crate::lsp::protocol::Message;

/// Reads LSP messages from stdout with Content-Length framing
pub struct MessageReader {
    reader: BufReader<ChildStdout>,
}

impl MessageReader {
    pub fn new(stdout: ChildStdout) -> Self {
        Self {
            reader: BufReader::new(stdout),
        }
    }

    pub async fn read_message(&mut self) -> Result<Message> {
        // Read headers
        let mut content_length = None;
        let mut line = String::new();

        loop {
            line.clear();
            self.reader.read_line(&mut line).await?;

            if line == "\r\n" || line == "\n" {
                break;
            }

            if line.starts_with("Content-Length: ") {
                let len_str = line.trim_start_matches("Content-Length: ").trim();
                content_length = Some(len_str.parse::<usize>()?);
            }
        }

        let content_length = content_length.context("Missing Content-Length header")?;

        // Read content
        let mut buffer = vec![0u8; content_length];
        self.reader.read_exact(&mut buffer).await?;

        let message: Message = serde_json::from_slice(&buffer)?;
        Ok(message)
    }
}

/// Writes LSP messages to stdin with Content-Length framing
pub struct MessageWriter {
    writer: ChildStdin,
}

impl MessageWriter {
    pub fn new(stdin: ChildStdin) -> Self {
        Self { writer: stdin }
    }

    pub async fn write_message(&mut self, message: &Message) -> Result<()> {
        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        self.writer.write_all(header.as_bytes()).await?;
        self.writer.write_all(content.as_bytes()).await?;
        self.writer.flush().await?;

        Ok(())
    }
}
