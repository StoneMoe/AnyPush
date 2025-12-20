use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncBufRead;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Stream holder - wrapped in Arc<Mutex> so Message can be Sync
pub type StreamReader = Arc<Mutex<Pin<Box<dyn AsyncBufRead + Send>>>>;

/// Message payload - can be buffered data or a live stream
#[derive(Clone)]
pub enum MessagePayload {
    /// Buffered data - fully loaded in memory
    Buffered,
    /// Stream - live connection for streaming processing (wrapped for Sync)
    Stream(StreamReader),
}

impl std::fmt::Debug for MessagePayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessagePayload::Buffered => write!(f, "Buffered"),
            MessagePayload::Stream(_) => write!(f, "Stream(...)"),
        }
    }
}

/// Message structure passed through the processing pipeline
#[derive(Debug, Clone)]
pub struct Message {
    /// Unique message ID
    pub id: String,

    /// Timestamp when message was received
    pub timestamp: DateTime<Utc>,

    /// Name of the inbound handler that received this message
    pub source_inbound: Option<String>,

    /// Raw bytes from inbound (original payload, immutable)
    pub raw: Option<Vec<u8>>,

    /// Parsed/intermediate data fields (mutable, actions can add/modify)
    pub metadata: HashMap<String, JsonValue>,

    /// Payload type (buffered or stream)
    pub payload: MessagePayload,
}

/// Serializable version of Message (without stream)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMessage {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_inbound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_base64: Option<String>,
    pub metadata: HashMap<String, JsonValue>,
}

impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        SerializableMessage {
            id: self.id.clone(),
            timestamp: self.timestamp,
            source_inbound: self.source_inbound.clone(),
            raw_base64: self
                .raw
                .as_ref()
                .map(|r| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, r)),
            metadata: self.metadata.clone(),
        }
        .serialize(serializer)
    }
}

impl Message {
    /// Create a new buffered message
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            source_inbound: None,
            raw: None,
            metadata: HashMap::new(),
            payload: MessagePayload::Buffered,
        }
    }

    /// Create a new message with raw data
    pub fn new_with_raw(raw_data: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            source_inbound: None,
            raw: Some(raw_data),
            metadata: HashMap::new(),
            payload: MessagePayload::Buffered,
        }
    }

    /// Create a new stream message
    pub fn new_stream(reader: impl AsyncBufRead + Send + 'static) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            source_inbound: None,
            raw: None,
            metadata: HashMap::new(),
            payload: MessagePayload::Stream(Arc::new(Mutex::new(Box::pin(reader)))),
        }
    }

    /// Create an email message (convenience, uses smtp. prefix)
    pub fn email(from: String, to: Vec<String>, subject: String, text: String) -> Self {
        let mut msg = Self::new();
        msg.metadata_mut()
            .set_str("smtp.from", from)
            .set_array("smtp.to", to)
            .set_str("smtp.subject", subject)
            .set_str("text", text); // text is generic for body content
        msg
    }

    /// Set the source inbound handler name
    pub fn with_source(mut self, inbound_name: &str) -> Self {
        self.source_inbound = Some(inbound_name.to_string());
        self
    }

    /// Get raw bytes as string (UTF-8 lossy)
    pub fn raw_as_str(&self) -> Option<String> {
        self.raw
            .as_ref()
            .map(|r| String::from_utf8_lossy(r).to_string())
    }

    /// Get raw bytes as base64
    pub fn raw_as_base64(&self) -> Option<String> {
        self.raw
            .as_ref()
            .map(|r| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, r))
    }

    /// Get immutable access to metadata
    pub fn metadata(&self) -> MetadataRef<'_> {
        MetadataRef(&self.metadata)
    }

    /// Get mutable access to metadata
    pub fn metadata_mut(&mut self) -> MetadataMut<'_> {
        MetadataMut(&mut self.metadata)
    }

    /// Check if this is a stream message
    pub fn is_stream(&self) -> bool {
        matches!(self.payload, MessagePayload::Stream(_))
    }

    /// Take the stream out of this message (consumes the stream)
    pub fn take_stream(&mut self) -> Option<StreamReader> {
        match std::mem::replace(&mut self.payload, MessagePayload::Buffered) {
            MessagePayload::Stream(reader) => Some(reader),
            MessagePayload::Buffered => None,
        }
    }

    /// Get a field value by name (for expression evaluation)
    pub fn get_field(&self, field: &str) -> Option<String> {
        match field {
            "id" => Some(self.id.clone()),
            "timestamp" => Some(self.timestamp.to_rfc3339()),
            "source" => self.source_inbound.clone(),
            "raw" => self.raw_as_str(),
            "raw_base64" => self.raw_as_base64(),
            _ => self.metadata().get_str(field),
        }
    }

    /// Set a field value by name
    pub fn set_field(&mut self, field: &str, value: String) -> bool {
        match field {
            "id" | "timestamp" | "source" | "raw" => false, // Cannot modify
            _ => {
                self.metadata_mut().set_str(field, value);
                true
            }
        }
    }

    /// Delete a field
    pub fn delete_field(&mut self, field: &str) -> bool {
        self.metadata.remove(field).is_some()
    }
}

impl Default for Message {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable reference to message metadata
pub struct MetadataRef<'a>(&'a HashMap<String, JsonValue>);

impl<'a> MetadataRef<'a> {
    /// Get a field as string
    pub fn get_str(&self, key: &str) -> Option<String> {
        self.0.get(key).and_then(|v| match v {
            JsonValue::String(s) => Some(s.clone()),
            _ => Some(v.to_string()),
        })
    }

    /// Get a field as JSON value
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.0.get(key)
    }

    /// Check if a field exists
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Get number of fields
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over all fields
    pub fn iter(&self) -> impl Iterator<Item = (&String, &JsonValue)> {
        self.0.iter()
    }
}

/// Mutable reference to message metadata  
pub struct MetadataMut<'a>(&'a mut HashMap<String, JsonValue>);

impl<'a> MetadataMut<'a> {
    /// Set a string field
    pub fn set_str(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.0.insert(key.into(), JsonValue::String(value.into()));
        self
    }

    /// Set a JSON value field
    pub fn set(&mut self, key: impl Into<String>, value: JsonValue) -> &mut Self {
        self.0.insert(key.into(), value);
        self
    }

    /// Set an array of strings
    pub fn set_array(&mut self, key: impl Into<String>, values: Vec<String>) -> &mut Self {
        self.0.insert(
            key.into(),
            JsonValue::Array(values.into_iter().map(JsonValue::String).collect()),
        );
        self
    }

    /// Delete a field
    pub fn delete(&mut self, key: &str) -> bool {
        self.0.remove(key).is_some()
    }

    /// Insert directly into the HashMap
    pub fn insert(&mut self, key: String, value: JsonValue) -> &mut Self {
        self.0.insert(key, value);
        self
    }

    /// Get a field as string (also available on mutable ref)
    pub fn get_str(&self, key: &str) -> Option<String> {
        self.0.get(key).and_then(|v| match v {
            JsonValue::String(s) => Some(s.clone()),
            _ => Some(v.to_string()),
        })
    }

    /// Get a field as JSON value
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.0.get(key)
    }

    /// Get number of fields
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
