//! Reject action - rejects the transaction with a message

use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::Result;
use async_trait::async_trait;
use serde_yaml::Value;
use std::collections::HashMap;
use tracing::info;

pub struct RejectAction {
    message: String,
}

impl RejectAction {
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Rejected")
            .to_string();
        Ok(Self { message })
    }
}

#[async_trait]
impl Action for RejectAction {
    fn name(&self) -> &str {
        "reject"
    }

    fn is_final(&self) -> bool {
        true
    }

    async fn execute(&self, message: Message) -> Result<ActionResult> {
        info!("Rejecting message {}: {}", message.id, self.message);
        Ok(ActionResult::Reject {
            message: self.message.clone(),
        })
    }
}
