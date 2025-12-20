//! When clause evaluation with expression support

use crate::config::WhenClause;
use crate::message::Message;
use anyhow::Result;
use evalexpr::{eval_boolean_with_context, ContextWithMutableVariables, HashMapContext, Value};
use regex::Regex;
use tracing::debug;

/// Evaluate a when clause against a message
pub fn evaluate_when_clause(
    when: &WhenClause,
    message: &Message,
    current_inbound: Option<&str>,
) -> bool {
    // Check 'from' matcher first
    if let Some(ref from) = when.from {
        if let Some(inbound) = current_inbound {
            if !from.matches(inbound) {
                return false;
            }
        } else {
            // No inbound context, can't match 'from'
            return false;
        }
    }

    // Check expression
    if let Some(ref expr) = when.expr {
        match evaluate_expression(expr, message) {
            Ok(result) => result,
            Err(e) => {
                debug!("Expression evaluation failed: {}", e);
                false
            }
        }
    } else {
        // No expression means always true (if 'from' matched or wasn't specified)
        true
    }
}

/// Evaluate an expression string against a message
fn evaluate_expression(expr: &str, message: &Message) -> Result<bool> {
    // Build context with message fields
    let mut context = HashMapContext::new();

    // Add core fields
    context.set_value("id".to_string(), Value::String(message.id.clone()))?;
    if let Some(source) = &message.source_inbound {
        context.set_value("source".to_string(), Value::String(source.clone()))?;
    }

    // Add data fields
    for (key, value) in &message.metadata {
        let str_value = match value {
            serde_json::Value::String(s) => s.clone(),
            _ => value.to_string(),
        };
        context.set_value(key.clone(), Value::String(str_value))?;
    }

    // Handle regex operators (~=) by preprocessing
    let processed_expr = preprocess_regex_expression(expr, message)?;

    // Evaluate the expression
    let result = eval_boolean_with_context(&processed_expr, &context)?;
    Ok(result)
}

/// Preprocess expression to handle ~= regex operator
fn preprocess_regex_expression(expr: &str, message: &Message) -> Result<String> {
    // Simple regex operator handling: field ~= "pattern"
    // Replace with true/false based on actual regex match
    let re = Regex::new(r#"(\w+)\s*~=\s*"([^"]+)""#)?;

    let mut result = expr.to_string();
    for cap in re.captures_iter(expr) {
        let field = &cap[1];
        let pattern = &cap[2];

        let field_value = message.get_field(field).unwrap_or_default();
        let regex = Regex::new(pattern)?;
        let matches = regex.is_match(&field_value);

        let original = cap.get(0).unwrap().as_str();
        result = result.replace(original, if matches { "true" } else { "false" });
    }

    Ok(result)
}
