//! Base64 encode/decode actions

use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_yaml::Value;
use std::collections::HashMap;

/// Base64 encode action
pub struct Base64EncodeAction {
    field: String,
}

impl Base64EncodeAction {
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
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
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
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
