use crate::config::{Config, InboundConfig, OutboundConfig, RouteConfig};
use crate::inbound::http::HttpInbound;
use crate::inbound::smtp::SmtpInbound;
use crate::inbound::tcp::TcpInbound;
use crate::inbound::InboundHandler;
use crate::message::Message;
use crate::outbound::http::HttpOutbound;
use crate::outbound::tcp::TcpOutbound;
use crate::outbound::OutboundHandler;
use crate::processor::{self, ActionResult};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Router manages message routing between inbound and outbound handlers
pub struct Router {
    inbound_handlers: HashMap<String, Arc<dyn InboundHandler>>,
    outbound_handlers: HashMap<String, Arc<dyn OutboundHandler>>,
    routes: Vec<RouteConfig>,
}

impl Router {
    /// Create a new router from configuration
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut inbound_handlers: HashMap<String, Arc<dyn InboundHandler>> = HashMap::new();
        let mut outbound_handlers: HashMap<String, Arc<dyn OutboundHandler>> = HashMap::new();

        // Create inbound handlers
        for inbound_cfg in &config.inbound {
            let handler: Arc<dyn InboundHandler> = match inbound_cfg {
                InboundConfig::Smtp(cfg) => Arc::new(SmtpInbound::new(cfg.clone())),
                InboundConfig::Tcp(cfg) => Arc::new(TcpInbound::new(cfg.clone())),
                InboundConfig::Http(cfg) => Arc::new(HttpInbound::new(cfg.clone())),
            };
            inbound_handlers.insert(inbound_cfg.name().to_string(), handler);
        }

        // Create outbound handlers
        for outbound_cfg in &config.outbound {
            let handler: Arc<dyn OutboundHandler> = match outbound_cfg {
                OutboundConfig::Http(cfg) => Arc::new(HttpOutbound::new(cfg.clone())),
                OutboundConfig::Tcp(cfg) => Arc::new(TcpOutbound::new(cfg.clone())),
            };
            outbound_handlers.insert(outbound_cfg.name().to_string(), handler);
        }

        Ok(Self {
            inbound_handlers,
            outbound_handlers,
            routes: config.routes.clone(),
        })
    }

    /// Start all inbound handlers
    pub async fn start(&self) -> Result<mpsc::Receiver<Message>> {
        let (tx, rx) = mpsc::channel(1000);

        for (name, handler) in &self.inbound_handlers {
            let handler = handler.clone();
            let tx = tx.clone();
            let name = name.clone();

            tokio::spawn(async move {
                if let Err(e) = handler.start(tx).await {
                    error!("Inbound handler '{}' failed: {}", name, e);
                }
            });
        }

        Ok(rx)
    }

    /// Process a message through the routing pipeline
    pub async fn process_message(&self, message: Message) -> Result<()> {
        let inbound_name = message.source_inbound.as_deref();
        debug!("Processing message {} from {:?}", message.id, inbound_name);

        // Find matching route
        for route in &self.routes {
            // Check if route matches this inbound
            if let Some(ref when) = route.when_clause {
                if !processor::evaluate_when_clause(when, &message, inbound_name) {
                    continue;
                }
            }

            // Execute pipeline or single action
            let result = if let Some(ref steps) = route.goto {
                processor::execute_pipeline(steps, message.clone(), inbound_name).await?
            } else if let Some(ref action_type) = route.action {
                // Single action shortcut
                let action = processor::actions::create_action(action_type, &route.action_params)?;
                action.execute(message.clone()).await?
            } else {
                // No action defined, skip to next route
                continue;
            };

            // Handle result
            match result {
                ActionResult::Route { to, message } => {
                    for target in &to {
                        if let Some(handler) = self.outbound_handlers.get(target) {
                            if let Err(e) = handler.send(&message).await {
                                error!("Failed to send to '{}': {}", target, e);
                            }
                        } else {
                            warn!("Outbound handler '{}' not found", target);
                        }
                    }
                    return Ok(());
                }
                ActionResult::Reject { message: msg } => {
                    info!("Message {} rejected: {}", message.id, msg);
                    return Ok(());
                }
                ActionResult::Drop => {
                    debug!("Message {} dropped", message.id);
                    return Ok(());
                }
                ActionResult::Continue(_) => {
                    // No final action, try next route
                    continue;
                }
            }
        }

        warn!(
            "No matching route for message {} from {:?}",
            message.id, inbound_name
        );
        Ok(())
    }
}
