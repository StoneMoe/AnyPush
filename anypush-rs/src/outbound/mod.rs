pub mod http;
pub mod s3;
pub mod tcp;

use crate::message::StreamReader;
use anyhow::{anyhow, Result};
use async_trait::async_trait;

/// Trait for outbound handlers that send messages
#[async_trait]
pub trait OutboundHandler: Send + Sync {
    /// Get the name of this handler
    fn name(&self) -> &str;

    /// Whether this outbound supports stream passthrough
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Send a buffered message to the outbound destination
    async fn send(&self, message: &crate::message::Message) -> Result<()>;

    /// Send a stream directly to the outbound (for passthrough)
    /// Default implementation returns an error
    async fn send_stream(&self, _reader: StreamReader) -> Result<()> {
        Err(anyhow!("Stream passthrough not supported by this outbound"))
    }
}
