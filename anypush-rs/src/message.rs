use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Message structure passed through the processing pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: String,

    /// Sender address/identifier
    pub from: String,

    /// Recipient address(es)
    pub to: Vec<String>,

    /// Message subject (for email) or empty
    pub subject: String,

    /// Message body/payload
    pub text: String,

    /// Timestamp when message was received
    pub timestamp: DateTime<Utc>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Name of the inbound handler that received this message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_inbound: Option<String>,
}

impl Message {
    /// Create a new message with generated ID and current timestamp
    pub fn new(from: String, to: Vec<String>, subject: String, text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from,
            to,
            subject,
            text,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            source_inbound: None,
        }
    }

    /// Set the source inbound handler name
    pub fn with_source(mut self, inbound_name: &str) -> Self {
        self.source_inbound = Some(inbound_name.to_string());
        self
    }

    /// Get a field value by name (for expression evaluation)
    pub fn get_field(&self, field: &str) -> Option<String> {
        match field {
            "id" => Some(self.id.clone()),
            "from" => Some(self.from.clone()),
            "to" => Some(self.to.join(", ")),
            "subject" => Some(self.subject.clone()),
            "text" => Some(self.text.clone()),
            "timestamp" => Some(self.timestamp.to_rfc3339()),
            _ if field.starts_with("metadata.") => {
                let key = &field[9..];
                self.metadata.get(key).cloned()
            }
            _ => None,
        }
    }

    /// Set a field value by name
    pub fn set_field(&mut self, field: &str, value: String) -> bool {
        match field {
            "from" => {
                self.from = value;
                true
            }
            "subject" => {
                self.subject = value;
                true
            }
            "text" => {
                self.text = value;
                true
            }
            _ if field.starts_with("metadata.") => {
                let key = &field[9..];
                self.metadata.insert(key.to_string(), value);
                true
            }
            _ => false, // Cannot modify id, to, timestamp
        }
    }

    /// Delete a field (only metadata fields can be deleted)
    pub fn delete_field(&mut self, field: &str) -> bool {
        if field.starts_with("metadata.") {
            let key = &field[9..];
            self.metadata.remove(key).is_some()
        } else {
            false
        }
    }
}
