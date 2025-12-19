use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber;

mod config;
mod inbound;
mod message;
mod outbound;
mod processor;
mod router;

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
    info!("Loaded configuration successfully");

    // Create router from config
    let router = Router::from_config(&config)?;
    info!(
        "Router initialized with {} inbound and {} outbound handlers",
        config.inbound.len(),
        config.outbound.len()
    );

    // Start inbound handlers and get message receiver
    let mut rx = router.start().await?;
    info!("AnyPush is running. Press Ctrl+C to stop.");

    // Process messages in a loop
    loop {
        tokio::select! {
            Some(message) = rx.recv() => {
                info!("Processing message: {}", message.id);
                if let Err(e) = router.process_message(message).await {
                    tracing::error!("Error processing message: {}", e);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down...");
                break;
            }
        }
    }

    Ok(())
}
