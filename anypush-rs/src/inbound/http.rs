use crate::config::HttpInboundConfig;
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// HTTP inbound handler - receives messages via HTTP POST
pub struct HttpInbound {
    config: HttpInboundConfig,
}

impl HttpInbound {
    pub fn new(config: HttpInboundConfig) -> Self {
        Self { config }
    }
}

/// Request body for incoming messages
#[derive(Debug, Deserialize)]
struct IncomingMessage {
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: Vec<String>,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    text: String,
}

#[async_trait]
impl InboundHandler for HttpInbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let listener = TcpListener::bind(addr).await?;
        info!("HTTP handler '{}' listening on {}", self.config.name, addr);

        let handler_name = Arc::new(self.config.name.clone());

        loop {
            match listener.accept().await {
                Ok((stream, _peer)) => {
                    let sender = sender.clone();
                    let name = handler_name.clone();
                    let io = TokioIo::new(stream);

                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            let sender = sender.clone();
                            let name = name.clone();
                            async move { handle_request(req, sender, &name).await }
                        });

                        if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                            error!("HTTP connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept HTTP connection: {}", e);
                }
            }
        }
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping HTTP handler '{}'", self.config.name);
        Ok(())
    }
}

async fn handle_request(
    req: Request<Incoming>,
    sender: mpsc::Sender<Message>,
    handler_name: &str,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let (parts, body) = req.into_parts();

    // Only accept POST requests
    if parts.method != Method::POST {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method not allowed. Use POST.")))
            .unwrap());
    }

    // Read body
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Failed to read body")))
                .unwrap());
        }
    };

    // Parse JSON
    let incoming: IncomingMessage = match serde_json::from_slice(&body_bytes) {
        Ok(msg) => msg,
        Err(e) => {
            debug!("Failed to parse JSON: {}", e);
            // If not JSON, treat body as raw text
            IncomingMessage {
                from: String::new(),
                to: vec![],
                subject: String::new(),
                text: String::from_utf8_lossy(&body_bytes).to_string(),
            }
        }
    };

    // Create message
    let message = Message::new(incoming.from, incoming.to, incoming.subject, incoming.text)
        .with_source(handler_name);

    info!("HTTP received message: {}", message.id);

    // Send to router
    if let Err(e) = sender.send(message).await {
        error!("Failed to send message to router: {}", e);
        return Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Full::new(Bytes::from("Internal error")))
            .unwrap());
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(r#"{"status":"ok"}"#)))
        .unwrap())
}
