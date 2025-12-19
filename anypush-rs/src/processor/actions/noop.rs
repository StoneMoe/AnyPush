//! No-op action for unimplemented features

use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::Result;
use async_trait::async_trait;
use tracing::warn;

pub struct NoOpAction {
    name: String,
}

impl NoOpAction {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl Action for NoOpAction {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_final(&self) -> bool {
        false
    }

    async fn execute(&self, message: Message) -> Result<ActionResult> {
        warn!("Action '{}' is a no-op (not implemented)", self.name);
        Ok(ActionResult::Continue(message))
    }
}
