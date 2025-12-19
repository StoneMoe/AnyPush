use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_yaml::Value;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Create an action from its type identifier and parameters
pub fn create_action(
    action_type: &str,
    params: &HashMap<String, Value>,
) -> Result<Box<dyn Action>> {
    match action_type {
        // Final actions
        "route" => Ok(Box::new(RouteAction::from_params(params)?)),
        "reject" => Ok(Box::new(RejectAction::from_params(params)?)),
        "drop" => Ok(Box::new(DropAction)),

        // Transformer actions
        "log" => Ok(Box::new(LogAction::from_params(params)?)),
        "base64_encode" => Ok(Box::new(Base64EncodeAction::from_params(params)?)),
        "base64_decode" => Ok(Box::new(Base64DecodeAction::from_params(params)?)),
        "set_field" => Ok(Box::new(SetFieldAction::from_params(params)?)),
        "delete_field" => Ok(Box::new(DeleteFieldAction::from_params(params)?)),

        // Crypto actions (stubs)
        "aes128cbc_encrypt" | "aes128cbc_decrypt" | "aes256cbc_encrypt" | "aes256cbc_decrypt" => {
            warn!("Crypto action '{}' not yet implemented", action_type);
            Ok(Box::new(NoOpAction::new(action_type)))
        }
        "rsa_encrypt" | "rsa_decrypt" => {
            warn!("RSA action '{}' not yet implemented", action_type);
            Ok(Box::new(NoOpAction::new(action_type)))
        }

        // Iterator actions (stubs)
        "foreach_line" => {
            warn!("foreach_line action not yet implemented");
            Ok(Box::new(NoOpAction::new(action_type)))
        }
        "len_data_pattern_bin" => {
            warn!("len_data_pattern_bin action not yet implemented");
            Ok(Box::new(NoOpAction::new(action_type)))
        }

        _ => Err(anyhow!("Unknown action type: {}", action_type)),
    }
}

// ============================================================================
// Final Actions
// ============================================================================

/// Route action - sends message to outbound handler(s)
pub struct RouteAction {
    to: Vec<String>,
}

impl RouteAction {
    fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let to = match params.get("to") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Sequence(seq)) => seq
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            None => return Err(anyhow!("route action requires 'to' parameter")),
            _ => return Err(anyhow!("'to' must be a string or list of strings")),
        };
        Ok(Self { to })
    }
}

#[async_trait]
impl Action for RouteAction {
    fn name(&self) -> &str {
        "route"
    }

    fn is_final(&self) -> bool {
        true
    }

    async fn execute(&self, message: Message) -> Result<ActionResult> {
        debug!("Routing message {} to {:?}", message.id, self.to);
        Ok(ActionResult::Route {
            to: self.to.clone(),
            message,
        })
    }
}

/// Reject action - rejects the transaction with a message
pub struct RejectAction {
    message: String,
}

impl RejectAction {
    fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
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

/// Drop action - silently drops the message
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

// ============================================================================
// Transformer Actions
// ============================================================================

/// Log action - logs message details
pub struct LogAction {
    level: String,
}

impl LogAction {
    fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
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

/// Base64 encode action
pub struct Base64EncodeAction {
    field: String,
}

impl Base64EncodeAction {
    fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let field = params
            .get("field")
            .and_then(|v| v.as_str())
            .unwrap_or("text")
            .to_string();
        Ok(Self { field })
    }
}

#[async_trait]
impl Action for Base64EncodeAction {
    fn name(&self) -> &str {
        "base64_encode"
    }

    fn is_final(&self) -> bool {
        false
    }

    async fn execute(&self, mut message: Message) -> Result<ActionResult> {
        if let Some(value) = message.get_field(&self.field) {
            let encoded = BASE64.encode(value.as_bytes());
            message.set_field(&self.field, encoded);
        }
        Ok(ActionResult::Continue(message))
    }
}

/// Base64 decode action
pub struct Base64DecodeAction {
    field: String,
}

impl Base64DecodeAction {
    fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let field = params
            .get("field")
            .and_then(|v| v.as_str())
            .unwrap_or("text")
            .to_string();
        Ok(Self { field })
    }
}

#[async_trait]
impl Action for Base64DecodeAction {
    fn name(&self) -> &str {
        "base64_decode"
    }

    fn is_final(&self) -> bool {
        false
    }

    async fn execute(&self, mut message: Message) -> Result<ActionResult> {
        if let Some(value) = message.get_field(&self.field) {
            let decoded = BASE64.decode(value.as_bytes())?;
            let decoded_str = String::from_utf8(decoded)?;
            message.set_field(&self.field, decoded_str);
        }
        Ok(ActionResult::Continue(message))
    }
}

/// Set field action
pub struct SetFieldAction {
    field: String,
    value: String,
}

impl SetFieldAction {
    fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let field = params
            .get("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("set_field requires 'field' parameter"))?
            .to_string();
        let value = params
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("set_field requires 'value' parameter"))?
            .to_string();
        Ok(Self { field, value })
    }
}

#[async_trait]
impl Action for SetFieldAction {
    fn name(&self) -> &str {
        "set_field"
    }

    fn is_final(&self) -> bool {
        false
    }

    async fn execute(&self, mut message: Message) -> Result<ActionResult> {
        // TODO: Support template substitution like {{subject}}
        message.set_field(&self.field, self.value.clone());
        Ok(ActionResult::Continue(message))
    }
}

/// Delete field action
pub struct DeleteFieldAction {
    field: String,
}

impl DeleteFieldAction {
    fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let field = params
            .get("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("delete_field requires 'field' parameter"))?
            .to_string();
        Ok(Self { field })
    }
}

#[async_trait]
impl Action for DeleteFieldAction {
    fn name(&self) -> &str {
        "delete_field"
    }

    fn is_final(&self) -> bool {
        false
    }

    async fn execute(&self, mut message: Message) -> Result<ActionResult> {
        message.delete_field(&self.field);
        Ok(ActionResult::Continue(message))
    }
}

/// No-op action for unimplemented features
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
