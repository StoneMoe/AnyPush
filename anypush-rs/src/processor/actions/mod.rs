//! Action implementations for the message processing pipeline

mod base64;
mod drop;
mod field;
mod framing;
mod log;
mod noop;
mod reject;
mod route;

pub use base64::{Base64DecodeAction, Base64EncodeAction};
pub use drop::DropAction;
pub use field::{DeleteFieldAction, SetFieldAction};
pub use framing::{ForeachLineAction, LengthPrefixAction};
pub use log::LogAction;
pub use noop::NoOpAction;
pub use reject::RejectAction;
pub use route::RouteAction;

use crate::processor::Action;
use anyhow::{anyhow, Result};
use serde_yaml::Value;
use std::collections::HashMap;
use tracing::warn;

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

        // Framing actions
        "foreach_line" => Ok(Box::new(ForeachLineAction::from_params(params)?)),
        "length_prefix" => Ok(Box::new(LengthPrefixAction::from_params(params)?)),

        // Crypto actions (stubs)
        "aes128cbc_encrypt" | "aes128cbc_decrypt" | "aes256cbc_encrypt" | "aes256cbc_decrypt" => {
            warn!("Crypto action '{}' not yet implemented", action_type);
            Ok(Box::new(NoOpAction::new(action_type)))
        }
        "rsa_encrypt" | "rsa_decrypt" => {
            warn!("RSA action '{}' not yet implemented", action_type);
            Ok(Box::new(NoOpAction::new(action_type)))
        }

        _ => Err(anyhow!("Unknown action type: {}", action_type)),
    }
}
