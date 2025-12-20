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
        let source = message.source_inbound.as_deref().unwrap_or("unknown");
        let text_preview = message
            .metadata()
            .get_str("text")
            .map(|t| {
                if t.len() > 50 {
                    format!("{}...", &t[..50])
                } else {
                    t
                }
            })
            .unwrap_or_default();

        match self.level.as_str() {
            "debug" => debug!(
                "Message {}: source={}, data_fields={}, text={}",
                message.id,
                source,
                message.metadata.len(),
                text_preview
            ),
            "warn" => warn!(
                "Message {}: source={}, data_fields={}, text={}",
                message.id,
                source,
                message.metadata.len(),
                text_preview
            ),
            _ => info!(
                "Message {}: source={}, data_fields={}, text={}",
                message.id,
                source,
                message.metadata.len(),
                text_preview
            ),
        }
        Ok(ActionResult::Continue(message))
    }
}
