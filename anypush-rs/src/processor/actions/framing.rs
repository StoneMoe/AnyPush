//! Framing actions for stream processing - now using YieldFrame approach

use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_yaml::Value;
use std::collections::HashMap;
use tokio::io::AsyncBufReadExt;
use tracing::{debug, info, warn};

/// foreach_line action - reads stream line by line, yields frames
pub struct ForeachLineAction {
    /// Whether to trim whitespace from lines
    trim: bool,
    /// Whether to skip empty lines
    skip_empty: bool,
}

impl ForeachLineAction {
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let trim = params.get("trim").and_then(|v| v.as_bool()).unwrap_or(true);
        let skip_empty = params
            .get("skip_empty")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        Ok(Self { trim, skip_empty })
    }
}

#[async_trait]
impl Action for ForeachLineAction {
    fn name(&self) -> &str {
        "foreach_line"
    }

    fn is_final(&self) -> bool {
        false // Uses YieldFrame, not final
    }

    async fn execute(&self, mut message: Message) -> Result<ActionResult> {
        // Check for stream
        let Some(stream_reader) = message.take_stream() else {
            warn!("foreach_line: message is not a stream, treating as buffered text");
            // If it's not a stream, treat text field as single-line message
            if let Some(text) = message.metadata().get_str("text") {
                // Process all lines and return last as Continue
                let lines: Vec<&str> = text.lines().collect();
                if lines.is_empty() {
                    return Ok(ActionResult::Continue(message));
                }

                // Just return the text as-is for non-stream
                return Ok(ActionResult::Continue(message));
            }
            return Err(anyhow!("foreach_line requires a stream or text field"));
        };

        // Try to read one line
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = {
                let mut reader = stream_reader.lock().await;
                reader.read_line(&mut line).await?
            };

            if bytes_read == 0 {
                // End of stream
                debug!("foreach_line: stream exhausted");
                return Ok(ActionResult::Continue(message));
            }

            let line_content = if self.trim { line.trim() } else { &line };

            if self.skip_empty && line_content.is_empty() {
                continue; // Skip empty lines, read next
            }

            debug!("foreach_line: yielding line: {:?}", line_content);

            // Create frame message
            let mut frame = Message::new();
            frame.source_inbound = message.source_inbound.clone();
            frame
                .metadata_mut()
                .set_str("text", line_content.to_string());

            // Put stream back into source message for next iteration
            message.payload = crate::message::MessagePayload::Stream(stream_reader);

            return Ok(ActionResult::YieldFrame {
                frame,
                source: message,
            });
        }
    }
}

/// length_prefix action - reads length-prefixed frames from stream
pub struct LengthPrefixAction {
    /// Number of bytes for length prefix (1, 2, or 4)
    length_bytes: u8,
    /// Endianness (big or little)
    big_endian: bool,
    /// Maximum frame size (for safety)
    max_frame_size: usize,
}

impl LengthPrefixAction {
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let length_bytes = params
            .get("length_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(4) as u8;

        if length_bytes != 1 && length_bytes != 2 && length_bytes != 4 {
            return Err(anyhow!("length_bytes must be 1, 2, or 4"));
        }

        let big_endian = params
            .get("endian")
            .and_then(|v| v.as_str())
            .map(|s| s == "big")
            .unwrap_or(true);

        let max_frame_size = params
            .get("max_frame_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(10 * 1024 * 1024) as usize;

        Ok(Self {
            length_bytes,
            big_endian,
            max_frame_size,
        })
    }

    fn read_length(&self, bytes: &[u8]) -> usize {
        match self.length_bytes {
            1 => bytes[0] as usize,
            2 => {
                if self.big_endian {
                    u16::from_be_bytes([bytes[0], bytes[1]]) as usize
                } else {
                    u16::from_le_bytes([bytes[0], bytes[1]]) as usize
                }
            }
            4 => {
                if self.big_endian {
                    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize
                } else {
                    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize
                }
            }
            _ => 0,
        }
    }
}

#[async_trait]
impl Action for LengthPrefixAction {
    fn name(&self) -> &str {
        "length_prefix"
    }

    fn is_final(&self) -> bool {
        false
    }

    async fn execute(&self, mut message: Message) -> Result<ActionResult> {
        use tokio::io::AsyncReadExt;

        let Some(stream_reader) = message.take_stream() else {
            return Err(anyhow!("length_prefix requires a stream message"));
        };

        // Read length prefix
        let mut length_buf = vec![0u8; self.length_bytes as usize];
        let read_result = {
            let mut reader = stream_reader.lock().await;
            reader.read_exact(&mut length_buf).await
        };

        match read_result {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                info!("length_prefix: stream exhausted");
                return Ok(ActionResult::Continue(message));
            }
            Err(e) => return Err(e.into()),
        }

        let frame_length = self.read_length(&length_buf);

        if frame_length > self.max_frame_size {
            return Err(anyhow!(
                "Frame size {} exceeds maximum {}",
                frame_length,
                self.max_frame_size
            ));
        }

        // Read frame data
        let mut frame_buf = vec![0u8; frame_length];
        {
            let mut reader = stream_reader.lock().await;
            reader.read_exact(&mut frame_buf).await?;
        }

        debug!("length_prefix: yielding frame of {} bytes", frame_length);

        // Create frame message
        let mut frame = Message::new();
        frame.source_inbound = message.source_inbound.clone();
        frame.raw = Some(frame_buf.clone());
        frame
            .metadata_mut()
            .set_str("data", String::from_utf8_lossy(&frame_buf).to_string())
            .set_str("frame_length", frame_length.to_string());

        // Put stream back into source message
        message.payload = crate::message::MessagePayload::Stream(stream_reader);

        Ok(ActionResult::YieldFrame {
            frame,
            source: message,
        })
    }
}
