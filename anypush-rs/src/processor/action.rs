//! Action result and trait definitions

use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;

/// Result of executing an action
#[derive(Debug)]
pub enum ActionResult {
    /// Continue to next step with (possibly modified) message
    Continue(Message),
    /// Stop pipeline, route to outbound handler(s)
    Route { to: Vec<String>, message: Message },
    /// Stop pipeline, reject the transaction
    Reject { message: String },
    /// Stop pipeline, silently drop the message
    Drop,
    /// Yield a frame and continue processing stream
    /// Used by framing actions (foreach_line, length_prefix)
    /// - frame: The framed message to process through remaining pipeline steps
    /// - source: The original message with stream for next frame
    YieldFrame { frame: Message, source: Message },
}

/// Request to route a message to outbound handlers
#[derive(Debug)]
pub struct RouteRequest {
    pub targets: Vec<String>,
    pub message: Message,
}

/// Trait for pipeline actions
#[async_trait]
pub trait Action: Send + Sync {
    /// Get the action name/type
    fn name(&self) -> &str;

    /// Whether this is a final action (terminates pipeline)
    fn is_final(&self) -> bool;

    /// Execute the action on the message
    async fn execute(&self, message: Message) -> Result<ActionResult>;
}
