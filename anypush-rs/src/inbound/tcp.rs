use crate::config::TcpInboundConfig;
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use tokio::io::BufReader;
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
        info!("TCP inbound '{}' listening on {}", self.config.name, addr);

        let name = self.config.name.clone();

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    debug!("TCP connection from {:?}", peer);
                    let sender = sender.clone();
                    let handler_name = name.clone();

                    tokio::spawn(async move {
                        let reader = BufReader::new(stream);
                        let mut message = Message::new_stream(reader).with_source(&handler_name);
                        message.metadata_mut().set_str("tcp.peer", peer.to_string());

                        info!("TCP stream message: {} from {}", message.id, peer);

                        if let Err(e) = sender.send(message).await {
                            error!("Failed to send message to router: {}", e);
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
        info!("Stopping TCP inbound '{}'", self.config.name);
        Ok(())
    }
}
