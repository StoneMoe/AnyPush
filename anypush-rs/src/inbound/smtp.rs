use crate::config::SmtpInboundConfig;
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use mail_parser::MessageParser;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// SMTP inbound handler - pure Rust implementation using tokio
pub struct SmtpInbound {
    config: SmtpInboundConfig,
}

impl SmtpInbound {
    pub fn new(config: SmtpInboundConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InboundHandler for SmtpInbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("SMTP inbound '{}' listening on {}", self.config.name, addr);

        let name = self.config.name.clone();

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    debug!("SMTP connection from {:?}", peer);
                    let sender = sender.clone();
                    let handler_name = name.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_smtp_connection(stream, sender, &handler_name).await
                        {
                            error!("SMTP session error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping SMTP inbound '{}'", self.config.name);
        Ok(())
    }
}

async fn handle_smtp_connection(
    stream: tokio::net::TcpStream,
    sender: mpsc::Sender<Message>,
    handler_name: &str,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    writer
        .write_all(b"220 AnyPush SMTP Server Ready\r\n")
        .await?;

    let mut mail_from = String::new();
    let mut rcpt_to: Vec<String> = Vec::new();
    let mut data_mode = false;
    let mut data_buffer = String::new();

    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break;
        }

        let line = line.trim();
        debug!("SMTP: {}", line);

        if data_mode {
            if line == "." {
                data_mode = false;

                let parser = MessageParser::default();
                if let Some(parsed) = parser.parse(data_buffer.as_bytes()) {
                    let subject = parsed.subject().map(|s| s.to_string()).unwrap_or_default();
                    let text = parsed
                        .body_text(0)
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    let message = Message::email(mail_from.clone(), rcpt_to.clone(), subject, text)
                        .with_source(handler_name);

                    info!(
                        "Received email: {} -> {:?}",
                        message.metadata().get_str("smtp.from").unwrap_or_default(),
                        message.metadata().get("smtp.to")
                    );

                    if let Err(e) = sender.send(message).await {
                        error!("Failed to send message to router: {}", e);
                    }
                } else {
                    warn!("Failed to parse email");
                }

                writer.write_all(b"250 OK\r\n").await?;
                data_buffer.clear();
                mail_from.clear();
                rcpt_to.clear();
            } else {
                data_buffer.push_str(line);
                data_buffer.push_str("\r\n");
            }
            continue;
        }

        let upper = line.to_uppercase();

        if upper.starts_with("HELO") || upper.starts_with("EHLO") {
            writer.write_all(b"250 Hello\r\n").await?;
        } else if upper.starts_with("MAIL FROM:") {
            mail_from = extract_address(line);
            writer.write_all(b"250 OK\r\n").await?;
        } else if upper.starts_with("RCPT TO:") {
            rcpt_to.push(extract_address(line));
            writer.write_all(b"250 OK\r\n").await?;
        } else if upper == "DATA" {
            writer
                .write_all(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n")
                .await?;
            data_mode = true;
        } else if upper == "QUIT" {
            writer.write_all(b"221 Bye\r\n").await?;
            break;
        } else if upper == "RSET" {
            mail_from.clear();
            rcpt_to.clear();
            data_buffer.clear();
            writer.write_all(b"250 OK\r\n").await?;
        } else if upper == "NOOP" {
            writer.write_all(b"250 OK\r\n").await?;
        } else {
            writer.write_all(b"500 Command not recognized\r\n").await?;
        }
    }

    Ok(())
}

fn extract_address(line: &str) -> String {
    if let Some(start) = line.find('<') {
        if let Some(end) = line.find('>') {
            return line[start + 1..end].to_string();
        }
    }
    if let Some(pos) = line.find(':') {
        return line[pos + 1..].trim().to_string();
    }
    line.to_string()
}
