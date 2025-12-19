//! Drop action - silently drops the message

use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::Result;
use async_trait::async_trait;
use tracing::debug;

pub struct DropAction;

#[async_trait]
impl Action for DropAction {
    fn name(&self) -> &str {
        "drop"
    }

    fn is_final(&self) -> bool {
        true
    }

    async fn execute(&self, message: Message) -> Result<ActionResult> {
        debug!("Dropping message {}", message.id);
        Ok(ActionResult::Drop)
    }
}
