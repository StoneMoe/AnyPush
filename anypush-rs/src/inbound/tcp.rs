use crate::config::{ConnectionMode, TcpInboundConfig};
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// TCP inbound handler for raw TCP connections
pub struct TcpInbound {
    config: TcpInboundConfig,
}

impl TcpInbound {
    pub fn new(config: TcpInboundConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InboundHandler for TcpInbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;
        info!(
            "TCP handler '{}' listening on {} (mode: {:?})",
            self.config.name, addr, self.config.connection_mode
        );

        let name = self.config.name.clone();
        let buffer_max = self.config.buffer_max_bytes;

        loop {
            match listener.accept().await {
                Ok((mut stream, peer)) => {
                    debug!("TCP connection from {:?}", peer);
                    let sender = sender.clone();
                    let handler_name = name.clone();

                    tokio::spawn(async move {
                        // Read data from connection
                        let mut buffer = vec![0u8; buffer_max.min(65536)];
                        match stream.read(&mut buffer).await {
                            Ok(0) => {
                                debug!("TCP connection closed");
                            }
                            Ok(n) => {
                                let data = String::from_utf8_lossy(&buffer[..n]).to_string();

                                let message =
                                    Message::new(peer.to_string(), vec![], String::new(), data)
                                        .with_source(&handler_name);

                                if let Err(e) = sender.send(message).await {
                                    error!("Failed to send message to router: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("TCP read error: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept TCP connection: {}", e);
                }
            }
        }
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping TCP handler '{}'", self.config.name);
        Ok(())
    }
}
