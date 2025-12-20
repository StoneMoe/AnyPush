//! S3 outbound handler for writing messages to S3 buckets

use crate::config::S3OutboundConfig;
use crate::message::Message;
use crate::outbound::OutboundHandler;
use anyhow::{Context, Result};
use async_trait::async_trait;
use aws_sdk_s3::Client;
use tracing::{debug, info};

/// S3 outbound handler for S3 PutObject operations
pub struct S3Outbound {
    config: S3OutboundConfig,
    client: Client,
}

impl S3Outbound {
    pub async fn new(config: S3OutboundConfig) -> Result<Self> {
        // Load AWS config
        let mut aws_config_builder = aws_config::from_env();

        // Apply region override if specified
        if let Some(ref region) = config.region {
            aws_config_builder =
                aws_config_builder.region(aws_sdk_s3::config::Region::new(region.clone()));
        }

        let aws_config = aws_config_builder.load().await;

        // Build S3 client
        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&aws_config);

        // Apply endpoint override if specified (for S3-compatible services like MinIO)
        if let Some(ref endpoint) = config.endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(endpoint);
        }

        let client = Client::from_conf(s3_config_builder.build());

        Ok(Self { config, client })
    }

    /// Render key template with message fields
    fn render_key(&self, message: &Message) -> String {
        let mut key = self.config.key.clone();
        key = key.replace("{{id}}", &message.id);
        key = key.replace(
            "{{timestamp}}",
            &message.timestamp.format("%Y%m%d_%H%M%S").to_string(),
        );

        // Replace data fields (smtp prefix for email messages)
        if let Some(from) = message.metadata().get_str("smtp.from") {
            key = key.replace("{{smtp.from}}", &from);
        }
        if let Some(subject) = message.metadata().get_str("smtp.subject") {
            key = key.replace("{{smtp.subject}}", &subject);
        }

        key
    }
}

#[async_trait]
impl OutboundHandler for S3Outbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn send(&self, message: &Message) -> Result<()> {
        let key = self.render_key(message);

        debug!(
            "Uploading message {} to S3 bucket '{}' key '{}'",
            message.id, self.config.bucket, key
        );

        // Serialize message to JSON
        let body =
            serde_json::to_vec(message).with_context(|| "Failed to serialize message to JSON")?;

        // Upload to S3
        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(body.into())
            .content_type("application/json")
            .send()
            .await
            .with_context(|| format!("Failed to upload to S3: {}/{}", self.config.bucket, key))?;

        info!(
            "Message {} uploaded to s3://{}/{}",
            message.id, self.config.bucket, key
        );
        Ok(())
    }
}
