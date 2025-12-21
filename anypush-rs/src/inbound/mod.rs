pub mod directory;
pub mod file;
pub mod ftp;
pub mod http;
pub mod smtp;
pub mod tcp;

use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Trait for inbound handlers that receive messages
#[async_trait]
pub trait InboundHandler: Send + Sync {
    /// Get the name of this handler
    fn name(&self) -> &str;

    /// Start receiving messages and send them to the provided channel
    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()>;

    /// Stop the handler gracefully
    async fn stop(&self) -> Result<()>;
}
