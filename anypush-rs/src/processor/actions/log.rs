//! Log action - logs message details

use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_yaml::Value;
use std::collections::HashMap;
use tracing::{debug, info, warn};

pub struct LogAction {
    level: String,
}

impl LogAction {
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let level = params
            .get("level")
            .and_then(|v| v.as_str())
            .unwrap_or("info")
            .to_string();
        Ok(Self { level })
    }
}

#[async_trait]
impl Action for LogAction {
    fn name(&self) -> &str {
        "log"
    }

    fn is_final(&self) -> bool {
        false
    }

    async fn execute(&self, message: Message) -> Result<ActionResult> {
        match self.level.as_str() {
            "debug" => debug!(
                "Message {}: from={}, subject={}",
                message.id, message.from, message.subject
            ),
            "warn" => warn!(
                "Message {}: from={}, subject={}",
                message.id, message.from, message.subject
            ),
            _ => info!(
                "Message {}: from={}, subject={}",
                message.id, message.from, message.subject
            ),
        }
        Ok(ActionResult::Continue(message))
    }
}
