use crate::config::TcpOutboundConfig;
use crate::message::Message;
use crate::outbound::OutboundHandler;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
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
        stream
            .write_all(message.text.as_bytes())
            .await
            .with_context(|| "Failed to write to TCP stream")?;

        stream.flush().await?;

        info!(
            "Message {} sent to '{}' (TCP)",
            message.id, self.config.name
        );
        Ok(())
    }
}
