//! S3 outbound handler for writing messages to S3 buckets

use crate::config::S3OutboundConfig;
use crate::message::Message;
use crate::outbound::{OutboundHandler, RouteParams};
use crate::template;
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
        let mut aws_config_builder = aws_config::from_env();

        if let Some(ref region) = config.region {
            aws_config_builder =
                aws_config_builder.region(aws_sdk_s3::config::Region::new(region.clone()));
        }

        let aws_config = aws_config_builder.load().await;

        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&aws_config);

        if let Some(ref endpoint) = config.endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(endpoint);
        }

        let client = Client::from_conf(s3_config_builder.build());

        Ok(Self { config, client })
    }
}

#[async_trait]
impl OutboundHandler for S3Outbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn support_route_params(&self) -> &'static [&'static str] {
        &["body", "key", "content_type"]
    }

    async fn send(&self, message: &Message, params: &RouteParams) -> Result<()> {
        // Key: route params override -> config template
        let key_template = params
            .get_str("key")
            .unwrap_or_else(|| self.config.key.clone());
        let key = template::render(&key_template, message);

        debug!(
            "Uploading message {} to S3 bucket '{}' key '{}'",
            message.id, self.config.bucket, key
        );

        // Body: route params override -> config template -> default JSON
        let body_template = params.get_str("body").or_else(|| self.config.body.clone());

        let (body, content_type) = if let Some(tpl) = body_template {
            let body = template::render(&tpl, message);
            let ct = params
                .get_str("content_type")
                .or_else(|| self.config.content_type.clone())
                .unwrap_or_else(|| "text/plain".to_string());
            (body.into_bytes(), ct)
        } else {
            let json = serde_json::to_vec(message)
                .with_context(|| "Failed to serialize message to JSON")?;
            (json, "application/json".to_string())
        };

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(body.into())
            .content_type(content_type)
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
