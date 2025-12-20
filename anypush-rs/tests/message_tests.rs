//! Tests for Message structure and operations

use anypush::message::{Message, MessagePayload};
use std::io::Cursor;
use tokio::io::BufReader;

#[test]
fn test_message_new_buffered() {
    let msg = Message::new();
    assert!(matches!(msg.payload, MessagePayload::Buffered));
    assert!(!msg.id.is_empty());
}

#[test]
fn test_message_email() {
    let msg = Message::email(
        "sender@test.com".to_string(),
        vec!["recipient@test.com".to_string()],
        "Test Subject".to_string(),
        "Test body".to_string(),
    );

    assert_eq!(
        msg.metadata().get_str("smtp.from"),
        Some("sender@test.com".to_string())
    );
    assert_eq!(
        msg.metadata().get_str("smtp.subject"),
        Some("Test Subject".to_string())
    );
    assert_eq!(
        msg.metadata().get_str("text"),
        Some("Test body".to_string())
    );
}

#[test]
fn test_message_with_raw() {
    let msg = Message::new_with_raw(b"raw data".to_vec());

    assert_eq!(msg.raw_as_str(), Some("raw data".to_string()));
    assert!(msg.raw.is_some());
}

#[test]
fn test_message_set_get_str() {
    let mut msg = Message::new();
    msg.metadata_mut().set_str("key1", "value1");
    msg.metadata_mut().set_str("key2", "value2");

    assert_eq!(msg.metadata().get_str("key1"), Some("value1".to_string()));
    assert_eq!(msg.metadata().get_str("key2"), Some("value2".to_string()));
    assert_eq!(msg.metadata().get_str("nonexistent"), None);
}

#[test]
fn test_message_set_array() {
    let mut msg = Message::new();
    msg.metadata_mut().set_array(
        "recipients",
        vec!["a@test.com".to_string(), "b@test.com".to_string()],
    );

    let metadata = msg.metadata();
    let recipients = metadata.get("recipients");
    assert!(recipients.is_some());

    if let Some(serde_json::Value::Array(arr)) = recipients {
        assert_eq!(arr.len(), 2);
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_message_with_source() {
    let msg = Message::new().with_source("http-handler");
    assert_eq!(msg.source_inbound, Some("http-handler".to_string()));
}

#[test]
fn test_message_get_field_legacy() {
    let msg = Message::email(
        "test@test.com".to_string(),
        vec![],
        "Subject".to_string(),
        "Body".to_string(),
    );

    assert_eq!(
        msg.get_field("smtp.from"),
        Some("test@test.com".to_string())
    );
    assert_eq!(msg.get_field("smtp.subject"), Some("Subject".to_string()));
    assert!(msg.get_field("id").is_some());
    assert!(msg.get_field("timestamp").is_some());
}

#[test]
fn test_message_set_field_legacy() {
    let mut msg = Message::new();

    assert!(msg.set_field("custom", "value".to_string()));
    assert_eq!(msg.get_field("custom"), Some("value".to_string()));

    // Cannot modify id or timestamp
    assert!(!msg.set_field("id", "new_id".to_string()));
    assert!(!msg.set_field("timestamp", "new_time".to_string()));
}

#[test]
fn test_message_delete_field() {
    let mut msg = Message::new();
    msg.metadata_mut().set_str("key", "value");

    assert!(msg.delete_field("key"));
    assert_eq!(msg.metadata().get_str("key"), None);

    // Deleting nonexistent field returns false
    assert!(!msg.delete_field("nonexistent"));
}

#[tokio::test]
async fn test_message_stream() {
    let data = "line1\nline2\nline3\n";
    let cursor = Cursor::new(data.as_bytes().to_vec());
    let reader = BufReader::new(cursor);

    let mut msg = Message::new_stream(reader);
    assert!(matches!(msg.payload, MessagePayload::Stream(_)));

    // Take stream should return Some
    let stream = msg.take_stream();
    assert!(stream.is_some());

    // After taking, payload becomes Buffered
    assert!(matches!(msg.payload, MessagePayload::Buffered));

    // Take again should return None
    assert!(msg.take_stream().is_none());
}

#[test]
fn test_message_serialization() {
    let msg = Message::email(
        "sender@test.com".to_string(),
        vec!["recipient@test.com".to_string()],
        "Test".to_string(),
        "Body".to_string(),
    );

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("sender@test.com"));
}

#[test]
fn test_message_raw_as_base64() {
    let msg = Message::new_with_raw(b"hello".to_vec());
    assert_eq!(msg.raw_as_base64(), Some("aGVsbG8=".to_string()));
}
