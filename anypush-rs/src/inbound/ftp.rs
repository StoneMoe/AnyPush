//! FTP server inbound handler - receives file uploads via FTP

use crate::config::FtpInboundConfig;
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tracing::{error, info};
use unftp_sbe_fs::ServerExt;

/// FTP inbound handler that runs an FTP server
pub struct FtpInbound {
    config: FtpInboundConfig,
}

impl FtpInbound {
    pub fn new(config: FtpInboundConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InboundHandler for FtpInbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);

        // Create temp directory for FTP uploads
        let temp_dir = std::env::temp_dir().join(format!("anypush-ftp-{}", self.config.name));
        std::fs::create_dir_all(&temp_dir)
            .with_context(|| format!("Failed to create FTP temp directory: {:?}", temp_dir))?;

        info!(
            "FTP inbound '{}' listening on {} (storage: {:?})",
            self.config.name, addr, temp_dir
        );

        let name = self.config.name.clone();
        let anonymous = self.config.anonymous;
        let temp_dir_clone = temp_dir.clone();

        // Start directory watcher for FTP uploads
        let sender_clone = sender.clone();
        let name_clone = name.clone();
        tokio::spawn(async move {
            watch_ftp_uploads(temp_dir_clone, sender_clone, name_clone).await;
        });

        // Build FTP server
        let server_builder = libunftp::Server::with_fs(temp_dir)
            .greeting("Welcome to AnyPush FTP")
            .passive_ports(50000..=50100);

        let server_builder = if anonymous {
            server_builder.authenticator(Arc::new(libunftp::auth::AnonymousAuthenticator))
        } else {
            server_builder
        };

        let server = server_builder
            .build()
            .context("Failed to build FTP server")?;

        // Run FTP server
        server.listen(addr).await.context("FTP server failed")?;

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping FTP inbound '{}'", self.config.name);
        Ok(())
    }
}

/// Watch FTP uploads directory and send new files as messages
async fn watch_ftp_uploads(dir: PathBuf, sender: mpsc::Sender<Message>, name: String) {
    use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc as std_mpsc;

    let (tx, rx) = std_mpsc::channel::<notify::Result<Event>>();

    let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create FTP upload watcher: {}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive) {
        error!("Failed to watch FTP directory: {}", e);
        return;
    }

    tokio::task::spawn_blocking(move || {
        let _watcher = watcher;

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    // Look for file close/modify events indicating upload complete
                    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        continue;
                    }

                    for path in event.paths {
                        if path.is_dir() {
                            continue;
                        }

                        let path_str = path.to_string_lossy().to_string();
                        let name = name.clone();
                        let sender = sender.clone();

                        let rt = tokio::runtime::Handle::current();
                        rt.spawn(async move {
                            // Wait for upload to complete
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                            match tokio::fs::File::open(&path).await {
                                Ok(file) => {
                                    let reader = BufReader::new(file);
                                    let mut msg = Message::new_stream(reader).with_source(&name);

                                    let file_name = path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();

                                    msg.metadata_mut()
                                        .set_str("file.path", &path_str)
                                        .set_str("file.name", &file_name)
                                        .set_str("ftp.uploaded", "true");

                                    info!("FTP upload message: {} - {}", msg.id, file_name);

                                    if let Err(e) = sender.send(msg).await {
                                        error!("Failed to send FTP message: {}", e);
                                    }

                                    // Clean up uploaded file after processing
                                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                    let _ = tokio::fs::remove_file(&path).await;
                                }
                                Err(e) => {
                                    error!("Failed to open FTP uploaded file: {}", e);
                                }
                            }
                        });
                    }
                }
                Ok(Err(e)) => {
                    error!("FTP watch error: {}", e);
                }
                Err(_) => break,
            }
        }
    });
}
