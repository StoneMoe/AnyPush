//! Message processor module
//!
//! This module handles pipeline execution and action processing.

mod action;
pub mod actions;
mod pipeline;
mod when;

// Re-export public items
pub use action::{Action, ActionResult, RouteRequest};
pub use pipeline::execute_pipeline;
