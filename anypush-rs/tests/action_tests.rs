//! Tests for basic actions (log, drop, reject, route, base64, field)

use anypush::message::Message;
use anypush::processor::actions::{
    Base64DecodeAction, Base64EncodeAction, DeleteFieldAction, DropAction, LogAction, RejectAction,
    RouteAction, SetFieldAction,
};
use anypush::processor::{Action, ActionResult};
use serde_yaml::Value;
use std::collections::HashMap;

#[tokio::test]
async fn test_drop_action() {
    let action = DropAction;
    let msg = Message::new();

    let result = action.execute(msg).await.unwrap();
    assert!(matches!(result, ActionResult::Drop));
}

#[tokio::test]
async fn test_log_action_default_level() {
    let action = LogAction::from_params(&HashMap::new()).unwrap();
    let msg = Message::email(
        "test@test.com".to_string(),
        vec![],
        "Subject".to_string(),
        "Body".to_string(),
    );

    let result = action.execute(msg).await.unwrap();
    assert!(matches!(result, ActionResult::Continue(_)));
}

#[tokio::test]
async fn test_log_action_debug_level() {
    let mut params = HashMap::new();
    params.insert("level".to_string(), Value::String("debug".to_string()));

    let action = LogAction::from_params(&params).unwrap();
    let msg = Message::new();

    let result = action.execute(msg).await.unwrap();
    assert!(matches!(result, ActionResult::Continue(_)));
}

#[tokio::test]
async fn test_reject_action_default_message() {
    let action = RejectAction::from_params(&HashMap::new()).unwrap();
    let msg = Message::new();

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Reject { message } = result {
        assert_eq!(message, "Rejected");
    } else {
        panic!("Expected Reject result");
    }
}

#[tokio::test]
async fn test_reject_action_custom_message() {
    let mut params = HashMap::new();
    params.insert(
        "message".to_string(),
        Value::String("Spam detected".to_string()),
    );

    let action = RejectAction::from_params(&params).unwrap();
    let msg = Message::new();

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Reject { message } = result {
        assert_eq!(message, "Spam detected");
    } else {
        panic!("Expected Reject result");
    }
}

#[tokio::test]
async fn test_route_action_single_target() {
    let mut params = HashMap::new();
    params.insert("to".to_string(), Value::String("webhook".to_string()));

    let action = RouteAction::from_params(&params).unwrap();
    let msg = Message::new();

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Route { to, .. } = result {
        assert_eq!(to, vec!["webhook".to_string()]);
    } else {
        panic!("Expected Route result");
    }
}

#[tokio::test]
async fn test_route_action_multiple_targets() {
    let mut params = HashMap::new();
    params.insert(
        "to".to_string(),
        Value::Sequence(vec![
            Value::String("webhook1".to_string()),
            Value::String("webhook2".to_string()),
        ]),
    );

    let action = RouteAction::from_params(&params).unwrap();
    let msg = Message::new();

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Route { to, .. } = result {
        assert_eq!(to.len(), 2);
        assert!(to.contains(&"webhook1".to_string()));
        assert!(to.contains(&"webhook2".to_string()));
    } else {
        panic!("Expected Route result");
    }
}

#[test]
fn test_route_action_missing_to() {
    let result = RouteAction::from_params(&HashMap::new());
    assert!(result.is_err());
}

#[tokio::test]
async fn test_base64_encode_action() {
    let mut params = HashMap::new();
    params.insert("field".to_string(), Value::String("text".to_string()));

    let action = Base64EncodeAction::from_params(&params).unwrap();

    let mut msg = Message::new();
    msg.metadata_mut().set_str("text", "hello world");

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Continue(msg) = result {
        let encoded = msg.metadata().get_str("text").unwrap();
        assert_eq!(encoded, "aGVsbG8gd29ybGQ=");
    } else {
        panic!("Expected Continue result");
    }
}

#[tokio::test]
async fn test_base64_decode_action() {
    let mut params = HashMap::new();
    params.insert("field".to_string(), Value::String("text".to_string()));

    let action = Base64DecodeAction::from_params(&params).unwrap();

    let mut msg = Message::new();
    msg.metadata_mut().set_str("text", "aGVsbG8gd29ybGQ=");

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Continue(msg) = result {
        let decoded = msg.metadata().get_str("text").unwrap();
        assert_eq!(decoded, "hello world");
    } else {
        panic!("Expected Continue result");
    }
}

#[tokio::test]
async fn test_base64_roundtrip() {
    let mut encode_params = HashMap::new();
    encode_params.insert("field".to_string(), Value::String("text".to_string()));

    let encode_action = Base64EncodeAction::from_params(&encode_params).unwrap();
    let decode_action = Base64DecodeAction::from_params(&encode_params).unwrap();

    let mut msg = Message::new();
    msg.metadata_mut().set_str("text", "test data 123");

    let result = encode_action.execute(msg).await.unwrap();
    let ActionResult::Continue(msg) = result else {
        panic!("Expected Continue");
    };

    let result = decode_action.execute(msg).await.unwrap();
    let ActionResult::Continue(msg) = result else {
        panic!("Expected Continue");
    };

    assert_eq!(msg.metadata().get_str("text").unwrap(), "test data 123");
}

#[tokio::test]
async fn test_set_field_action() {
    let mut params = HashMap::new();
    params.insert("field".to_string(), Value::String("custom".to_string()));
    params.insert(
        "value".to_string(),
        Value::String("custom_value".to_string()),
    );

    let action = SetFieldAction::from_params(&params).unwrap();
    let msg = Message::new();

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Continue(msg) = result {
        assert_eq!(
            msg.metadata().get_str("custom"),
            Some("custom_value".to_string())
        );
    } else {
        panic!("Expected Continue result");
    }
}

#[test]
fn test_set_field_action_missing_field() {
    let mut params = HashMap::new();
    params.insert("value".to_string(), Value::String("value".to_string()));

    let result = SetFieldAction::from_params(&params);
    assert!(result.is_err());
}

#[test]
fn test_set_field_action_missing_value() {
    let mut params = HashMap::new();
    params.insert("field".to_string(), Value::String("field".to_string()));

    let result = SetFieldAction::from_params(&params);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_field_action() {
    let mut params = HashMap::new();
    params.insert("field".to_string(), Value::String("to_delete".to_string()));

    let action = DeleteFieldAction::from_params(&params).unwrap();

    let mut msg = Message::new();
    msg.metadata_mut().set_str("to_delete", "value");

    let result = action.execute(msg).await.unwrap();
    if let ActionResult::Continue(msg) = result {
        assert_eq!(msg.metadata().get_str("to_delete"), None);
    } else {
        panic!("Expected Continue result");
    }
}
