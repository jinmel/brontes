use std::sync::atomic::Ordering;
use tokio::signal;
use tracing_subscriber::FmtSubscriber;
use dotenv::dotenv;
use tokio::sync::mpsc;
use eyre::Result;

mod models;
mod streams;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenv().ok();

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
} 