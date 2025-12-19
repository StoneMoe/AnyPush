use crate::config::HttpOutboundConfig;
use crate::message::Message;
use crate::outbound::OutboundHandler;
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, info};

/// HTTP outbound handler for HTTP POST requests
pub struct HttpOutbound {
    config: HttpOutboundConfig,
    client: Client,
}

impl HttpOutbound {
    pub fn new(config: HttpOutboundConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl OutboundHandler for HttpOutbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn send(&self, message: &Message) -> Result<()> {
        debug!(
            "Sending message {} to HTTP '{}'",
            message.id, self.config.name
        );

        let mut request = match self.config.method.to_uppercase().as_str() {
            "POST" => self.client.post(&self.config.url),
            "PUT" => self.client.put(&self.config.url),
            method => {
                return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method));
            }
        };

        // Add headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // Send JSON payload
        let response = request
            .json(message)
            .send()
            .await
            .with_context(|| format!("Failed to send to HTTP: {}", self.config.url))?;

        if response.status().is_success() {
            info!(
                "Message {} sent to '{}' successfully",
                message.id, self.config.name
            );
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "HTTP returned error status: {}",
                response.status()
            ))
        }
    }
}
