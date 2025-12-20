//! Pipeline execution logic

use super::action::{ActionResult, RouteRequest};
use super::actions;
use super::when::evaluate_when_clause;
use crate::config::StepConfig;
use crate::message::Message;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::debug;

/// Execute a pipeline of steps on a message
pub fn execute_pipeline<'a>(
    steps: &'a [StepConfig],
    message: Message,
    inbound_name: Option<&'a str>,
    route_tx: mpsc::Sender<RouteRequest>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ActionResult>> + Send + 'a>> {
    Box::pin(async move {
        let mut current_msg = message;
        let mut step_idx = 0;

        while step_idx < steps.len() {
            let step = &steps[step_idx];

            // Check when clause - skip step if condition not met
            if let Some(ref when) = step.when_clause {
                if !evaluate_when_clause(when, &current_msg, inbound_name) {
                    step_idx += 1;
                    continue;
                }
            }

            // Execute action if present
            if let Some(ref action_type) = step.action {
                let action = actions::create_action(action_type, &step.params)?;
                let result = action.execute(current_msg.clone()).await?;

                match result {
                    ActionResult::Continue(msg) => {
                        current_msg = msg;
                    }
                    ActionResult::YieldFrame { frame, source } => {
                        // Framing action: process frame through goto then remaining steps
                        let remaining_steps = &steps[step_idx + 1..];
                        process_frame(
                            step.goto.as_deref(),
                            remaining_steps,
                            frame,
                            inbound_name,
                            route_tx.clone(),
                        )
                        .await?;
                        current_msg = source;
                        continue; // Stay on same step to get next frame
                    }
                    ActionResult::Route { to, message } => {
                        let _ = route_tx
                            .send(RouteRequest {
                                targets: to,
                                message,
                            })
                            .await;
                        return Ok(ActionResult::Drop);
                    }
                    result @ (ActionResult::Reject { .. } | ActionResult::Drop) => {
                        return Ok(result);
                    }
                }
            }

            // Execute goto sub-pipeline if present
            if let Some(ref sub_steps) = step.goto {
                let result = execute_pipeline(
                    sub_steps,
                    current_msg.clone(),
                    inbound_name,
                    route_tx.clone(),
                )
                .await?;
                match result {
                    ActionResult::Continue(msg) => {
                        current_msg = msg;
                    }
                    result => return Ok(result),
                }
            }

            step_idx += 1;
        }

        Ok(ActionResult::Continue(current_msg))
    })
}

/// Process a single yielded frame
async fn process_frame(
    goto_steps: Option<&[StepConfig]>,
    remaining_steps: &[StepConfig],
    frame: Message,
    inbound_name: Option<&str>,
    route_tx: mpsc::Sender<RouteRequest>,
) -> Result<()> {
    let mut current_frame = frame;

    // Run through goto if present
    if let Some(goto) = goto_steps {
        if !goto.is_empty() {
            let result =
                execute_pipeline(goto, current_frame.clone(), inbound_name, route_tx.clone())
                    .await?;
            match result {
                ActionResult::Continue(msg) => {
                    current_frame = msg;
                }
                _ => return Ok(()), // Final action - done with this frame
            }
        }
    }

    // Run through remaining parent steps
    if !remaining_steps.is_empty() {
        let _ = execute_pipeline(remaining_steps, current_frame, inbound_name, route_tx).await?;
    }

    Ok(())
}
