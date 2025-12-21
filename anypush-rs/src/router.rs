use crate::config::{Config, InboundConfig, OutboundConfig, StepConfig};
use crate::inbound::directory::DirectoryInbound;
use crate::inbound::file::FileInbound;
use crate::inbound::ftp::FtpInbound;
use crate::inbound::http::HttpInbound;
use crate::inbound::smtp::SmtpInbound;
use crate::inbound::tcp::TcpInbound;
use crate::inbound::InboundHandler;
use crate::message::Message;
use crate::outbound::http::HttpOutbound;
use crate::outbound::s3::S3Outbound;
use crate::outbound::tcp::TcpOutbound;
use crate::outbound::{OutboundHandler, RouteParams};
use crate::processor::{self, RouteRequest};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Router manages message routing between inbound and outbound handlers
pub struct Router {
    inbound_handlers: HashMap<String, Arc<dyn InboundHandler>>,
    outbound_handlers: HashMap<String, Arc<dyn OutboundHandler>>,
    pipeline: Vec<StepConfig>,
}

impl Router {
    /// Create a new router from configuration
    pub async fn from_config(config: &Config) -> Result<Self> {
        let mut inbound_handlers: HashMap<String, Arc<dyn InboundHandler>> = HashMap::new();
        let mut outbound_handlers: HashMap<String, Arc<dyn OutboundHandler>> = HashMap::new();

        for inbound_cfg in &config.inbound {
            let name = inbound_cfg.name().to_string();
            let handler: Arc<dyn InboundHandler> = match inbound_cfg {
                InboundConfig::Smtp(cfg) => Arc::new(SmtpInbound::new(cfg.clone())),
                InboundConfig::Tcp(cfg) => Arc::new(TcpInbound::new(cfg.clone())),
                InboundConfig::Http(cfg) => Arc::new(HttpInbound::new(cfg.clone())),
                InboundConfig::File(cfg) => Arc::new(FileInbound::new(cfg.clone())),
                InboundConfig::Directory(cfg) => Arc::new(DirectoryInbound::new(cfg.clone())),
                InboundConfig::Ftp(cfg) => Arc::new(FtpInbound::new(cfg.clone())),
            };
            inbound_handlers.insert(name, handler);
        }

        for outbound_cfg in &config.outbound {
            let name = outbound_cfg.name().to_string();
            let handler: Arc<dyn OutboundHandler> = match outbound_cfg {
                OutboundConfig::Http(cfg) => Arc::new(HttpOutbound::new(cfg.clone())),
                OutboundConfig::Tcp(cfg) => Arc::new(TcpOutbound::new(cfg.clone())),
                OutboundConfig::S3(cfg) => Arc::new(S3Outbound::new(cfg.clone()).await?),
            };
            outbound_handlers.insert(name, handler);
        }

        Ok(Self {
            inbound_handlers,
            outbound_handlers,
            pipeline: config.pipeline.clone(),
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
                    error!("Inbound '{}' start failed: {}", name, e);
                }
            });
        }

        Ok(rx)
    }

    /// Process a message through the pipeline
    pub async fn process_message(&self, message: Message) -> Result<()> {
        let inbound_name = message.source_inbound.clone();
        debug!(
            "Processing message {} from {:?}",
            message.id,
            inbound_name.as_deref()
        );

        // Create channel for route requests
        let (route_tx, mut route_rx) = mpsc::channel::<RouteRequest>(100);
        let outbound_handlers = self.outbound_handlers.clone();

        // Spawn task to handle route requests
        tokio::spawn(async move {
            while let Some(mut req) = route_rx.recv().await {
                for target in &req.targets {
                    if let Some(handler) = outbound_handlers.get(target) {
                        // Filter params to only include keys this outbound accepts
                        let valid_keys = handler.support_route_params();
                        let filtered_params: RouteParams = req
                            .params
                            .iter()
                            .filter(|(k, _)| valid_keys.contains(&k.as_str()))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();

                        // Use stream passthrough if both source is stream and outbound supports it
                        let result = if req.message.is_stream() && handler.supports_streaming() {
                            if let Some(reader) = req.message.take_stream() {
                                handler.send_stream(reader, &filtered_params).await
                            } else {
                                handler.send(&req.message, &filtered_params).await
                            }
                        } else {
                            handler.send(&req.message, &filtered_params).await
                        };

                        if let Err(e) = result {
                            error!("Failed to route to '{}': {}", target, e);
                        } else {
                            debug!("Routed {} to '{}'", req.message.id, target);
                        }
                    } else {
                        warn!("Outbound '{}' not found", target);
                    }
                }
            }
        });

        // Execute pipeline - ignore ActionResult, routing is handled via channel
        let _ =
            processor::execute_pipeline(&self.pipeline, message, inbound_name.as_deref(), route_tx)
                .await?;

        Ok(())
    }
}
