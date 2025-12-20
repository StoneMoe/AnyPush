use crate::config::HttpInboundConfig;
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Incoming, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{error, info};

/// HTTP inbound handler - receives messages via HTTP requests
pub struct HttpInbound {
    config: HttpInboundConfig,
}

impl HttpInbound {
    pub fn new(config: HttpInboundConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InboundHandler for HttpInbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let listener = TcpListener::bind(addr).await?;
        info!("HTTP inbound '{}' listening on {}", self.config.name, addr);

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
        info!("Stopping HTTP inbound '{}'", self.config.name);
        Ok(())
    }
}

async fn handle_request(
    req: Request<Incoming>,
    sender: mpsc::Sender<Message>,
    handler_name: &str,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let (parts, body) = req.into_parts();

    // Read body as raw bytes
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

    // Extract content type from headers
    let content_type = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    // Create message with raw bytes
    let mut msg = Message::new_with_raw(body_bytes.to_vec()).with_source(handler_name);

    // Store HTTP metadata (prefixed with http. for clarity)
    msg.metadata_mut()
        .set_str("http.method", parts.method.to_string())
        .set_str("http.uri", parts.uri.to_string())
        .set_str("http.path", parts.uri.path().to_string())
        .set_str("http.content_type", content_type.clone());

    // Store query parameters if present
    if let Some(query) = parts.uri.query() {
        msg.metadata_mut().set_str("http.query", query.to_string());
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                msg.metadata_mut()
                    .set_str(&format!("http.query.{}", key), urlencoding_decode(value));
            }
        }
    }

    // Store headers
    let mut headers_map: HashMap<String, JsonValue> = HashMap::new();
    for (name, value) in parts.headers.iter() {
        if let Ok(v) = value.to_str() {
            headers_map.insert(name.to_string(), JsonValue::String(v.to_string()));
        }
    }
    msg.metadata_mut().insert(
        "http.headers".to_string(),
        JsonValue::Object(headers_map.into_iter().collect()),
    );

    // Pre-parse body based on content type (for convenience)
    if content_type.starts_with("application/json") {
        if let Ok(json) = serde_json::from_slice::<JsonValue>(&body_bytes) {
            msg.metadata_mut().set("http.body", json.clone());
            if let JsonValue::Object(map) = json {
                for (key, value) in map {
                    msg.metadata_mut().insert(key, value);
                }
            }
        }
    } else if content_type.starts_with("application/x-www-form-urlencoded") {
        let body_str = String::from_utf8_lossy(&body_bytes);
        for pair in body_str.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                msg.metadata_mut()
                    .set_str(&format!("http.form.{}", key), urlencoding_decode(value));
            }
        }
    } else if content_type.starts_with("text/") {
        msg.metadata_mut()
            .set_str("text", String::from_utf8_lossy(&body_bytes).to_string());
    }

    info!(
        "HTTP {} {} received ({}, {} bytes)",
        parts.method,
        parts.uri.path(),
        msg.id,
        body_bytes.len()
    );

    if let Err(e) = sender.send(msg).await {
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

fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}
