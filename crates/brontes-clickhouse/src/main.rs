use std::sync::atomic::Ordering;
use tokio::signal;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use dotenv::dotenv;
use tokio::sync::mpsc;

mod clickhouse;
mod binance_fetcher;

use clickhouse::{ClickHouseConfig, ClickHouseWriter};
use binance_fetcher::BinanceFetcher;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenv().ok();

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Failed to set tracing subscriber: {}", e);
        return;
    }

    // Load ClickHouse configuration
    let clickhouse_config = match ClickHouseConfig::from_env() {
        Ok(config) => {
            info!("Successfully loaded ClickHouse configuration");
            config
        }
        Err(e) => {
            eprintln!("Failed to load ClickHouse configuration: {}", e);
            return;
        }
    };

    // Create ClickHouse writer
    let clickhouse_writer = ClickHouseWriter::new(clickhouse_config);
    let keep_running = clickhouse_writer.get_keep_running();
    let keep_running_fetcher = keep_running.clone();

    // Create channel for price updates
    let (tx, rx) = mpsc::channel(1000);

    // Spawn ClickHouse writer task
    let writer_handle = tokio::spawn(async move {
        if let Err(e) = clickhouse_writer.start_writer(rx).await {
            eprintln!("ClickHouse writer error: {}", e);
        }
    });

    // Spawn Binance fetcher task
    let mut binance_fetcher = BinanceFetcher::new(tx, keep_running_fetcher);
    let fetcher_handle = tokio::spawn(async move {
        binance_fetcher.run().await;
    });

    // Handle Ctrl+C
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        keep_running.store(false, Ordering::SeqCst);
        info!("Ctrl+C received, shutting down...");
    });

    // Wait for tasks to complete
    if let Err(e) = fetcher_handle.await {
        eprintln!("Binance fetcher task error: {:?}", e);
    }
    if let Err(e) = writer_handle.await {
        eprintln!("ClickHouse writer task error: {:?}", e);
    }

    info!("Application shut down gracefully");
} 