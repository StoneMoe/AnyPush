pub mod http;
pub mod tcp;

use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;

/// Trait for outbound handlers that send messages
#[async_trait]
pub trait OutboundHandler: Send + Sync {
    /// Get the name of this handler
    fn name(&self) -> &str;

    /// Send a message to the outbound destination
    async fn send(&self, message: &Message) -> Result<()>;
}
