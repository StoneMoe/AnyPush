use crate::config::HttpOutboundConfig;
use crate::message::Message;
use crate::outbound::{OutboundHandler, RouteParams};
use crate::template;
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

    fn support_route_params(&self) -> &'static [&'static str] {
        &["body", "content_type", "headers"]
    }

    async fn send(&self, message: &Message, params: &RouteParams) -> Result<()> {
        debug!(
            "Sending message {} to HTTP '{}'",
            message.id, self.config.name
        );

        // URL from config
        let url = &self.config.url;

        let mut request = match self.config.method.to_uppercase().as_str() {
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            method => {
                return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method));
            }
        };

        // Add headers from config
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // Add headers from params
        for (key, value) in params.get_map("headers") {
            request = request.header(key, template::render(&value, message));
        }

        // Body: route params override -> config template -> empty
        let body_template = params.get_str("body").or_else(|| self.config.body.clone());

        let body = body_template
            .map(|tpl| template::render(&tpl, message))
            .unwrap_or_default();

        let content_type = params
            .get_str("content_type")
            .or_else(|| self.config.content_type.clone())
            .unwrap_or_else(|| "text/plain".to_string());

        let response = request
            .header("Content-Type", content_type)
            .body(body)
            .send()
            .await
            .with_context(|| format!("Failed to send to HTTP: {}", url))?;

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
