use crate::config::TcpOutboundConfig;
use crate::message::{Message, StreamReader};
use crate::outbound::{OutboundHandler, RouteParams};
use crate::template;
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

    fn support_route_params(&self) -> &'static [&'static str] {
        &["body", "data"]
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn send(&self, message: &Message, params: &RouteParams) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        debug!(
            "Sending message {} to TCP '{}' at {}",
            message.id, self.config.name, addr
        );

        let mut stream = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Failed to connect to {}", addr))?;

        // Data: route params override -> config template -> default text field
        let data_template = params
            .get_str("body")
            .or_else(|| params.get_str("data"))
            .or_else(|| self.config.data.clone());

        let data = if let Some(tpl) = data_template {
            template::render(&tpl, message)
        } else {
            message.metadata().get_str("text").unwrap_or_default()
        };

        stream
            .write_all(data.as_bytes())
            .await
            .with_context(|| "Failed to write to TCP stream")?;

        stream.flush().await?;

        info!(
            "Message {} sent to '{}' (TCP)",
            message.id, self.config.name
        );
        Ok(())
    }

    async fn send_stream(&self, reader: StreamReader, _params: &RouteParams) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        debug!(
            "Stream passthrough to TCP '{}' at {}",
            self.config.name, addr
        );

        let mut stream = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Failed to connect to {}", addr))?;

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
