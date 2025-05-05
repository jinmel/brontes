use tracing_subscriber::FmtSubscriber;
use dotenv::dotenv;
use eyre::Result;
use crate::streams::binance::Binance;
use crate::streams::Exchange;
use crate::models::NormalizedEvent;

use futures::stream::{Stream, StreamExt};

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

    let binance = Binance::builder()
        .symbol("BTCUSDT")
        .build()
        .unwrap();

    let trades = binance.normalized_trades().await?.map(|result| result.map(NormalizedEvent::Trade));
    let quotes = binance.normalized_quotes().await?.map(|result| result.map(NormalizedEvent::Quote));

    let merged = futures::stream::select(trades, quotes);

    merged.for_each(|event| async {
        match event {
            Ok(event) => {
                println!("Event: {:?}", event);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }).await;
    Ok(())
} 