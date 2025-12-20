use crate::config::TcpOutboundConfig;
use crate::message::{Message, StreamReader};
use crate::outbound::OutboundHandler;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, info};

/// TCP outbound handler for raw TCP connections
pub struct TcpOutbound {
    config: TcpOutboundConfig,
}

impl TcpOutbound {
    pub fn new(config: TcpOutboundConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl OutboundHandler for TcpOutbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn send(&self, message: &Message) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        debug!(
            "Sending message {} to TCP '{}' at {}",
            message.id, self.config.name, addr
        );

        let mut stream = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Failed to connect to {}", addr))?;

        // Send message text as raw bytes
        let text = message.metadata().get_str("text").unwrap_or_default();
        stream
            .write_all(text.as_bytes())
            .await
            .with_context(|| "Failed to write to TCP stream")?;

        stream.flush().await?;

        info!(
            "Message {} sent to '{}' (TCP)",
            message.id, self.config.name
        );
        Ok(())
    }

    async fn send_stream(&self, reader: StreamReader) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        debug!(
            "Stream passthrough to TCP '{}' at {}",
            self.config.name, addr
        );

        let mut stream = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Failed to connect to {}", addr))?;

        // Read from input stream and write to output stream
        let mut buf = [0u8; 8192];
        loop {
            let n = {
                let mut reader_guard = reader.lock().await;
                reader_guard.read(&mut buf).await?
            };

            if n == 0 {
                break;
            }

            stream.write_all(&buf[..n]).await?;
        }

        stream.flush().await?;

        info!("Stream passthrough to '{}' completed", self.config.name);
        Ok(())
    }
}
