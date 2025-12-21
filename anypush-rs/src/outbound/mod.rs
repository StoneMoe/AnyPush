pub mod http;
pub mod s3;
pub mod tcp;

use crate::message::StreamReader;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_yaml::Value;
use std::collections::HashMap;

/// Route parameters passed from route action to outbound handlers
#[derive(Debug, Clone, Default)]
pub struct RouteParams(HashMap<String, Value>);

impl RouteParams {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.0.iter()
    }

    /// Get string param value
    pub fn get_str(&self, key: &str) -> Option<String> {
        self.0
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get a string-to-string map from params by key
    pub fn get_map(&self, key: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();
        if let Some(Value::Mapping(map)) = self.0.get(key) {
            for (k, v) in map {
                if let (Value::String(key), Value::String(value)) = (k, v) {
                    result.insert(key.clone(), value.clone());
                }
            }
        }
        result
    }
}

impl std::iter::FromIterator<(String, Value)> for RouteParams {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Trait for outbound handlers that send messages
#[async_trait]
pub trait OutboundHandler: Send + Sync {
    /// Get the name of this handler
    fn name(&self) -> &str;

    /// Get valid route param keys this outbound accepts
    fn support_route_params(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether this outbound supports stream passthrough
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Send a buffered message to the outbound destination
    async fn send(&self, message: &crate::message::Message, params: &RouteParams) -> Result<()>;

    /// Send a stream directly to the outbound (for passthrough)
    /// Default implementation returns an error
    async fn send_stream(&self, _reader: StreamReader, _params: &RouteParams) -> Result<()> {
        Err(anyhow!("Stream passthrough not supported by this outbound"))
    }
}
