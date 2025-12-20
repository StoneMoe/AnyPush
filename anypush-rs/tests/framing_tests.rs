//! Tests for framing actions (foreach_line, length_prefix)

use anypush::message::Message;
use anypush::processor::actions::{ForeachLineAction, LengthPrefixAction};
use anypush::processor::{Action, ActionResult};
use serde_yaml::Value;
use std::collections::HashMap;
use std::io::Cursor;
use tokio::io::BufReader;

#[tokio::test]
async fn test_foreach_line_buffered_text() {
    let action = ForeachLineAction::from_params(&HashMap::new()).unwrap();

    let mut msg = Message::new();
    msg.metadata_mut().set_str("text", "line1\nline2\nline3");

    // For buffered text without stream, it just returns Continue
    let result = action.execute(msg).await.unwrap();
    assert!(matches!(result, ActionResult::Continue(_)));
}

#[tokio::test]
async fn test_foreach_line_stream() {
    let action = ForeachLineAction::from_params(&HashMap::new()).unwrap();

    let data = "line1\nline2\nline3\n";
    let cursor = Cursor::new(data.as_bytes().to_vec());
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    // Should yield first frame
    let result = action.execute(msg).await.unwrap();

    match result {
        ActionResult::YieldFrame { frame, source } => {
            assert_eq!(frame.metadata().get_str("text"), Some("line1".to_string()));
            // Source should still have stream for next frame
            assert!(matches!(
                source.payload,
                anypush::message::MessagePayload::Stream(_)
            ));
        }
        _ => panic!("Expected YieldFrame, got {:?}", result),
    }
}

#[tokio::test]
async fn test_foreach_line_skip_empty() {
    let mut params = HashMap::new();
    params.insert("skip_empty".to_string(), Value::Bool(true));

    let action = ForeachLineAction::from_params(&params).unwrap();

    let data = "\n\nline1\n\n";
    let cursor = Cursor::new(data.as_bytes().to_vec());
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    // Should skip empty lines and yield "line1"
    let result = action.execute(msg).await.unwrap();

    match result {
        ActionResult::YieldFrame { frame, .. } => {
            assert_eq!(frame.metadata().get_str("text"), Some("line1".to_string()));
        }
        _ => panic!("Expected YieldFrame"),
    }
}

#[tokio::test]
async fn test_foreach_line_no_skip_empty() {
    let mut params = HashMap::new();
    params.insert("skip_empty".to_string(), Value::Bool(false));

    let action = ForeachLineAction::from_params(&params).unwrap();

    let data = "\nline1";
    let cursor = Cursor::new(data.as_bytes().to_vec());
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    // Should yield empty line first
    let result = action.execute(msg).await.unwrap();

    match result {
        ActionResult::YieldFrame { frame, .. } => {
            // First yield is empty line (after trim it's "")
            assert_eq!(frame.metadata().get_str("text"), Some("".to_string()));
        }
        _ => panic!("Expected YieldFrame"),
    }
}

#[tokio::test]
async fn test_foreach_line_no_trim() {
    let mut params = HashMap::new();
    params.insert("trim".to_string(), Value::Bool(false));

    let action = ForeachLineAction::from_params(&params).unwrap();

    let data = "  line1  \n";
    let cursor = Cursor::new(data.as_bytes().to_vec());
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    let result = action.execute(msg).await.unwrap();

    match result {
        ActionResult::YieldFrame { frame, .. } => {
            // Should preserve whitespace (but newline is stripped by read_line)
            let text = frame.metadata().get_str("text").unwrap();
            assert!(text.starts_with("  "));
        }
        _ => panic!("Expected YieldFrame"),
    }
}

#[tokio::test]
async fn test_length_prefix_4_byte_big_endian() {
    let action = LengthPrefixAction::from_params(&HashMap::new()).unwrap();

    let mut data = Vec::new();
    data.extend_from_slice(&5u32.to_be_bytes());
    data.extend_from_slice(b"hello");

    let cursor = Cursor::new(data);
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    let result = action.execute(msg).await.unwrap();

    match result {
        ActionResult::YieldFrame { frame, .. } => {
            assert_eq!(frame.metadata().get_str("data"), Some("hello".to_string()));
            assert_eq!(
                frame.metadata().get_str("frame_length"),
                Some("5".to_string())
            );
        }
        _ => panic!("Expected YieldFrame"),
    }
}

#[tokio::test]
async fn test_length_prefix_2_byte_little_endian() {
    let mut params = HashMap::new();
    params.insert("length_bytes".to_string(), Value::Number(2.into()));
    params.insert("endian".to_string(), Value::String("little".to_string()));

    let action = LengthPrefixAction::from_params(&params).unwrap();

    let mut data = Vec::new();
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(b"test");

    let cursor = Cursor::new(data);
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    let result = action.execute(msg).await.unwrap();

    match result {
        ActionResult::YieldFrame { frame, .. } => {
            assert_eq!(frame.metadata().get_str("data"), Some("test".to_string()));
        }
        _ => panic!("Expected YieldFrame"),
    }
}

#[tokio::test]
async fn test_length_prefix_1_byte() {
    let mut params = HashMap::new();
    params.insert("length_bytes".to_string(), Value::Number(1.into()));

    let action = LengthPrefixAction::from_params(&params).unwrap();

    let mut data = Vec::new();
    data.push(3u8);
    data.extend_from_slice(b"abc");

    let cursor = Cursor::new(data);
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    let result = action.execute(msg).await.unwrap();

    match result {
        ActionResult::YieldFrame { frame, .. } => {
            assert_eq!(frame.metadata().get_str("data"), Some("abc".to_string()));
        }
        _ => panic!("Expected YieldFrame"),
    }
}

#[tokio::test]
async fn test_length_prefix_max_frame_size() {
    let mut params = HashMap::new();
    params.insert("max_frame_size".to_string(), Value::Number(10.into()));

    let action = LengthPrefixAction::from_params(&params).unwrap();

    let mut data = Vec::new();
    data.extend_from_slice(&100u32.to_be_bytes()); // Frame size exceeds max
    data.extend_from_slice(&[0u8; 100]);

    let cursor = Cursor::new(data);
    let reader = BufReader::new(cursor);
    let msg = Message::new_stream(reader);

    let result = action.execute(msg).await;
    assert!(result.is_err()); // Should error on oversized frame
}

#[test]
fn test_length_prefix_invalid_bytes() {
    let mut params = HashMap::new();
    params.insert("length_bytes".to_string(), Value::Number(3.into()));

    let result = LengthPrefixAction::from_params(&params);
    assert!(result.is_err());
}
