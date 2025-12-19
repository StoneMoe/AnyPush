//! Field manipulation actions (set_field, delete_field)

use crate::message::Message;
use crate::processor::{Action, ActionResult};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_yaml::Value;
use std::collections::HashMap;

/// Set field action
pub struct SetFieldAction {
    field: String,
    value: String,
}

impl SetFieldAction {
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
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
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
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
