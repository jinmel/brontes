use clickhouse_rs::{Pool, Block, row};
use tokio::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use eyre::Result;
#[derive(Debug)]
pub struct ClickHouseConfig {
    pub url: String,
    pub port: String,
    pub user: String,
    pub database: String,
    pub password: String,
}

impl ClickHouseConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            url: std::env::var("CLICKHOUSE_URL")?,
            port: std::env::var("CLICKHOUSE_PORT")?,
            user: std::env::var("CLICKHOUSE_USER")?,
            database: std::env::var("CLICKHOUSE_DATABASE")?,
            password: std::env::var("CLICKHOUSE_PASS")?,
        })
    }

    pub fn connection_string(&self) -> String {
        format!(
            "tcp://{}:{}?user={}&password={}&database={}",
            self.url, self.port, self.user, self.password, self.database
        )
    }
}

#[derive(Debug, Clone)]
pub struct PriceUpdate {
    pub symbol: String,
    pub timestamp: u64,
    pub ask_amount: f64,
    pub ask_price: f64,
    pub bid_price: f64,
    pub bid_amount: f64,
}

pub struct ClickHouseWriter {
    pool: Pool,
    keep_running: Arc<AtomicBool>,
}

impl ClickHouseWriter {
    pub fn new(config: ClickHouseConfig) -> Self {
        let pool = Pool::new(config.connection_string());
        let keep_running = Arc::new(AtomicBool::new(true));
        Self { pool, keep_running }
    }

    pub fn get_keep_running(&self) -> Arc<AtomicBool> {
        self.keep_running.clone()
    }

    pub async fn start_writer(&self, mut receiver: mpsc::Receiver<PriceUpdate>) -> Result<()> {
        while self.keep_running.load(Ordering::Relaxed) {
            let mut updates = Vec::new();
            
            // Collect updates for 1 second or until channel is empty
            let timeout = tokio::time::sleep(Duration::from_secs(1));
            tokio::select! {
                _ = timeout => {}
                Some(update) = receiver.recv() => {
                    updates.push(update);
                    // Try to collect more updates if available
                    while let Ok(update) = receiver.try_recv() {
                        updates.push(update);
                    }
                }
            }

            if !updates.is_empty() {
                let updates_len = updates.len();
                let mut block = Block::new();
                for update in updates {
                    block.push(row!(
                        exchange: "binance".to_string(),
                        symbol: update.symbol,
                        timestamp: update.timestamp,
                        ask_amount: update.ask_amount,
                        ask_price: update.ask_price,
                        bid_price: update.bid_price,
                        bid_amount: update.bid_amount,
                    ))?;
                }

                let mut client = self.pool.get_handle().await?;
                client.insert("cex.normalized_quotes", block).await?;
                info!("Inserted {} price updates to ClickHouse", updates_len);
            }
        }

        Ok(())
    }
} 