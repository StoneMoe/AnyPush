//! Directory inbound handler - watches a directory for new files

use crate::config::DirectoryInboundConfig;
use crate::inbound::InboundHandler;
use crate::message::Message;
use anyhow::{Context, Result};
use async_trait::async_trait;
use glob::Pattern;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc as std_mpsc;
use tokio::fs::File;
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Directory inbound handler for watching a directory for new files
pub struct DirectoryInbound {
    config: DirectoryInboundConfig,
}

impl DirectoryInbound {
    pub fn new(config: DirectoryInboundConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InboundHandler for DirectoryInbound {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn start(&self, sender: mpsc::Sender<Message>) -> Result<()> {
        let path = Path::new(&self.config.path);

        // Ensure directory exists
        if !path.exists() {
            std::fs::create_dir_all(path)
                .with_context(|| format!("Failed to create directory: {}", self.config.path))?;
        }

        info!(
            "Directory inbound '{}' watching {}",
            self.config.name, self.config.path
        );

        let (tx, rx) = std_mpsc::channel::<notify::Result<Event>>();

        let mut watcher = RecommendedWatcher::new(tx, Config::default())
            .context("Failed to create directory watcher")?;

        let recursive_mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher
            .watch(path, recursive_mode)
            .with_context(|| format!("Failed to watch directory: {}", self.config.path))?;

        let name = self.config.name.clone();
        let pattern = self.config.pattern.clone();
        let delete_after = self.config.delete_after;

        // Compile glob pattern if provided
        let glob_pattern = pattern.as_ref().map(|p| Pattern::new(p).ok()).flatten();

        // Track processed files to avoid duplicates
        let mut processed: HashSet<String> = HashSet::new();

        // Process directory events in blocking task
        tokio::task::spawn_blocking(move || {
            // Keep watcher alive
            let _watcher = watcher;

            loop {
                match rx.recv() {
                    Ok(Ok(event)) => {
                        // Only process create events (new files)
                        if !matches!(event.kind, EventKind::Create(_)) {
                            continue;
                        }

                        for path in event.paths {
                            // Skip directories
                            if path.is_dir() {
                                continue;
                            }

                            let path_str = path.to_string_lossy().to_string();

                            // Skip if already processed
                            if processed.contains(&path_str) {
                                continue;
                            }

                            // Check glob pattern if configured
                            if let Some(ref pattern) = glob_pattern {
                                if let Some(file_name) = path.file_name() {
                                    if !pattern.matches(&file_name.to_string_lossy()) {
                                        debug!("File {} doesn't match pattern, skipping", path_str);
                                        continue;
                                    }
                                }
                            }

                            debug!("New file detected: {}", path_str);
                            processed.insert(path_str.clone());

                            let sender = sender.clone();
                            let name = name.clone();
                            let path_for_task = path.clone();
                            let delete = delete_after;

                            let rt = tokio::runtime::Handle::current();
                            rt.spawn(async move {
                                // Wait a bit for file to be fully written
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                                match File::open(&path_for_task).await {
                                    Ok(file) => {
                                        let reader = BufReader::new(file);
                                        let mut msg =
                                            Message::new_stream(reader).with_source(&name);

                                        let file_name = path_for_task
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_default();

                                        msg.metadata_mut()
                                            .set_str("file.path", path_for_task.to_string_lossy())
                                            .set_str("file.name", &file_name);

                                        info!("Directory file message: {} - {}", msg.id, file_name);

                                        if let Err(e) = sender.send(msg).await {
                                            error!("Failed to send message: {}", e);
                                        }

                                        // Delete file if configured (after a delay to allow processing)
                                        if delete {
                                            tokio::time::sleep(std::time::Duration::from_secs(1))
                                                .await;
                                            if let Err(e) =
                                                tokio::fs::remove_file(&path_for_task).await
                                            {
                                                warn!(
                                                    "Failed to delete file after processing: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to open file {}: {}",
                                            path_for_task.display(),
                                            e
                                        );
                                    }
                                }
                            });
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
        info!("Stopping directory inbound '{}'", self.config.name);
        Ok(())
    }
}
