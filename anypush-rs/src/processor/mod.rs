pub mod actions;

use crate::config::{StepConfig, WhenClause};
use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use evalexpr::{eval_boolean_with_context, ContextWithMutableVariables, HashMapContext, Value};
use regex::Regex;
use tracing::debug;

/// Result of executing an action
#[derive(Debug)]
pub enum ActionResult {
    /// Continue to next step with (possibly modified) message
    Continue(Message),
    /// Stop pipeline, route to outbound handler(s)
    Route { to: Vec<String>, message: Message },
    /// Stop pipeline, reject the transaction
    Reject { message: String },
    /// Stop pipeline, silently drop the message
    Drop,
}

/// Trait for pipeline actions
#[async_trait]
pub trait Action: Send + Sync {
    /// Get the action name/type
    fn name(&self) -> &str;

    /// Whether this is a final action (terminates pipeline)
    fn is_final(&self) -> bool;

    /// Execute the action on the message
    async fn execute(&self, message: Message) -> Result<ActionResult>;
}

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

    // Add standard fields
    if let Some(v) = message.get_field("from") {
        context.set_value("from".to_string(), Value::String(v))?;
    }
    if let Some(v) = message.get_field("to") {
        context.set_value("to".to_string(), Value::String(v))?;
    }
    if let Some(v) = message.get_field("subject") {
        context.set_value("subject".to_string(), Value::String(v))?;
    }
    if let Some(v) = message.get_field("text") {
        context.set_value("text".to_string(), Value::String(v))?;
    }

    // Add metadata fields
    for (key, value) in &message.metadata {
        context.set_value(format!("metadata_{}", key), Value::String(value.clone()))?;
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

pub fn execute_pipeline<'a>(
    steps: &'a [StepConfig],
    mut message: Message,
    current_inbound: Option<&'a str>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ActionResult>> + Send + 'a>> {
    Box::pin(async move {
        for step in steps {
            // Check when clause
            if let Some(ref when) = step.when_clause {
                if !evaluate_when_clause(when, &message, current_inbound) {
                    continue; // Skip this step
                }
            }

            // Handle goto (sub-pipeline)
            if let Some(ref sub_steps) = step.goto {
                let result = execute_pipeline(sub_steps, message.clone(), current_inbound).await?;
                match result {
                    ActionResult::Continue(msg) => {
                        // Sub-pipeline returned, continue with modified message
                        message = msg;
                        continue;
                    }
                    _ => return Ok(result), // Final action in sub-pipeline terminates
                }
            }

            // Handle action
            if let Some(ref action_type) = step.action {
                let action = actions::create_action(action_type, &step.params)?;
                let result = action.execute(message.clone()).await?;
                match result {
                    ActionResult::Continue(msg) => {
                        message = msg;
                        // Continue to next step
                    }
                    _ => return Ok(result), // Final action terminates pipeline
                }
            }
        }

        // No final action hit, return with current message
        Ok(ActionResult::Continue(message))
    })
}
