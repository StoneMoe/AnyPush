//! Route action - sends message to outbound handler(s)

use crate::message::Message;
use crate::outbound::RouteParams;
use crate::processor::{Action, ActionResult};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_yaml::Value;
use std::collections::HashMap;
use tracing::debug;

pub struct RouteAction {
    to: Vec<String>,
    params: RouteParams,
}

impl RouteAction {
    pub fn from_params(params: &HashMap<String, Value>) -> Result<Self> {
        let to = match params.get("to") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Sequence(seq)) => seq
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            None => return Err(anyhow!("route action requires 'to' parameter")),
            _ => return Err(anyhow!("'to' must be a string or list of strings")),
        };

        // Pass all route params (except control keys), filtering by valid keys happens in router
        let mut route_params = RouteParams::new();
        for (key, value) in params {
            if key != "action" && key != "to" && key != "when" {
                route_params.insert(key.clone(), value.clone());
            }
        }

        Ok(Self {
            to,
            params: route_params,
        })
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
            params: self.params.clone(),
        })
    }
}
