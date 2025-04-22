use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use binance::websockets::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use dotenv::dotenv;
use tokio::sync::mpsc;

mod clickhouse;
use clickhouse::{ClickHouseConfig, ClickHouseWriter, PriceUpdate};

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

    // Create channel for price updates
    let (tx, rx) = mpsc::channel(1000);

    // Spawn ClickHouse writer task
    let writer_handle = tokio::spawn(async move {
        if let Err(e) = clickhouse_writer.start_writer(rx).await {
            eprintln!("ClickHouse writer error: {}", e);
        }
    });

    info!("Starting Binance price fetcher for all pairs");

    // Track price changes and last timestamp
    let mut last_prices: HashMap<String, f64> = HashMap::new();
    let mut last_timestamp = SystemTime::now();

    // Create a websocket instance
    let mut web_socket = WebSockets::new(move |event: WebsocketEvent| {
        match event {
            WebsocketEvent::DayTickerAll(ticker_events) => {
                let now = SystemTime::now();
                let timestamp = now.duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
                
                // Print timestamp separator if more than 1 second has passed
                if now.duration_since(last_timestamp).unwrap_or(Duration::from_secs(0)) >= Duration::from_secs(1) {
                    info!("─────────────────────────────────────────────────");
                    last_timestamp = now;
                }

                for tick_event in ticker_events {
                    if let Ok(price) = tick_event.current_close.parse::<f64>() {
                        // Only log if price changed or new symbol
                        if let Some(&last_price) = last_prices.get(&tick_event.symbol) {
                            if (price - last_price).abs() > f64::EPSILON {
                                info!(
                                    "Symbol: {:<12} Price: {:<12}",
                                    tick_event.symbol,
                                    price,
                                );
                                last_prices.insert(tick_event.symbol.clone(), price);
                                
                                // Send update to ClickHouse writer
                                if let Err(e) = tx.try_send(PriceUpdate {
                                    symbol: tick_event.symbol.clone(),
                                    price,
                                    timestamp,
                                }) {
                                    eprintln!("Failed to send price update to ClickHouse writer: {}", e);
                                }
                            }
                        } else {
                            // First time seeing this symbol
                            info!(
                                "Symbol: {:<12} Price: {:<12}",
                                tick_event.symbol, price
                            );
                            last_prices.insert(tick_event.symbol.clone(), price);
                            
                            // Send update to ClickHouse writer
                            if let Err(e) = tx.try_send(PriceUpdate {
                                symbol: tick_event.symbol.clone(),
                                price,
                                timestamp,
                            }) {
                                eprintln!("Failed to send price update to ClickHouse writer: {}", e);
                            }
                        }
                    }
                }
            },
            _ => (),
        };
        Ok(())
    });

    // Connect to the websocket
    let agg_trade = "!ticker@arr"; // All symbols ticker stream
    if let Err(e) = web_socket.connect(&agg_trade) {
        eprintln!("Failed to connect to websocket: {}", e);
        return;
    }

    // Handle Ctrl+C
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        keep_running.store(false, Ordering::SeqCst);
    });

    // Start the event loop
    if let Err(e) = web_socket.event_loop(&keep_running) {
        eprintln!("Websocket error: {:?}", e);
    }

    // Disconnect when done
    if let Err(e) = web_socket.disconnect() {
        eprintln!("Error disconnecting: {:?}", e);
    }

    // Wait for ClickHouse writer to finish
    if let Err(e) = writer_handle.await {
        eprintln!("ClickHouse writer task error: {:?}", e);
    }

    info!("Disconnected from Binance websocket");
} 