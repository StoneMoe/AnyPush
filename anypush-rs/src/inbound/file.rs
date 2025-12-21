//! File inbound handler - watches a single file for changes

use crate::config::{FileInboundConfig, FileMode};
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::{Context, Result};
use async_trait::async_trait;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// File inbound handler for watching a single file
pub struct FileInbound {
    config: FileInboundConfig,
}

impl FileInbound {
    pub fn new(config: FileInboundConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InboundHandler for FileInbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()> {
        let path = Path::new(&self.config.path);

        // Ensure file exists
        if !path.exists() {
            // Create empty file if it doesn't exist
            File::create(path)
                .with_context(|| format!("Failed to create file: {}", self.config.path))?;
        }

        info!(
            "File inbound '{}' watching {}",
            self.config.name, self.config.path
        );

        let (tx, rx) = std_mpsc::channel::<notify::Result<Event>>();

        let mut watcher = RecommendedWatcher::new(tx, Config::default())
            .context("Failed to create file watcher")?;

        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .with_context(|| format!("Failed to watch file: {}", self.config.path))?;

        let path_owned = self.config.path.clone();
        let name = self.config.name.clone();
        let mode = self.config.mode.clone();
        let delete_after = self.config.delete_after;

        // Track file position for tail mode
        let mut last_pos = match mode {
            FileMode::Tail => {
                // Start at end of file for tail mode
                std::fs::metadata(&path_owned).map(|m| m.len()).unwrap_or(0)
            }
            FileMode::Full => 0,
        };

        // Process file events in blocking task
        tokio::task::spawn_blocking(move || {
            // Keep watcher alive
            let _watcher = watcher;

            loop {
                match rx.recv() {
                    Ok(Ok(event)) => {
                        // Only process modify events
                        if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                            continue;
                        }

                        debug!("File event: {:?}", event);

                        match mode {
                            FileMode::Tail => {
                                // Read new content from last position
                                match File::open(&path_owned) {
                                    Ok(mut file) => {
                                        let current_size =
                                            file.metadata().map(|m| m.len()).unwrap_or(0);

                                        if current_size < last_pos {
                                            // File was truncated, start from beginning
                                            last_pos = 0;
                                        }

                                        if current_size > last_pos {
                                            if file.seek(SeekFrom::Start(last_pos)).is_ok() {
                                                let mut content = String::new();
                                                if file.read_to_string(&mut content).is_ok()
                                                    && !content.is_empty()
                                                {
                                                    let mut msg = Message::new_with_raw(
                                                        content.as_bytes().to_vec(),
                                                    )
                                                    .with_source(&name);
                                                    msg.metadata_mut()
                                                        .set_str("text", content.trim())
                                                        .set_str("file.path", &path_owned);

                                                    info!(
                                                        "File tail message: {} ({} bytes)",
                                                        msg.id,
                                                        current_size - last_pos
                                                    );

                                                    let sender = sender.clone();
                                                    let rt = tokio::runtime::Handle::current();
                                                    rt.spawn(async move {
                                                        if let Err(e) = sender.send(msg).await {
                                                            error!("Failed to send message: {}", e);
                                                        }
                                                    });
                                                }
                                                last_pos = current_size;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to open file: {}", e);
                                    }
                                }
                            }
                            FileMode::Full => {
                                // Read entire file
                                match std::fs::read_to_string(&path_owned) {
                                    Ok(content) if !content.is_empty() => {
                                        let mut msg =
                                            Message::new_with_raw(content.as_bytes().to_vec())
                                                .with_source(&name);
                                        msg.metadata_mut()
                                            .set_str("text", &content)
                                            .set_str("file.path", &path_owned);

                                        info!(
                                            "File full message: {} ({} bytes)",
                                            msg.id,
                                            content.len()
                                        );

                                        let sender = sender.clone();
                                        let rt = tokio::runtime::Handle::current();
                                        rt.spawn(async move {
                                            if let Err(e) = sender.send(msg).await {
                                                error!("Failed to send message: {}", e);
                                            }
                                        });

                                        // Delete file if configured
                                        if delete_after {
                                            if let Err(e) = std::fs::remove_file(&path_owned) {
                                                warn!(
                                                    "Failed to delete file after processing: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Ok(_) => {} // Empty file, ignore
                                    Err(e) => {
                                        error!("Failed to read file: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Watch error: {}", e);
                    }
                    Err(_) => {
                        // Channel closed, watcher dropped
                        break;
                    }
                }
            }
        });

        // Keep the task running
        std::future::pending::<()>().await;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping file inbound '{}'", self.config.name);
        Ok(())
    }
}
