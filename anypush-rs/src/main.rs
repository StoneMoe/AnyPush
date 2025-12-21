use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber;

mod config;
mod inbound;
mod message;
mod outbound;
mod processor;
mod router;
mod template;

use router::Router;

#[derive(Parser, Debug)]
#[command(name = "anypush")]
#[command(about = "A flexible message router with multiple inbound/outbound handlers")]
struct Args {
    /// Path to config file
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,

    /// Override working directory
    #[arg(short, long)]
    workdir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse CLI args
    let args = Args::parse();
    info!("Starting AnyPush with config: {:?}", args.config);

    // Load config
    let config = config::load_config(&args.config, args.workdir.as_deref())?;

    // Create router from config (Arc for concurrent message processing)
    let router = Arc::new(Router::from_config(&config).await?);

    // Start inbound handlers and get message receiver
    let mut rx = router.start().await?;

    // Process messages concurrently
    loop {
        tokio::select! {
            Some(message) = rx.recv() => {
                let router = router.clone();
                tokio::spawn(async move {
                    if let Err(e) = router.process_message(message).await {
                        tracing::error!("Error processing message: {}", e);
                    }
                });
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down...");
                break;
            }
        }
    }

    Ok(())
}
